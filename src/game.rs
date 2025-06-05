use rand::prelude::*;

use crate::sound::Player;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

pub type Pos = [(i8, i8); 4];

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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

#[derive(Copy, Clone, Debug)]
pub enum Direction {
    Left,
    Right,
}

#[derive(Copy, Clone, Debug)]
pub enum Spin {
    Cw,
    Ccw,
    Flip,
}

impl TryFrom<InputEvent> for Spin {
    type Error = ();
    fn try_from(input: InputEvent) -> Result<Self, Self::Error> {
        match input {
            InputEvent::Cw => Ok(Self::Cw),
            InputEvent::Ccw => Ok(Self::Ccw),
            InputEvent::Flip => Ok(Self::Flip),
            _ => Err(()),
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
#[repr(u8)]
pub enum Rotation {
    #[default]
    North,
    East,
    South,
    West,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InputEvent {
    PressLeft,
    ReleaseLeft,
    PressRight,
    ReleaseRight,
    PressSoft,
    ReleaseSoft,
    Cw,
    Ccw,
    Flip,
    Hard,
    Hold,
    // maybe pull these out along with Garbage/Pause/StartSound to a "misc" event
    Restart,
    Quit,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TimerEvent {
    DasLeft,
    DasRight,
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub das: u8,
    pub arr: u8,
    pub gravity: u16,
    pub soft_drop: u8,
    pub lock_delay: (u8, u16, u16),
    pub ghost: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Startup,
    Done,
    Lost,
    Running,
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
    pub board: [[Cell; 10]; 23],
    pub upcomming: VecDeque<Piece>,
    pub current: (Piece, (i8, i8), Rotation),
    pub hold: Option<Piece>,
    pub lines: u16,
    pub config: Config,
    pub timers: VecDeque<(Instant, TimerEvent)>,
    pub started_right: Option<Instant>,
    pub started_left: Option<Instant>,
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
        // TODO: start sound event?
        // self.set_timer(TimerEvent::StartSound, self.last_update, 60);
        self.set_timer(TimerEvent::Start, Instant::now(), 120);
    }

    pub fn handle(&mut self, event: Event, time: Instant, player: &Player) {
        use {Event::*, InputEvent::*, TimerEvent::*};
        match event {
            Input(Hard) | Timer(Lock | Extended | Timeout) => self.hard_drop(player),
            Input(PressLeft) => {
                if self.try_move((-1, 0)) {
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
                        self.state = GameState::Lost;
                    } else {
                        player.play("hold").ok();
                        self.can_hold = false;
                    }
                }
            }
            Input(rot @ (Cw | Ccw | Flip)) => {
                if self.try_rotate(rot.try_into().expect("should always be a rotation")) {
                    self.clear_timer(Lock);
                    player.play("rotate").ok();
                }
                self.clear_timer(Extended);
            }
            Input(PressSoft) => {
                self.clear_timer(Gravity);
                self.set_timer(SoftDrop, time, self.config.soft_drop as u32);
            }
            Input(ReleaseSoft) => {
                self.clear_timer(SoftDrop);
                self.set_timer(Gravity, time, self.config.gravity as u32);
            }
            Input(Restart | Quit) => unreachable!("should be handled in outer event loop"),
            Timer(DasLeft) => {
                // TODO: add das sound effect
                todo!()
            }
            Timer(DasRight) => {
                // TODO: add das sound effect
                todo!()
            }
            Timer(t @ (SoftDrop | Gravity)) => {
                todo!()
            }
            Timer(Arr) => {
                todo!()
            }
            // Timer(StartSound) => {
            //     player.play("go").ok();
            //     self.set_timer(Start, time, 60);
            // }
            Timer(Start) => {
                self.state = GameState::Running;
                let next = self.pop_piece();
                self.spawn(next);
                self.start_time = Some(time);
            }
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
        while self.try_move((0, -1)) {}
        let old_lines = self.lines;
        if self.lock() {
            match self.lines {
                n if n == old_lines => player.play("lock").ok(),
                40.. => {
                    player.play("win").or_else(|_| player.play("lock")).ok();
                    self.state = GameState::Done;
                    Some(())
                }
                _ => player.play("line").ok(),
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
        self.try_move((0, -1));
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
                return true;
            }
        }
        false
    }

    fn try_move(&mut self, (dx, dy): (i8, i8)) -> bool {
        let (p, (x, y), rot) = self.current;
        let pos = (x + dx, y + dy);
        if self.check_valid(p.get_pos(rot, pos)) {
            self.current = (p, pos, rot);
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
