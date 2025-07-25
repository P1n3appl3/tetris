use std::{collections::VecDeque, time::Instant};

use tetrizz::beam_search::Node;

use crate::*;

#[derive(Clone)]
pub struct Game {
    pub board: [[Cell; 10]; 50], // hope no one stacks higher than this 👀
    pub upcomming: VecDeque<Piece>,
    pub current: PieceLocation,
    pub hold: Option<Piece>,
    pub lines: u16,
    pub target_lines: Option<u16>,
    pub config: Config,
    pub timers: VecDeque<(Instant, TimerEvent)>,
    pub time: Instant,
    pub started_right: Option<Instant>,
    pub started_left: Option<Instant>,
    pub start_time: Option<Instant>,
    pub end_time: Option<Instant>,
    pub soft_dropping: bool,
    pub can_hold: bool,
    pub state: GameState,
    pub rng: StdRng,
    pub spins: Vec<Node>,
    pub solution: Option<(Node, Box<Game>)>,
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
        let mut board = tetrizz::data::Board {
            cols: [tetrizz::data::Column(0); 10],
        };
        for column in 0..10 {
            for row in 0..50 {
                if self.board[row][column] != Cell::Empty {
                    board.cols[column].0 |= 1 << row;
                }
            }
        }
        board
    }
}

impl Game {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            rng: StdRng::from_os_rng(),
            board: [[Cell::Empty; 10]; 50],
            upcomming: Default::default(),
            current: PieceLocation::new(Piece::I, (0, 0), Rotation::North),
            hold: None,
            lines: 0,
            target_lines: Some(40),
            timers: Default::default(),
            started_right: None,
            started_left: None,
            time: Instant::now(),
            start_time: None,
            end_time: None,
            soft_dropping: false,
            can_hold: true,
            state: GameState::Done,
            spins: Default::default(),
            solution: None,
        }
    }

    pub fn start(&mut self, seed: u64, player: &impl Sound) {
        self.state = GameState::Startup;
        self.board = [[Cell::Empty; 10]; 50];
        self.hold = None;
        self.lines = 0;
        self.upcomming.clear();
        self.rng = StdRng::seed_from_u64(seed);
        self.fill_bag();
        self.time = Instant::now();
        while let Some(Piece::Z | Piece::S) = self.upcomming.front() {
            self.pop_piece();
        }
        player.play("start").ok();
        // TODO: combine "ready" and "go" sounds
        // TODO: make startup time configurable
        self.timers.clear();
        self.set_timer(TimerEvent::Start);
    }

    pub fn handle(&mut self, event: Event, time: Instant, player: &impl Sound) -> bool {
        use {Event::*, GameState::*, InputEvent::*, TimerEvent::*};
        self.time = time;
        match event {
            Input(PressLeft) => {
                if self.state == Running && self.try_move((-1, 0)) {
                    player.play("move").ok();
                }
                self.started_left = Some(time);
                self.clear_timer(DasRight);
                self.clear_timer(Arr);
                self.set_timer(DasLeft);
            }
            Input(PressRight) => {
                if self.state == Running && self.try_move((1, 0)) {
                    player.play("move").ok();
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
                        player.play("lose").ok();
                        self.state = Done;
                        self.end_time = Some(self.time);
                        self.timers.clear();
                    } else {
                        player.play("hold").ok();
                        // self.can_hold = false;
                    }
                } else {
                    // TODO: add failed hold sound
                    player.play("nohold").ok();
                }
            }
            Input(Hard) | Timer(Lock | Extended | Timeout) => {
                self.hard_drop(player);
                return true;
            }
            Timer(t @ (SoftDrop | Gravity)) => {
                if self.state == Running {
                    self.try_drop();
                }
                self.set_timer(t);
            }
            Timer(Start) => {
                self.state = Running;
                let next = self.pop_piece();
                self.spawn(next);
                self.start_time = Some(time);
            }
            Input(rot @ (Cw | Ccw | Flip)) => {
                if self.try_rotate(rot.try_into().expect("should always be a rotation")) {
                    player.play("rotate").ok();
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
            // Input(Undo | Redo) => {
            //     // this might make using instants impossible... if you undo you'd want to
            //     // also roll back the current time
            //     unreachable!("should be handled in outer event loop")
            // }

            // TODO: add das sound effect
            Input(ShowSolution(ind)) => match ind {
                0 => {
                    self.solution = None;
                }
                ind => {
                    let suggestion = &self.spins[ind as usize - 1];
                    let mut game = self.clone();
                    game.config.ghost = false;
                    for (m, _) in &suggestion.moves {
                        log::info!(
                            "{:?} x:{} y:{} {:?} spin:{}",
                            m.piece,
                            m.x,
                            m.y,
                            m.rotation,
                            m.spun
                        );
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
        };
        // TODO: set lock timers if on the ground and they arent already set
        false
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
            Start => 120,
            Are => todo!(),
        };
        let time = self.time + FRAME * frames as u32;
        let idx = self.timers.partition_point(|&(i, _)| i < time);
        self.timers.insert(idx, (time, t))
    }

    fn clear_timer(&mut self, t: TimerEvent) {
        self.timers.retain(|&(_, ev)| ev != t)
    }

    fn hard_drop(&mut self, player: &impl Sound) {
        while self.try_drop() {}
        let old_lines = self.lines;
        // TODO: redo with "piece placement result struct"
        if self.lock() {
            if self.lines == old_lines {
                player.play("lock").ok();
            } else if self.lines < self.target_lines.unwrap_or(u16::MAX) {
                player.play("line").ok();
            } else {
                // TODO: maybe just play both at the same time?
                player.play("win").or_else(|_| player.play("lock")).ok();
                self.finish();
            }
        } else {
            player.play("lose").ok();
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
        log::info!("pos: {:?}", self.current.pos);
        for (x, y) in self.current.blocks() {
            log::info!("{} {}", x, y);
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
        self.spawn(next)
    }

    fn pop_piece(&mut self) -> Piece {
        let next = self.upcomming.pop_front().unwrap();
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
        self.set_timer(if self.soft_dropping {
            TimerEvent::SoftDrop
        } else {
            TimerEvent::Gravity
        });
        self.set_timer(TimerEvent::Timeout);
        true
    }

    pub fn hold(&mut self) -> bool {
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
        for (dx, dy) in piece.get_your_kicks(rot, dir) {
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
        let mut next_current = self.current.clone();
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
