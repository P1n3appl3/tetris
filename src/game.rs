use std::{collections::VecDeque, fmt};

use log::{debug, info};
use ringbuffer::{ConstGenericRingBuffer, RingBuffer};
use tetrizz::beam_search::Node;
use web_time::{Instant, SystemTime};

use crate::{
    sound::{Action, Clear, Meta, Sink, Sound, SoundPlayer},
    *,
};

pub type Board = [[Cell; 10]; 50]; // hope no one stacks higher than this 👀

#[derive(Clone, Debug)]
pub struct Lookahead {
    /// number of placements before board will render again
    pub min_placements: usize,
    /// Amount of time user has to stop making inputs after min_placements has been reached for
    /// board to render
    ///
    /// unit: frames
    pub timeout: u16,
    board_visible: bool,
    next_piece_goal: usize,
    // state
    // /// count of pieces per burst, for proper undo support
    // bursts: VecDeque<u8>,
}

impl Lookahead {
    pub fn new(min_placements: usize, timeout: u16) -> Self {
        Self { min_placements, timeout, board_visible: true, next_piece_goal: min_placements }
    }
}

#[derive(Debug, Clone)]
pub enum Mode {
    Sprint {
        target_lines: u16,
    },
    // Cheese { target_lines: u16 },
    TrainingLab {
        lookahead: Option<Lookahead>,
        search: bool,
        mino_mode: bool,
        // search config
    },
}

impl Mode {
    pub fn is_complete(&self, lines: u16) -> bool {
        match self {
            &Mode::Sprint { target_lines } => lines >= target_lines,
            _ => false,
        }
    }

    fn allows_undo(&self) -> bool {
        match self {
            Mode::Sprint { .. } => false,
            Mode::TrainingLab { .. } => true,
        }
    }

    pub fn search_enabled(&self) -> bool {
        match self {
            Mode::Sprint { .. } => false,
            Mode::TrainingLab { search, .. } => *search,
        }
    }

    fn lookahead_timeout(&self) -> u16 {
        match self {
            Mode::TrainingLab { lookahead: Some(lookahead), .. } => lookahead.timeout,
            _ => unreachable!(),
        }
    }

    fn start(&mut self) {
        if let Mode::TrainingLab { lookahead: Some(lookahead), .. } = self {
            lookahead.board_visible = true;
            lookahead.next_piece_goal = lookahead.min_placements;
        }
    }
}

#[derive(Clone)]
pub struct Moment {
    pub board: [[Cell; 10]; 50], // hope no one stacks higher than this 👀
    pub current: PieceLocation,
    pub hold: Option<Piece>,
    pub upcomming: ConstGenericRingBuffer<Piece, 14>,
    pub spins: Vec<Node>,
    pub pieces_placed: usize,
}

#[derive(Clone)]
pub struct Game {
    pub board: Board,
    pub upcomming: ConstGenericRingBuffer<Piece, 14>,
    pub current: PieceLocation,
    pub hold: Option<Piece>,
    pub lines: u16,
    pub pieces: usize,
    pub mode: Mode,
    pub config: Config,
    pub timers: VecDeque<(Instant, TimerEvent)>,
    pub time: Instant,
    pub started_right: Option<Instant>,
    pub started_left: Option<Instant>,
    pub start_time: Option<Instant>,
    pub end_time: Option<Instant>,
    pub soft_dropping: bool,
    pub can_hold: bool,
    pub spin: bool, // TODO: detection
    pub state: GameState,
    pub rng: StdRng,
    pub spins: Vec<Node>,
    pub solution: Option<(Node, Box<Game>)>,
    pub history: VecDeque<Moment>,
}

struct SpinFormatter<'a>(&'a Game);

