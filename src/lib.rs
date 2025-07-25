mod game;
pub mod replay;
pub mod settings;

use std::time::Duration;

use anyhow::Result;
use rand::prelude::*;
use serde::{Deserialize, Serialize};

pub use game::Game;

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
pub enum Spin {
    Cw,
    Ccw,
    Flip,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    // Undo,
    // Redo,
    // Garbage(n) // just for garbage line timer, need special handling to displace current piece upwards
    // Attack(n)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TimerEvent {
    DasLeft,
    DasRight,
    Arr,
    SoftDrop, // TODO: only use 1 timer for gravity?
    Gravity,
    Lock,
    Extended,
    Timeout,
    Start,
    Are,
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

#[derive(Copy, Clone, Debug)]
pub struct Bindings {
    pub left: char,
    pub right: char,
    pub soft: char,
    pub hard: char,
    pub cw: char,
    pub ccw: char,
    pub flip: char,
    pub hold: char,
}

impl Default for Bindings {
    fn default() -> Self {
        use settings::keys::*;
        Self {
            left: LEFT,
            right: RIGHT,
            soft: DOWN,
            hard: UP,
            cw: 'x',
            ccw: 'z',
            flip: 'a',
            hold: LEFT_SHIFT,
        }
    }
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
        PIECE_DATA[self as usize][r as usize].map(|(a, b)| (x + a, y - b))
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

pub trait Sound {
    // TODO: asyncify this to lazy-load sounds while playing
    // TODO: handle url instead of just path, download to cache
    fn add_sound(&mut self, name: &str, resource: &str) -> Result<()>;
    fn set_volume(&mut self, level: f32);
    fn play(&self, s: &str) -> Result<()>;
}

impl TryFrom<InputEvent> for Spin {
    type Error = ();

    fn try_from(value: InputEvent) -> Result<Self, ()> {
        Ok(match value {
            InputEvent::Cw => Spin::Cw,
            InputEvent::Ccw => Spin::Ccw,
            InputEvent::Flip => Spin::Flip,
            _ => return Err(()),
        })
    }
}

// ordered n, e, s, w
const PIECE_DATA: [[Pos; 4]; 7] = [
    [
        [(-2, -1), (-1, -1), (0, -1), (1, -1)], // I
        [(0, -2), (0, -1), (0, 0), (0, 1)],
        [(-2, 0), (-1, 0), (0, 0), (1, 0)],
        [(-1, -2), (-1, -1), (-1, 0), (-1, 1)],
    ],
    [
        [(-2, -2), (-2, -1), (-1, -1), (0, -1)], // J
        [(-1, -2), (0, -2), (-1, -1), (-1, 0)],
        [(-2, -1), (-1, -1), (0, -1), (0, 0)],
        [(-1, -2), (-1, -1), (-2, 0), (-1, 0)],
    ],
    [
        [(0, -2), (-2, -1), (-1, -1), (0, -1)], // L
        [(-1, -2), (-1, -1), (-1, 0), (0, 0)],
        [(-2, -1), (-1, -1), (0, -1), (-2, 0)],
        [(-2, -2), (-1, -2), (-1, -1), (-1, 0)],
    ],
    [
        [(-1, -2), (0, -2), (-1, -1), (0, -1)], // O
        [(-1, -2), (0, -2), (-1, -1), (0, -1)],
        [(-1, -2), (0, -2), (-1, -1), (0, -1)],
        [(-1, -2), (0, -2), (-1, -1), (0, -1)],
    ],
    [
        [(-1, -2), (0, -2), (-2, -1), (-1, -1)], // S
        [(-1, -2), (-1, -1), (0, -1), (0, 0)],
        [(-1, -1), (0, -1), (-2, 0), (-1, 0)],
        [(-2, -2), (-2, -1), (-1, -1), (-1, 0)],
    ],
    [
        [(-1, -2), (-2, -1), (-1, -1), (0, -1)], // T
        [(-1, -2), (-1, -1), (0, -1), (-1, 0)],
        [(-2, -1), (-1, -1), (0, -1), (-1, 0)],
        [(-1, -2), (-2, -1), (-1, -1), (-1, 0)],
    ],
    [
        [(-2, -2), (-1, -2), (-1, -1), (0, -1)], // Z
        [(0, -2), (-1, -1), (0, -1), (-1, 0)],
        [(-2, -1), (-1, -1), (-1, 0), (0, 0)],
        [(-1, -2), (-2, -1), (-1, -1), (-2, 0)],
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
