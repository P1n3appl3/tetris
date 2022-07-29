use rand::prelude::*;

use crate::sound::Player;
use std::collections::VecDeque;

pub type Pos = [(i8, i8); 4];

#[derive(Copy, Clone, Debug)]
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

#[derive(Copy, Clone, Debug, Default)]
#[repr(u8)]
pub enum Rotation {
    #[default]
    North,
    East,
    South,
    West,
}

#[derive(Default, Clone, Debug)]
pub struct Inputs {
    pub dir: Option<Direction>,
    pub rotate: Option<Direction>,
    pub left: bool,
    pub right: bool,
    pub soft: bool,
    pub hard: bool,
    pub hold: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub das: u8,
    pub arr: u8,
    pub gravity: u16,
    pub soft_drop: u8,
    pub lock_delay: (u8, u16, u16),
    pub ghost: bool,
}

#[derive(Debug, Clone)]
pub struct Timers {
    pub das_left: u8,
    pub das_right: u8,
    pub arr: i8,
    pub soft: i8,
    pub gravity: u16,
    pub lock: u8,
    pub extended: u16,
    pub timeout: u16,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Startup,
    Done,
    Lost,
    Running,
}

#[derive(Clone)]
pub struct Game {
    pub board: [[Option<Piece>; 10]; 23],
    pub upcomming: VecDeque<Piece>,
    pub current: (Piece, (i8, i8), Rotation),
    pub hold: Option<Piece>,
    pub lines: u16,
    pub config: Config,
    pub timers: Timers,
    pub last_dir: Direction,
    pub can_hold: bool,
    pub current_frame: u16,
    pub state: GameState,
    pub rng: StdRng,
}

impl Rotation {
    const fn rotate(self, dir: Direction) -> Self {
        use {Direction::*, Rotation::*};
        match (self, dir) {
            (North, Left) => West,
            (North, Right) => East,
            (East, Left) => North,
            (East, Right) => South,
            (South, Left) => East,
            (South, Right) => West,
            (West, Left) => South,
            (West, Right) => North,
        }
    }
}

impl Piece {
    pub fn get_pos(self, r: Rotation, (x, y): (i8, i8)) -> Pos {
        DATA[self as usize][r as usize].map(|(a, b)| (x + a, y - b))
    }

