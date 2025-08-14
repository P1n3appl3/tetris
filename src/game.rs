use std::collections::VecDeque;

use log::debug;
use ringbuffer::{ConstGenericRingBuffer, RingBuffer};
use web_time::{Instant, SystemTime};

use crate::{
    sound::{Sink, SoundPlayer},
    *,
};

pub type Board = [[Cell; 10]; 50]; // hope no one stacks higher than this 👀

#[derive(Clone)]
pub enum Mode {
    Sprint { target_lines: u16 },
    // Cheese { target_lines: u16 },
    Practice,
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
            Mode::Practice {} => true,
        }
    }
}

#[derive(Clone)]
pub struct Moment {
    pub board: [[Cell; 10]; 50],
    pub current: Piece,
    pub hold: Option<Piece>,
    pub upcomming: ConstGenericRingBuffer<Piece, 14>,
}

#[derive(Clone)]
pub struct Game {
    pub board: Board,
    pub upcomming: ConstGenericRingBuffer<Piece, 14>,
    pub current: (Piece, (i8, i8), Rotation),
    pub hold: Option<Piece>,
    pub lines: u16,
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
    pub state: GameState,
    pub rng: StdRng,
    pub history: VecDeque<Moment>,
}

impl Game {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            rng: StdRng::from_os_rng(),
            board: [[Cell::Empty; 10]; 50],
            upcomming: Default::default(),
            current: (Piece::I, (3, 21), Rotation::North),
            hold: None,
            lines: 0,
            mode: Mode::Sprint { target_lines: 40 },
            timers: Default::default(),
            started_right: None,
            started_left: None,
            time: Instant::now(),
            start_time: None,
            end_time: None,
            soft_dropping: false,
            can_hold: true,
            state: GameState::Done,
            history: VecDeque::new(),
        }
    }

    pub fn start(&mut self, seed: Option<u64>, sound: &SoundPlayer<impl Sink>) {
        self.state = GameState::Startup;
        self.board = [[Cell::Empty; 10]; 50];
        self.hold = None;
        self.lines = 0;
        self.upcomming.clear();
        self.rng = StdRng::seed_from_u64(seed.unwrap_or_else(|| {
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as u64
        }));
        self.fill_bag();
        self.time = Instant::now();
        self.start_time = None;
        if matches!(self.mode, Mode::Sprint { .. }) {
            while let Some(Piece::Z | Piece::S) = self.upcomming.front() {
                self.pop_piece();
            }
        }
        sound.play(sound::Meta::Go).ok();
        // TODO: combine "ready" and "go" sounds
        // TODO: make startup time configurable or maybe even based on the sound length?
        self.timers.clear();
        self.set_timer(TimerEvent::Start);
    }

    pub fn handle(&mut self, event: Event, time: Instant, sound: &SoundPlayer<impl Sink>) {
        use {Event::*, GameState::*, InputEvent::*, TimerEvent::*};
        self.time = time;
        debug!("handling event: {event:?}");
        match event {
            Input(PressLeft) => {
                if self.state == Running && self.try_move((-1, 0)) {
                    sound.play(sound::Action::Move).ok();
                }
                self.started_left = Some(time);
                self.clear_timer(DasRight);
                self.clear_timer(Arr);
                self.set_timer(DasLeft);
            }
            Input(PressRight) => {
                if self.state == Running && self.try_move((1, 0)) {
                    sound.play(sound::Action::Move).ok();
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
                        sound.play(sound::Meta::Lose).ok();
                        self.state = Done;
                        self.end_time = Some(self.time);
                        self.timers.clear();
                    } else {
                        sound.play(sound::Action::Hold).ok();
                        self.can_hold = false;
                    }
                } else {
                    sound.play(sound::Action::NoHold).ok();
                }
            }
            Input(Undo) => {
                if !self.mode.allows_undo() {
                    return;
                }
                let Some(prev) = self.history.pop_back() else {
                    return;
                };
                self.board = prev.board;
                debug_assert!(
                    self.spawn(prev.current),
                    "shouldn't be invalid since that piece was able to be placed"
                );
                self.hold = prev.hold;
                self.upcomming = prev.upcomming;
            }
            Input(Hard) | Timer(Lock | Extended | Timeout) => {
                let moment = Moment {
                    board: self.board,
                    current: self.current.0,
                    hold: self.hold,
                    upcomming: self.upcomming.clone(),
                };
                // at 536 bytes per Moment, we store 200 moves (107.2kB) max
                if self.history.len() > 200 {
                    self.history.pop_front();
                }
                self.history.push_back(moment);
                self.hard_drop(sound)
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
                    sound.play(sound::Action::Rotate).ok();
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
    }

    pub fn ghost_pos(&self) -> (i8, i8) {
        let (piece, pos, rot) = self.current;
        let current_pos = piece.get_pos(rot, pos);
        let mut ghost = current_pos;
        let mut y = pos.1;
        loop {
            let next = ghost.map(|(x, y)| (x, y - 1));
            if !self.check_valid(next) {
                break;
            }
            y -= 1;
            ghost = next;
        }
        (pos.0, y)
    }

    fn set_timer(&mut self, t: TimerEvent) {
        use TimerEvent::*;
        let c = self.config;
        let frames = match t {
            DasLeft | DasRight => c.das,
            Arr => c.arr,
            SoftDrop => c.soft_drop,
            Gravity => c.gravity,
            Lock => c.lock_delay.0,
            Extended => c.lock_delay.1,
            Timeout => c.lock_delay.2,
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

    fn hard_drop(&mut self, sound: &SoundPlayer<impl Sink>) {
        while self.try_drop() {}
        let old_lines = self.lines;
        // TODO: redo with "piece placement result struct"
        if self.lock() {
            if self.lines == old_lines {
                sound.play(sound::Action::Lock).ok();
            } else if !self.mode.is_complete(self.lines) {
                // TODO: only output this sound when pressing harddrop
                sound.play(sound::Action::HardDrop).ok();
            } else {
                // TODO: maybe just play both at the same time?
                sound.play(sound::Meta::Win).or_else(|_| sound.play(sound::Clear::Single)).ok();
                self.finish();
            }
        } else {
            sound.play(sound::Meta::Lose).ok();
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

    fn check_valid(&self, pos: Pos) -> bool {
        pos.into_iter().all(|(x, y)| {
            (0..10).contains(&x)
                && (0..30).contains(&y)
                && self.board[y as usize][x as usize] == Cell::Empty
        })
    }

    fn lock(&mut self) -> bool {
        let (p, pos, rot) = self.current;
        for (x, y) in p.get_pos(rot, pos) {
            self.board[y as usize][x as usize] = Cell::Piece(p);
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
        let pos = (3, 21);
        let rot = Rotation::North;
        if !self.check_valid(next.get_pos(rot, pos)) {
            return false;
        }
        self.current = (next, (3, 21), Rotation::North);
        self.try_drop();
        self.set_timer(if self.soft_dropping { TimerEvent::SoftDrop } else { TimerEvent::Gravity });
        self.set_timer(TimerEvent::Timeout);
        true
    }

    fn hold(&mut self) -> bool {
        let piece = if let Some(p) = self.hold {
            self.hold = Some(self.current.0);
            p
        } else {
            self.hold = Some(self.current.0);
            self.pop_piece()
        };
        self.spawn(piece)
    }

    fn try_rotate(&mut self, dir: Spin) -> bool {
        let (piece, pos, rot) = self.current;
        let new_rot = rot.rotate(dir);
        let new_pos = piece.get_pos(new_rot, pos);
        for (dx, dy) in piece.get_your_kicks(rot, dir) {
            let displaced = new_pos.map(|(x, y)| (x + dx, y + dy));
            if self.check_valid(displaced) {
                self.current = (self.current.0, (pos.0 + dx, pos.1 + dy), new_rot);
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
        let (piece, pos, rot) = self.current;
        self.check_valid(piece.get_pos(rot, pos).map(|(x, y)| (x, y - 1)))
    }

    fn try_move(&mut self, (dx, dy): (i8, i8)) -> bool {
        let (piece, (x, y), rot) = self.current;
        let pos = (x + dx, y + dy);
        if self.check_valid(piece.get_pos(rot, pos)) {
            self.current = (piece, pos, rot);
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
