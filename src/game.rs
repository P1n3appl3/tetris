use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use rand::prelude::*;
use serde::{Deserialize, Serialize};

use crate::sound::Player;

pub type Pos = [(i8, i8); 4];

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Piece {
    I,
    J,
    L,
    O,
    S,
    T,
    Z,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i8)]
pub enum Direction {
    Left = -1,
    Right = 1,
}

impl Direction {
    fn reverse(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Spin {
    Cw,
    Ccw,
    Flip,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Rotation {
    #[default]
    North,
    East,
    South,
    West,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputEvent {
    PressDir(Direction),
    ReleaseDir(Direction),
    PressSoft,
    ReleaseSoft,
    Rotate(Spin),
    Hard,
    Hold,
    // maybe pull these out along with Garbage/Pause/StartSound to a "misc" event
    Restart,
    Quit,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TimerEvent {
    Das(Direction),
    Arr,
    SoftDrop,
    Gravity,
    Lock,
    Extended,
    Timeout,
    Start,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Timer(TimerEvent),
    Input(InputEvent),
}

const FRAME: Duration = Duration::from_nanos(16_666_667);

// TODO: make all these floats (maybe ms instead of frames?)
// TODO: find jstris softdrop delays and match them
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub das: u16,
    pub arr: u16,
    pub gravity: u16,
    pub soft_drop: u16,
    pub lock_delay: (u16, u16, u16),
    pub ghost: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Startup,
    Running,
    Done,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Cell {
    Piece(Piece),
    Garbage,
    #[default]
    Empty,
}

#[derive(Clone)]
pub struct Game {
    pub board: [[Cell; 10]; 30], // hope no one stacks higher than this 👀
    pub upcomming: VecDeque<Piece>,
    pub current: (Piece, (i8, i8), Rotation),
    pub hold: Option<Piece>,
    pub lines: u16,
    pub config: Config,
    pub timers: VecDeque<(Instant, TimerEvent)>,
    pub started_right: Option<Instant>,
    pub started_left: Option<Instant>,
    pub start_time: Instant,
    pub current_time: Instant,
    pub soft_dropping: bool,
    pub can_hold: bool,
    pub state: GameState,
    pub start_time: Option<Instant>,
    pub end_time: Option<Instant>,
    pub last_update: Option<Instant>,
    pub rng: StdRng,
}

impl Rotation {
    const fn rotate(self, dir: Spin) -> Self {
        use {Rotation::*, Spin::*};
        match (self, dir) {
            (North, Ccw) => West,
            (North, Cw) => East,
            (North, Flip) => South,
            (East, Ccw) => North,
            (East, Cw) => South,
            (East, Flip) => West,
            (South, Ccw) => East,
            (South, Cw) => West,
            (South, Flip) => North,
            (West, Ccw) => South,
            (West, Cw) => North,
            (West, Flip) => East,
        }
    }
}

impl Piece {
    pub fn get_pos(self, r: Rotation, (x, y): (i8, i8)) -> Pos {
        DATA[self as usize][r as usize].map(|(a, b)| (x + a, y - b))
    }

    fn get_your_kicks(self, rot: Rotation, dir: Spin) -> [(i8, i8); 5] {
        let next_rot = rot.rotate(dir);
        use {Piece::*, Rotation::*};
        let idx = match (rot, next_rot) {
            (North, East) => 0,
            (East, North) => 1,
            (East, South) => 2,
            (South, East) => 3,
            (South, West) => 4,
            (West, South) => 5,
            (West, North) => 6,
            (North, West) => 7,
            (North, South) => 8,
            (South, North) => 9,
            (East, West) => 10,
            (West, East) => 11,
            (a, b) => unreachable!("invalid rotation: {a:?} -> {b:?}"),
        };
        match self {
            I => ROTI[idx],
            O => Default::default(),
            _ => ROTJLSTZ[idx],
        }
    }
}

impl Game {
    pub fn new(config: Config) -> Self {
        let t = Instant::now();
        Self {
            config,
            rng: StdRng::from_entropy(),
            board: Default::default(),
            upcomming: Default::default(),
            current: (Piece::I, (0, 0), Rotation::North),
            hold: None,
            lines: 0,
            timers: Default::default(),
            started_left: None,
            started_right: None,
            start_time: None,
            end_time: None,
            last_update: None,
            start_time: t,
            current_time: t,
            soft_dropping: false,
            can_hold: true,
            state: GameState::Done,
        }
    }

    pub fn start(&mut self, seed: u64, player: &Player) {
        self.state = GameState::Startup;
        self.board = Default::default();
        self.hold = None;
        self.last_update = Some(Instant::now());
        self.lines = 0;
        self.upcomming.clear();
        self.rng = StdRng::seed_from_u64(seed);
        self.fill_bag();
        while let Some(Piece::Z | Piece::S) = self.upcomming.front() {
            self.pop_piece();
        }
        player.play("ready").ok();
        // TODO: combine "ready" and "go" sounds
        // TODO: make startup time configurable
        self.set_timer(TimerEvent::Start, Instant::now(), 120);
    }

    pub fn handle_event(&mut self, event: Event, time: Instant, player: &Player) {
        self.current_time = time;
        use {Event::*, GameState::*, InputEvent::*, TimerEvent::*};
        match event {
            Input(PressDir(dir)) => {
                self.last_dir = dir;
                self.set_timer(Das(dir));
                if self.dasing {
                    self.das_charged = true;
                }
                if self.state == Running && self.try_shift(dir) {
                    player.play("move").ok();
                    self.clear_timer(Lock);
                }
                self.started_left = Some(time);
                self.clear_timer(DasRight);
                self.clear_timer(Arr);
                self.set_timer(DasLeft, time, self.config.das as u32);
            }
            Input(PressRight) => {
                if self.try_move((1, 0)) {
                    player.play("move").ok();
                    self.clear_timer(Lock);
                }
                self.started_right = Some(time);
                self.clear_timer(DasLeft);
                self.clear_timer(Arr);
                self.set_timer(DasRight, time, self.config.das as u32);
            }
            Input(ReleaseLeft) => {
                self.clear_timer(DasLeft);
                // TODO: clear ARR if we were going left
            }
            Input(ReleaseRight) => self.clear_timer(DasRight),
            Input(Hold) => {
                if self.can_hold {
                    if !self.hold() {
                        player.play("lose").ok();
                        self.state = GameState::Done;
                        self.timers.clear();
                    } else {
                        player.play("hold").ok();
                        self.can_hold = false;
                    }
                } else {
                    // TODO: add failed hold sound
                    // player.play("nohold");
                }
            }
            Input(Hard) | Timer(Lock | Extended | Timeout) => self.hard_drop(player),
            Timer(t @ (SoftDrop | Gravity)) => {
                self.try_drop();
                self.set_timer(t);
            }
            Timer(StartSound) => {
                player.play("go").ok();
                self.set_timer(Start, time, 60);
            }
            Timer(Start) => {
                self.start_time = self.current_time;
                self.state = GameState::Running;
                let next = self.pop_piece();
                self.spawn(next);
                self.start_time = Some(time);
            }
            Input(Restart | Quit) => unreachable!("should be handled in outer event loop"),
        };
        self.last_update = Some(time);
        // TODO: set lock timers if on the ground and they arent already set
    }

    fn set_timer(&mut self, t: TimerEvent, time: Instant, frames: u32) {
        let time = time + FRAME * frames;
        let idx = self.timers.partition_point(|&(i, _)| i < time);
        self.timers.insert(idx, (time, t))
    }

    fn clear_timer(&mut self, t: TimerEvent) {
        self.timers.retain(|&(_, ev)| ev != t)
    }

    fn hard_drop(&mut self, player: &Player) {
        while self.try_drop() {}
        let old_lines = self.lines;
        if self.lock() {
            match self.lines {
                n if n == old_lines => {
                    player.play("lock").ok();
                }
                40.. => {
                    // TODO: maybe just play both at the same time?
                    player.play("win").or_else(|_| player.play("lock")).ok();
                    self.state = GameState::Done;
                    Some(())
                }
            };
        } else {
            player.play("lose").ok();
            self.state = GameState::Lost;
        }
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
        let next = self.upcomming.pop_front().unwrap();
        if self.upcomming.len() < 7 {
            self.fill_bag();
        }
        next
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
        self.set_timer(if self.soft_dropping { SoftDrop } else { Gravity });
        self.set_timer(Timeout);
        if self.dasing && self.config.arr == 0 {
            while self.try_shift(self.last_dir) {}
        }
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
                self.current.1 = (pos.0 + dx, pos.1 + dy);
                self.current.2 = new_rot;
                if self.dasing && self.config.arr == 0 {
                    while self.try_shift(self.last_dir) {}
                }
                return true;
            }
        }
        false
    }

    fn try_shift(&mut self, dir: Direction) -> bool {
        use {Direction::*, TimerEvent::*};
        let dx = match dir {
            Left => -1,
            Right => 1,
        };
        self.try_move((dx, 0))
            .then(|| {
                if self.can_drop() {
                    self.clear_timer(Lock);
                } else if !self.timers.iter().any(|(_, ev)| *ev == Lock) {
                    self.set_timer(Lock);
                }
            })
            .is_some()
    }

    fn try_drop(&mut self) -> bool {
        use TimerEvent::*;
        self.try_move((0, -1))
            .then(|| {
                if self.can_drop() {
                    self.clear_timer(Lock);
                    self.clear_timer(Extended);
                } else {
                    self.set_timer(Lock);
                    self.set_timer(Extended);
                }
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
            true
        } else {
            false
        }
    }
}

// ordered n, e, s, w
const DATA: [[Pos; 4]; 7] = [
    [
        [(0, 1), (1, 1), (2, 1), (3, 1)], // I
        [(2, 0), (2, 1), (2, 2), (2, 3)],
        [(0, 2), (1, 2), (2, 2), (3, 2)],
        [(1, 0), (1, 1), (1, 2), (1, 3)],
    ],
    [
        [(0, 0), (0, 1), (1, 1), (2, 1)], // J
        [(1, 0), (2, 0), (1, 1), (1, 2)],
        [(0, 1), (1, 1), (2, 1), (2, 2)],
        [(1, 0), (1, 1), (0, 2), (1, 2)],
    ],
    [
        [(2, 0), (0, 1), (1, 1), (2, 1)], // L
        [(1, 0), (1, 1), (1, 2), (2, 2)],
        [(0, 1), (1, 1), (2, 1), (0, 2)],
        [(0, 0), (1, 0), (1, 1), (1, 2)],
    ],
    [
        [(1, 0), (2, 0), (1, 1), (2, 1)], // O
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
    ],
    [
        [(1, 0), (2, 0), (0, 1), (1, 1)], // S
        [(1, 0), (1, 1), (2, 1), (2, 2)],
        [(1, 1), (2, 1), (0, 2), (1, 2)],
        [(0, 0), (0, 1), (1, 1), (1, 2)],
    ],
    [
        [(1, 0), (0, 1), (1, 1), (2, 1)], // T
        [(1, 0), (1, 1), (2, 1), (1, 2)],
        [(0, 1), (1, 1), (2, 1), (1, 2)],
        [(1, 0), (0, 1), (1, 1), (1, 2)],
    ],
    [
        [(0, 0), (1, 0), (1, 1), (2, 1)], // Z
        [(2, 0), (1, 1), (2, 1), (1, 2)],
        [(0, 1), (1, 1), (1, 2), (2, 2)],
        [(1, 0), (0, 1), (1, 1), (0, 2)],
    ],
];

const ROTI: [[(i8, i8); 5]; 12] = [
    [(0, 0), (-2, 0), (1, 0), (-2, -1), (1, 2)], // n -> e
    [(0, 0), (2, 0), (-1, 0), (2, 1), (-1, -2)], // e -> n
    [(0, 0), (-1, 0), (2, 0), (-1, 2), (2, -1)], // e -> s
    [(0, 0), (1, 0), (-2, 0), (1, -2), (-2, 1)], // s -> e
    [(0, 0), (2, 0), (-1, 0), (2, 1), (-1, -2)], // s -> w
    [(0, 0), (-2, 0), (1, 0), (-2, -1), (1, 2)], // w -> s
    [(0, 0), (1, 0), (-2, 0), (1, -2), (-2, 1)], // w -> n
    [(0, 0), (-1, 0), (2, 0), (-1, 2), (2, -1)], // n -> w
    [(0, 0), (0, 1), (0, 0), (0, 0), (0, 0)],    // n -> s
    [(0, 0), (0, -1), (0, 0), (0, 0), (0, 0)],   // s -> n
    [(0, 0), (1, 0), (0, 0), (0, 0), (0, 0)],    // e -> w
    [(0, 0), (-1, 0), (0, 0), (0, 0), (0, 0)],   // w -> e
];

const ROTJLSTZ: [[(i8, i8); 5]; 12] = [
    [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)], // n -> e
    [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],     // e -> n
    [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],     // e -> s
    [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)], // s -> e
    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],    // s -> w
    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],  // w -> s
    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],  // w -> n
    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],    // n -> w
    [(0, 0), (0, 1), (0, 0), (0, 0), (0, 0)],      // n -> s
    [(0, 0), (0, -1), (0, 0), (0, 0), (0, 0)],     // s -> n
    [(0, 0), (1, 0), (0, 0), (0, 0), (0, 0)],      // e -> w
    [(0, 0), (-1, 0), (0, 0), (0, 0), (0, 0)],     // w -> e
];