    fn get_your_kicks(self, rot: Rotation, dir: Direction) -> [(i8, i8); 5] {
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
            _ => unreachable!("invalid rotation"),
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
            last_dir: Direction::Left,
            can_hold: true,
            current_frame: Default::default(),
            state: GameState::Done,
        }
    }

    pub fn start(&mut self, seed: u64) {
        self.state = GameState::Startup;
        self.board = Default::default();
        self.hold = None;
        self.current_frame = 0;
        self.lines = 0;
        self.upcomming.clear();
        self.rng = StdRng::seed_from_u64(seed);
        self.fill_bag();
        while let Some(Piece::Z | Piece::S) = self.upcomming.front() {
            self.pop_piece();
        }
    }

    pub fn step(&mut self, inputs: &Inputs, player: &Player) {
        // buffer das before the game starts
        match self.state {
            GameState::Running => {}
            GameState::Startup => {
                self.check_das(inputs);
                match self.current_frame {
                    0 => {
                        player.play("ready").ok();
                    }
                    60 => {
                        player.play("go").ok();
                    }
                    120 => {
                        self.state = GameState::Running;
                        let next = self.pop_piece();
                        self.spawn(next);
                    }
                    _ => {}
                }
                self.current_frame += 1;
                return;
            }
            _ => return,
        }
        self.current_frame += 1;

        //locking
        self.timers.timeout += 1;
        let (piece, pos, rot) = self.current;
        if !self.check_valid(piece.get_pos(rot, pos).map(|(x, y)| (x, y - 1))) {
            self.timers.lock += 1;
            self.timers.extended += 1;
        } else {
            self.timers.lock = 0;
            self.timers.extended = 0;
        }
        if inputs.hard
            || self.timers.lock == self.config.lock_delay.0
            || self.timers.extended == self.config.lock_delay.1
            || self.timers.timeout == self.config.lock_delay.2
        {
            while self.try_move((0, -1)) {}
            let old_lines = self.lines;
            if self.lock() {
                match self.lines {
                    n if n == old_lines => player.play("lock").ok(),
                    40.. => {
                        player.play("win").or_else(|_| player.play("lock")).ok();
                        self.state = GameState::Done;
                        return;
                    }
                    _ => player.play("line").ok(),
                };
            } else {
                player.play("lose").ok();
                self.state = GameState::Lost;
                return;
            }
        }

        //hold
        if inputs.hold && self.can_hold {
            if !self.hold() {
                player.play("lose").ok();
                self.state = GameState::Lost;
                return;
            }
            player.play("hold").ok();
            self.can_hold = false;
        }

        // rotation
        if let Some(dir) = inputs.rotate {
            if self.try_rotate(dir) {
                self.timers.lock = 0;
                self.timers.extended = 0;
                player.play("rotate").ok();
            }
        }

        // left/right movement
        match inputs.dir {
            Some(Direction::Left) => {
                self.try_move((-1, 0));
                self.last_dir = Direction::Left;
                self.timers.lock = 0;
                player.play("move").ok();
            }
            Some(Direction::Right) => {
                self.try_move((1, 0));
                self.last_dir = Direction::Right;
                self.timers.lock = 0;
                player.play("move").ok();
            }
            // DAS
            None => {
                self.check_das(inputs);
                let (dir, current_das) = match self.last_dir {
                    Direction::Left => (-1, self.timers.das_left),
                    Direction::Right => (1, self.timers.das_right),
                };
                if current_das == self.config.das {
                    if self.timers.arr == -1 {
                        while self.try_move((dir, 0)) && self.config.arr == 0 {}
                        self.timers.arr = 0;
                    } else if self.config.arr == 0 {
                        while self.try_move((dir, 0)) {}
                    } else {
                        self.timers.arr += 1;
                        if self.timers.arr == self.config.arr as i8 {
                            self.try_move((dir, 0));
                            self.timers.arr = 0;
                        }
                    }
                } else {
                    self.timers.arr = -1;
                }
            }
        }

        self.apply_gravity(inputs);
    }

    fn check_das(&mut self, inputs: &Inputs) {
        if inputs.left {
            self.timers.das_left = self.config.das.min(self.timers.das_left + 1);
            if !inputs.right {
                self.last_dir = Direction::Left;
            }
        } else {
            self.timers.das_left = 0;
        }
        if inputs.right {
            self.timers.das_right = self.config.das.min(self.timers.das_right + 1);
            if !inputs.left {
                self.last_dir = Direction::Right;
            }
        } else {
            self.timers.das_right = 0;
        }
    }

    fn apply_gravity(&mut self, inputs: &Inputs) {
        if inputs.soft {
            self.timers.gravity = 0;
            if self.timers.soft != -1 {
                self.timers.soft += 1;
                if self.timers.soft == self.config.soft_drop as i8 {
                    self.try_move((0, -1));
                    self.timers.soft = 0;
                }
            } else {
                self.try_move((0, -1));
                self.timers.soft = 0;
            }
        } else {
            self.timers.gravity += 1;
            if self.timers.gravity == self.config.gravity {
                self.try_move((0, -1));
                self.timers.gravity = 0;
                if self.timers.soft != -1 {
                    self.timers.soft = 0;
                }
            }
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
                && self.board[y as usize][x as usize].is_none()
        })
    }

    fn lock(&mut self) -> bool {
        let (p, pos, rot) = self.current;
        for (x, y) in p.get_pos(rot, pos) {
            self.board[y as usize][x as usize] = Some(p);
        }
        for i in (0..23).rev() {
            if self.board[i].iter().all(|p| p.is_some()) {
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
        self.timers.soft = -1;
        self.timers.gravity = 0;
        self.timers.lock = 0;
        self.timers.extended = 0;
        self.timers.timeout = 0;
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

    fn try_rotate(&mut self, dir: Direction) -> bool {
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

impl Default for Timers {
    fn default() -> Self {
        Self {
            soft: -1,
            arr: -1,
            das_left: 0,
            das_right: 0,
            gravity: 0,
            lock: 0,
            extended: 0,
            timeout: 0,
        }
    }
}

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

const ROTI: [[(i8, i8); 5]; 8] = [
    [(0, 0), (-2, 0), (1, 0), (-2, -1), (1, 2)],
    [(0, 0), (2, 0), (-1, 0), (2, 1), (-1, -2)],
    [(0, 0), (-1, 0), (2, 0), (-1, 2), (2, -1)],
    [(0, 0), (1, 0), (-2, 0), (1, -2), (-2, 1)],
    [(0, 0), (2, 0), (-1, 0), (2, 1), (-1, -2)],
    [(0, 0), (-2, 0), (1, 0), (-2, -1), (1, 2)],
    [(0, 0), (1, 0), (-2, 0), (1, -2), (-2, 1)],
    [(0, 0), (-1, 0), (2, 0), (-1, 2), (2, -1)],
];

const ROTJLSTZ: [[(i8, i8); 5]; 8] = [
    [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)],
    [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],
    [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],
    [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)],
    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],
    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],
];