impl fmt::Display for SpinFormatter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let spins = Game::spin_shortlist(&self.0.spins);
        if let Some((suggestion, _)) = &self.0.solution {
            // render selected solution
            let last = suggestion.moves.len() - 1;
            log::info!("{last}");
            for (m, i) in suggestion.moves.iter() {
                let b2b = if m.spun && i.lines_cleared > 0 { " +1 b2b" } else { "" };
                writeln!(f, "{:?} x:{} y:{} {:?}{}", m.piece, m.x, m.y, m.rotation, b2b)?;
                if !b2b.is_empty() {
                    break;
                }
            }
        } else {
            if self.0.history.is_empty() {
                return Ok(());
            }
            let prev = self.0.history.back().unwrap();
            // render available solutions
            for (id, spin) in spins.iter().enumerate() {
                let spin_move_position =
                    spin.moves.iter().position(|m| m.0.spun && m.1.lines_cleared > 0).unwrap();
                let spin_move = spin.moves[spin_move_position];
                let prev_moves = prev.spins.iter().map(|node| &node.moves).collect::<Vec<_>>();

                let matches_previous_move = prev_moves.iter().any(|&prev_moveset| {
                    let prev_spin_move_position = prev_moveset
                        .iter()
                        .position(|m| m.0.spun && m.1.lines_cleared > 0)
                        .unwrap();
                    let prev_spin_move = prev_moveset[prev_spin_move_position];
                    prev_spin_move.0.piece == spin_move.0.piece
                        && prev_spin_move_position > spin_move_position
                });
                let solution_indicator = if matches_previous_move { "→" } else { "★" };
                writeln!(
                    f,
                    "{}: {} {:?} spin in {} pieces clearing {} lines",
                    id + 1,
                    solution_indicator,
                    spin_move.0.piece,
                    spin_move_position,
                    spin_move.1.lines_cleared,
                )?;
            }
        }
        Ok(())
    }
}

impl Game {
    pub fn as_tetrizz_game_and_queue(&self) -> (tetrizz::data::Game, Vec<tetrizz::data::Piece>) {
        let mut queue: Vec<_> = std::iter::once(self.current.piece.into())
            .chain(self.upcomming.iter().cloned().map(Into::into))
            .collect();
        let game = tetrizz::data::Game {
            board: self.as_tetrizz_board(),
            hold: self.hold.map(Into::into).unwrap_or_else(|| queue.remove(0)),
            b2b: 0,
            b2b_deficit: 0,
        };
        (game, queue)
    }
    fn as_tetrizz_board(&self) -> tetrizz::data::Board {
        let mut board = tetrizz::data::Board { cols: [tetrizz::data::Column(0); 10] };
        for column in 0..10 {
            for row in 0..50 {
                if self.board[row][column] != Cell::Empty {
                    board.cols[column].0 |= 1 << row;
                }
            }
        }
        board
    }

    pub fn spin_shortlist(nodes: &[Node]) -> Vec<Node> {
        let mut bounties =
            vec![Piece::I, Piece::J, Piece::L, Piece::T, Piece::Z, Piece::S, Piece::T];
        let mut spins = vec![];
        'find_bounties: for node in nodes.iter() {
            for (m, placement_info) in node.moves.iter() {
                if m.spun && placement_info.lines_cleared > 0 {
                    let piece = m.piece.into();
                    let bounty = bounties.iter().position(|&b| b == piece);
                    if let Some(bounty) = bounty {
                        spins.push(node.clone());
                        bounties.remove(bounty);
                        if bounties.is_empty() {
                            break 'find_bounties;
                        }
                    }
                    break;
                }
            }
        }
        spins.sort_by_key(|s| s.moves.iter().take_while(|m| m.1.lines_cleared == 0).count());
        spins
    }

    pub fn display_spins(&self) -> impl fmt::Display + '_ {
        SpinFormatter(self)
    }

    pub fn draw_only_mino(&self) -> bool {
        match self.mode {
            Mode::Sprint { .. } => false,
            Mode::TrainingLab { mino_mode, .. } => mino_mode,
        }
    }

    pub fn should_draw_board(&self) -> bool {
        match &self.mode {
            Mode::TrainingLab { lookahead: Some(lookahead), .. } => lookahead.board_visible,
            _ => true,
        }
    }

    pub fn should_draw_hold(&self) -> bool {
        match &self.mode {
            Mode::TrainingLab { lookahead: Some(lookahead), .. } => lookahead.board_visible,
            _ => true,
        }
    }

    pub fn should_draw_queue(&self) -> bool {
        match &self.mode {
            Mode::TrainingLab { lookahead: Some(lookahead), .. } => lookahead.board_visible,
            _ => true,
        }
    }
}

impl Game {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            rng: StdRng::from_os_rng(),
            board: [[Cell::Empty; 10]; 50],
            upcomming: Default::default(),
            current: PieceLocation::new(Piece::I, (3, 21), Rotation::North),
            hold: None,
            lines: 0,
            pieces: 0,
            mode: Mode::Sprint { target_lines: 40 },
            timers: Default::default(),
            started_right: None,
            started_left: None,
            time: Instant::now(),
            start_time: None,
            end_time: None,
            soft_dropping: false,
            can_hold: true,
            spin: false,
            state: GameState::Done,
            spins: Default::default(),
            solution: None,
            history: VecDeque::new(),
        }
    }

    pub fn start(&mut self, seed: Option<u64>, sound: &SoundPlayer<impl Sink>) {
        self.state = GameState::Startup;
        self.board = [[Cell::Empty; 10]; 50];
        self.hold = None;
        self.lines = 0;
        self.pieces = 0;
        self.upcomming.clear();
        self.rng = StdRng::seed_from_u64(seed.unwrap_or_else(|| {
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as u64
        }));
        self.fill_bag();
        self.time = Instant::now();
        self.start_time = None;
        if matches!(self.mode, Mode::Sprint { .. }) {
            while let Some(Piece::Z | Piece::S) = self.upcomming.front() {
                self.upcomming.clear();
                self.fill_bag();
            }
        }
        self.mode.start();
        sound.play(Meta::Go).ok();
        // TODO: combine "ready" and "go" sounds
        // TODO: make startup time configurable or maybe even based on the sound length?
        self.timers.clear();
        self.set_timer(TimerEvent::Start);
    }

    pub fn handle(&mut self, event: Event, time: Instant, sound: &SoundPlayer<impl Sink>) -> bool {
        use {Event::*, TimerEvent::*};
        let ret = self._handle(event, time, sound);
        if let Input(kind) = event {
            if let Mode::TrainingLab { lookahead: Some(lookahead), .. } = &mut self.mode {
                if lookahead.board_visible && kind != InputEvent::Undo {
                    lookahead.board_visible = false;
                } else if self.pieces >= lookahead.next_piece_goal {
                    self.clear_timer(Lookahead);
                    self.set_timer(Lookahead);
                }
            }
        }
        ret
    }

    pub fn _handle(&mut self, event: Event, time: Instant, sound: &SoundPlayer<impl Sink>) -> bool {
        use {Event::*, GameState::*, InputEvent::*, TimerEvent::*};
        self.time = time;
        debug!("handling event: {event:?}");
        match event {
            Input(PressLeft) => {
                if self.state == Running && self.try_move((-1, 0)) {
                    sound.play(Action::Move).ok();
                }
                self.started_left = Some(time);
                self.clear_timer(DasRight);
                self.clear_timer(Arr);
                self.set_timer(DasLeft);
            }
            Input(PressRight) => {
                if self.state == Running && self.try_move((1, 0)) {
                    sound.play(Action::Move).ok();
                }
                self.started_right = Some(time);
                self.clear_timer(DasLeft);
                self.clear_timer(Arr);
                self.set_timer(DasRight);
            }
            Input(ReleaseLeft) => {
                self.clear_timer(DasLeft);
                if match (self.started_left, self.started_right) {
                    (None, _) => false, // only reachable by holding it down between games
                    (Some(_), None) => true,
                    (Some(l), Some(r)) => l > r,
                } {
                    self.clear_timer(Arr);
                }
                self.started_left = None;
                self.handle_das();
            }
            Input(ReleaseRight) => {
                self.clear_timer(DasRight);
                if match (self.started_left, self.started_right) {
                    (_, None) => false, // only reachable by holding it down between games
                    (None, Some(_)) => true,
                    (Some(l), Some(r)) => l < r,
                } {
                    self.clear_timer(Arr);
                }
                self.started_right = None;
                self.handle_das();
            }
            Input(Hold) => {
                if self.can_hold {
                    if !self.hold() {
                        sound.play(Meta::Lose).ok();
                        self.state = Done;
                        self.end_time = Some(self.time);
                        self.timers.clear();
                    } else {
                        sound.play(Action::Hold).ok();
                        self.can_hold = false;
                    }
                } else {
                    sound.play(Action::NoHold).ok();
                }
            }
            Input(Undo) => {
                if !self.mode.allows_undo() {
                    return false;
                }
                let Some(prev) = self.history.pop_back() else {
                    return false;
                };
                self.board = prev.board;
                assert!(
                    self.spawn(prev.current.piece),
                    "shouldn't be invalid since that piece was able to be placed"
                );
                self.hold = prev.hold;
                self.upcomming = prev.upcomming;
                self.pieces = prev.pieces_placed;
                self.spins = prev.spins;
                if let Mode::TrainingLab { lookahead: Some(lookahead), .. } = &mut self.mode {
                    lookahead.board_visible = true;
                    lookahead.next_piece_goal = self.pieces + lookahead.min_placements;
                }
            }
            Input(Hard) | Timer(Lock | Extended | Timeout) => {
                self.hard_drop(sound);
                // at 536 bytes per Moment, we store 200 moves (107.2kB) max
                if self.history.len() > 200 {
                    self.history.pop_front();
                }
                return true;
            }
            Timer(t @ (SoftDrop | Gravity)) => {
                if self.state == Running {
                    if self.config.soft_drop == 0 && t == SoftDrop {
                        while self.try_drop() {}
                    } else {
                        self.try_drop();
                    }
                }
                self.set_timer(t);
            }
            Timer(Start) => {
                self.state = Running;
                let next = self.pop_piece();
                self.spawn(next);
                self.start_time = Some(time);
                sound.play(Meta::Go).ok();
            }
            Input(rot @ (Cw | Ccw | Flip)) => {
                if self.try_rotate(rot.try_into().expect("should always be a rotation")) {
                    sound.play(Action::Rotate).ok();
                }
                // confirmed: jstris resets it even if you don't successfully rotate
                self.clear_timer(Lock);
                self.clear_timer(Extended);
            }
            Input(PressSoft) => {
                self.soft_dropping = true;
                self.clear_timer(Gravity);
                self.set_timer(SoftDrop);
            }
            Input(ReleaseSoft) => {
                self.soft_dropping = false;
                self.clear_timer(SoftDrop);
                self.set_timer(Gravity);
            }
            Input(Restart | Quit) => unreachable!("should be handled in outer event loop"),

            // TODO: add das sound effect
            Input(ShowSolution(ind)) => match ind {
                0 => {
                    self.solution = None;
                }
                ind => {
                    let suggestion = &Game::spin_shortlist(&self.spins)[ind as usize - 1];
                    let mut game = self.clone();
                    game.config.ghost = false;
                    for (m, _) in &suggestion.moves {
                        info!("{:?} x:{} y:{} {:?} spin:{}", m.piece, m.x, m.y, m.rotation, m.spun);
                    }
                    self.solution = Some((suggestion.clone(), Box::new(game)));
                }
            },
            Timer(DasLeft | DasRight) => {
                if self.state == Running {
                    self.handle_das()
                }
            }
            Timer(Arr) => {
                todo!()
            }
            Timer(Are) => {
                todo!()
            }
            Timer(Lookahead) => {
                if let Mode::TrainingLab { lookahead: Some(lookahead), .. } = &mut self.mode {
                    lookahead.board_visible = true;
                    lookahead.next_piece_goal = self.pieces + lookahead.min_placements;
                }
            }
        };
        // TODO: set lock timers if on the ground and they arent already set
        false
    }

    pub fn ghost_pos(&self) -> PieceLocation {
        let current_pos = self.current.blocks();
        let mut ghost = current_pos;
        let mut y = self.current.pos.1;
        loop {
            let next = ghost.map(|(x, y)| (x, y - 1));
            if !self.check_valid(next) {
                break;
            }
            y -= 1;
            ghost = next;
        }
        let pos = (self.current.pos.0, y);
        let rot = self.current.rot;
        let piece = self.current.piece;
        PieceLocation { piece, pos, rot }
    }

    fn set_timer(&mut self, t: TimerEvent) {
        use TimerEvent::*;
        let c = self.config;
        let frames = match t {
            DasLeft | DasRight => c.das,
            Arr => c.arr,
            SoftDrop => c.soft_drop,
            Gravity => {
                let Some(gravity) = c.gravity else { return };
                gravity
            }
            Lock => c.lock_delay.0,
            Extended => c.lock_delay.1,
            Timeout => {
                if c.gravity.is_none() {
                    return;
                };
                c.lock_delay.2
            }
            Start => 60,
            Are => todo!(),
            Lookahead => self.mode.lookahead_timeout(),
        };
        let time = self.time + FRAME * frames as u32;
        let idx = self.timers.partition_point(|&(i, _)| i < time);
        self.timers.insert(idx, (time, t))
    }

    fn clear_timer(&mut self, t: TimerEvent) {
        self.timers.retain(|&(_, ev)| ev != t)
    }

    fn push_moment(&mut self) {
        let moment = Moment {
            board: self.board,
            current: self.current,
            hold: self.hold,
            upcomming: self.upcomming.clone(),
            spins: self.spins.clone(),
            pieces_placed: self.pieces,
        };
        self.history.push_back(moment);
    }

    fn hard_drop(&mut self, sound: &SoundPlayer<impl Sink>) {
        while self.try_drop() {}
        let old_lines = self.lines;
        self.push_moment();
        // TODO: redo with "piece placement result struct"
        if self.lock() {
            // TODO: maybe just play both at the same time?
            let effect: Sound = match (self.lines - old_lines, self.spin) {
                (0, _) => Action::Lock.into(), // TODO: differentiate lock/harddrop
                (1, false) => Clear::Single.into(),
                (1, true) => Clear::Tspin.into(),
                (2, false) => Clear::Double.into(),
                (2, true) => Clear::TspinDouble.into(),
                (3, false) => Clear::Triple.into(),
                (3, true) => Clear::TSpinTriple.into(),
                (4, _) => Clear::Quad.into(),
                _ => unreachable!("impossible line clear"),
            };
            sound.play(effect).ok();
            if self.mode.is_complete(self.lines) {
                sound.play(Meta::Win).or_else(|_| sound.play(Clear::Single)).ok();
                self.finish();
            }
        } else {
            sound.play(Meta::Lose).ok();
            self.finish();
        }
    }

    fn finish(&mut self) {
        self.state = GameState::Done;
        self.end_time = Some(self.time);
        self.timers.clear();
    }

    fn fill_bag(&mut self) -> &mut Self {
        use Piece::*;
        let mut pieces = [I, J, L, O, S, T, Z];
        pieces.shuffle(&mut self.rng);
        self.upcomming.extend(pieces);
        self
    }

    pub fn check_valid(&self, pos: Pos) -> bool {
        pos.into_iter().all(|(x, y)| {
            (0..10).contains(&x)
                && (0..30).contains(&y)
                && self.board[y as usize][x as usize] == Cell::Empty
        })
    }

    pub fn lock(&mut self) -> bool {
        info!("pos: {:?}", self.current.pos);
        for (x, y) in self.current.blocks() {
            info!("{x} {y}");
            self.board[y as usize][x as usize] = Cell::Piece(self.current.piece);
        }
        for i in (0..23).rev() {
            if self.board[i].iter().all(|c| matches!(c, Cell::Piece(_))) {
                for j in i..22 {
                    self.board[j] = self.board[j + 1];
                }
                self.lines += 1;
            }
        }
        let next = self.pop_piece();
        self.pieces += 1;
        self.spawn(next)
    }

    fn pop_piece(&mut self) -> Piece {
        let next = self.upcomming.dequeue().unwrap();
        if self.upcomming.len() < 7 {
            self.fill_bag();
        }
        next
    }

    fn das_helper(&mut self, dir: i8) {
        if self.config.arr == 0 {
            while self.try_move((dir, 0)) {}
        } else {
            self.set_timer(TimerEvent::Arr)
        }
    }

    // TODO: return bool for whether it moved to trigger sound
    fn handle_das(&mut self) {
        let t = self.time;
        let threshold = FRAME * self.config.das as u32;
        match (self.started_left, self.started_right) {
            (Some(l), Some(r)) => {
                if r < l && t - l > threshold {
                    self.das_helper(-1)
                } else if l < r && t - r > FRAME * self.config.das as u32 {
                    self.das_helper(1)
                }
            }
            (None, Some(r)) if t - r > threshold => self.das_helper(1),
            (Some(l), None) if t - l > threshold => self.das_helper(-1),
            _ => {}
        }
    }

    fn spawn(&mut self, next: Piece) -> bool {
        {
            use TimerEvent::*;
            self.clear_timer(SoftDrop);
            self.clear_timer(Gravity);
            self.clear_timer(Lock);
            self.clear_timer(Extended);
            self.clear_timer(Timeout);
        }
        self.can_hold = true;
        let pos = (4, 21);
        let rotation = Rotation::North;
        let next = PieceLocation::new(next, pos, rotation);
        if !self.check_valid(next.blocks()) {
            return false;
        }
        self.current = next;
        self.try_drop();
        self.set_timer(if self.soft_dropping { TimerEvent::SoftDrop } else { TimerEvent::Gravity });
        self.set_timer(TimerEvent::Timeout);
        true
    }

    pub fn hold(&mut self) -> bool {
        self.push_moment();
        let piece = if let Some(p) = self.hold {
            self.hold = Some(self.current.piece);
            p
        } else {
            self.hold = Some(self.current.piece);
            self.pop_piece()
        };
        self.spawn(piece)
    }

    fn try_rotate(&mut self, dir: Spin) -> bool {
        let PieceLocation { piece, pos, rot } = self.current;
        let new_rot = rot.rotate(dir);
        let new_current = PieceLocation::new(piece, pos, new_rot);
        let new_pos = new_current.blocks();
        let kicks = piece.get_your_kicks(rot, dir);
        for (dx, dy) in kicks {
            let displaced = new_pos.map(|(x, y)| (x + dx, y + dy));
            if self.check_valid(displaced) {
                self.current =
                    PieceLocation::new(self.current.piece, (pos.0 + dx, pos.1 + dy), new_rot);
                self.handle_das();
                use TimerEvent::*;
                if self.can_drop() {
                    self.clear_timer(Lock);
                    self.clear_timer(Extended);
                } else {
                    self.set_timer(Lock);
                    self.set_timer(Extended);
                }
                return true;
            }
        }
        if piece == Piece::I {
            info!("pos: {pos:?}");
            info!("kicks: {kicks:?}");
        }
        false
    }

    fn try_drop(&mut self) -> bool {
        self.try_move((0, -1))
            .then(|| {
                self.handle_das();
            })
            .is_some()
    }

    fn can_drop(&self) -> bool {
        self.check_valid(self.current.blocks().map(|(x, y)| (x, y - 1)))
    }

    fn try_move(&mut self, (dx, dy): (i8, i8)) -> bool {
        let mut next_current = self.current;
        next_current.pos.0 += dx;
        next_current.pos.1 += dy;

        if self.check_valid(next_current.blocks()) {
            self.current = next_current;
            use TimerEvent::*;
            self.clear_timer(Lock);
            if self.can_drop() {
                self.clear_timer(Extended);
            } else {
                self.set_timer(Lock);
                self.set_timer(Extended);
            }
            true
        } else {
            false
        }
    }
}
