pub mod game;
pub mod replay;
pub mod sound;
#[cfg(test)]
mod tests;

use std::time::Duration;

use anyhow::Result;
use rand::prelude::*;
use serde::{Deserialize, Serialize};

pub use game::Game;
pub use game::Mode;

pub type Pos = [(i8, i8); 4];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct PieceLocation {
    pub piece: Piece,
    pub pos: (i8, i8),
    pub rot: Rotation,
}

impl From<tetrizz::data::PieceLocation> for PieceLocation {
    fn from(value: tetrizz::data::PieceLocation) -> Self {
        PieceLocation {
            piece: value.piece.into(),
            pos: (value.x, value.y),
            rot: value.rotation.into(),
        }
    }
}

macro_rules! lutify {
    (($e:expr) for $v:ident in [$($val:expr),*]) => {
        [
            $(
                {
                    let $v = $val;
                    $e
                }
            ),*
        ]
    };
}

macro_rules! piece_lut {
    ($v:ident => $e:expr) => {
        lutify!(($e) for $v in [Piece::I, Piece::J, Piece::L, Piece::O, Piece::S, Piece::T, Piece::Z])
    };
}

macro_rules! rotation_lut {
    ($v:ident => $e:expr) => {
        lutify!(($e) for $v in [Rotation::North, Rotation::East, Rotation::South, Rotation::West])
    };
}
pub const LUT: [[[(i8, i8); 4]; 4]; 7] =
    piece_lut!(piece => rotation_lut!(rotation => rotation.rotate_blocks(piece.blocks())));

impl PieceLocation {
    pub const fn blocks(&self) -> [(i8, i8); 4] {
        self.translate_blocks(LUT[self.piece as usize][self.rot as usize])
    }

    const fn translate(&self, (x, y): (i8, i8)) -> (i8, i8) {
        (x + self.pos.0, y + self.pos.1)
    }

    const fn translate_blocks(&self, cells: [(i8, i8); 4]) -> [(i8, i8); 4] {
        [
            self.translate(cells[0]),
            self.translate(cells[1]),
            self.translate(cells[2]),
            self.translate(cells[3]),
        ]
    }

    pub fn new(piece: Piece, pos: (i8, i8), rot: Rotation) -> Self {
        Self { piece, rot, pos }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum Piece {
    I,
    J,
    L,
    O,
    S,
    T,
    Z,
}
impl Piece {
    pub const fn blocks(&self) -> [(i8, i8); 4] {
        match self {
            Piece::Z => [(-1, 1), (0, 1), (0, 0), (1, 0)],
            Piece::S => [(-1, 0), (0, 0), (0, 1), (1, 1)],
            Piece::I => [(-1, 0), (0, 0), (1, 0), (2, 0)],
            Piece::O => [(0, 0), (1, 0), (0, 1), (1, 1)],
            Piece::J => [(-1, 0), (0, 0), (1, 0), (-1, 1)],
            Piece::L => [(-1, 0), (0, 0), (1, 0), (1, 1)],
            Piece::T => [(-1, 0), (0, 0), (1, 0), (0, 1)],
        }
    }
}

impl From<Piece> for tetrizz::data::Piece {
    fn from(value: Piece) -> Self {
        match value {
            Piece::I => tetrizz::data::Piece::I,
            Piece::J => tetrizz::data::Piece::J,
            Piece::L => tetrizz::data::Piece::L,
            Piece::O => tetrizz::data::Piece::O,
            Piece::S => tetrizz::data::Piece::S,
            Piece::T => tetrizz::data::Piece::T,
            Piece::Z => tetrizz::data::Piece::Z,
        }
    }
}
impl From<tetrizz::data::Piece> for Piece {
    fn from(value: tetrizz::data::Piece) -> Self {
        match value {
            tetrizz::data::Piece::I => Piece::I,
            tetrizz::data::Piece::J => Piece::J,
            tetrizz::data::Piece::L => Piece::L,
            tetrizz::data::Piece::O => Piece::O,
            tetrizz::data::Piece::S => Piece::S,
            tetrizz::data::Piece::T => Piece::T,
            tetrizz::data::Piece::Z => Piece::Z,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Spin {
    Cw,
    Ccw,
    Flip,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rotation {
    #[default]
    North,
    East,
    South,
    West,
}

impl Rotation {
    pub const fn rotate_block(&self, (x, y): (i8, i8)) -> (i8, i8) {
        match self {
            Rotation::North => (x, y),
            Rotation::East => (y, -x),
            Rotation::South => (-x, -y),
            Rotation::West => (-y, x),
        }
    }

    pub const fn rotate_blocks(&self, blocks: [(i8, i8); 4]) -> [(i8, i8); 4] {
        [
            self.rotate_block(blocks[0]),
            self.rotate_block(blocks[1]),
            self.rotate_block(blocks[2]),
            self.rotate_block(blocks[3]),
        ]
    }
}

impl From<tetrizz::data::Rotation> for Rotation {
    fn from(value: tetrizz::data::Rotation) -> Self {
        match value {
            tetrizz::data::Rotation::North => Rotation::North,
            tetrizz::data::Rotation::East => Rotation::East,
            tetrizz::data::Rotation::South => Rotation::South,
            tetrizz::data::Rotation::West => Rotation::West,
        }
    }
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
    ShowSolution(u8),
    Undo,
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
    Lookahead,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Timer(TimerEvent),
    Input(InputEvent),
}

impl From<TimerEvent> for Event {
    fn from(t: TimerEvent) -> Self {
        Self::Timer(t)
    }
}

impl From<InputEvent> for Event {
    fn from(i: InputEvent) -> Self {
        Self::Input(i)
    }
}

const FRAME: Duration = Duration::from_nanos(16_666_667);

// TODO: make all these floats (maybe ms instead of frames?)
// TODO: find jstris softdrop delays and match them
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub das: u16,
    pub arr: u16,
    pub gravity: Option<u16>,
    pub soft_drop: u16, // TODO: add support for 0 for instant
    pub lock_delay: (u16, u16, u16),
    pub ghost: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            das: 10,
            arr: 2,
            gravity: Some(60),
            soft_drop: 4,
            lock_delay: (30, 300, 1200),
            ghost: true,
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
// SRS kicks from: https://harddrop.com/wiki/SRS#How_guideline_SRS_actually_works
// 180 kicks from: https://tetrio.wiki.gg/images/5/52/TETR.IO_180kicks.png?6d5d9d
// I spins are slightly asymetrical, see https://harddrop.com/wiki/I-spins_in_SRS
// (yum)
impl Piece {
    const fn get_your_kicks(self, rot: Rotation, dir: Spin) -> [(i8, i8); 6] {
        let next_rot = rot.rotate(dir);
        match self {
            Piece::O => [(0, 0); 6], // just be careful not to rotate the O piece at all lol
            Piece::I => match (rot, next_rot) {
                (Rotation::East, Rotation::North) => {
                    [(-1, 0), (-2, 0), (1, 0), (-2, -2), (1, 1), (-1, 0)]
                }
                (Rotation::East, Rotation::South) => {
                    [(0, -1), (-1, -1), (2, -1), (-1, 1), (2, -2), (0, -1)]
                }
                (Rotation::East, Rotation::West) => {
                    [(-1, -1), (0, -1), (-1, -1), (-1, -1), (-1, -1), (-1, -1)]
                }
                (Rotation::South, Rotation::North) => {
                    [(-1, 1), (-1, 0), (-1, 1), (-1, 1), (-1, 1), (-1, 1)]
                }
                (Rotation::South, Rotation::East) => {
                    [(0, 1), (-2, 1), (1, 1), (-2, 2), (1, -1), (0, 1)]
                }
                (Rotation::South, Rotation::West) => {
                    [(-1, 0), (1, 0), (-2, 0), (1, 1), (-2, -2), (-1, 0)]
                }
                (Rotation::West, Rotation::North) => {
                    [(0, 1), (1, 1), (-2, 1), (1, -1), (-2, 2), (0, 1)]
                }
                (Rotation::West, Rotation::East) => {
                    [(1, 1), (0, 1), (1, 1), (1, 1), (1, 1), (1, 1)]
                }
                (Rotation::West, Rotation::South) => {
                    [(1, 0), (2, 0), (-1, 0), (2, 2), (-1, -1), (1, 0)]
                }
                (Rotation::North, Rotation::East) => {
                    [(1, 0), (2, 0), (-1, 0), (-1, -1), (2, 2), (1, 0)]
                }
                (Rotation::North, Rotation::West) => {
                    [(0, -1), (-1, -1), (2, -1), (2, -2), (-1, 1), (0, -1)]
                }
                (Rotation::North, Rotation::South) => {
                    [(1, -1), (1, 0), (1, -1), (1, -1), (1, -1), (1, -1)]
                }
                _ => unreachable!(),
            },
            _ => match (rot, next_rot) {
                (Rotation::East, Rotation::North) => {
                    [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2), (0, 0)]
                }
                (Rotation::East, Rotation::South) => {
                    [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2), (0, 0)]
                }
                (Rotation::East, Rotation::West) => {
                    [(0, 0), (1, 0), (1, 2), (1, 1), (0, 2), (0, 1)]
                }
                (Rotation::South, Rotation::North) => {
                    [(0, 0), (0, -1), (-1, -1), (1, -1), (-1, 0), (1, 0)]
                }
                (Rotation::South, Rotation::East) => {
                    [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2), (0, 0)]
                }
                (Rotation::South, Rotation::West) => {
                    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2), (0, 0)]
                }
                (Rotation::West, Rotation::North) => {
                    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2), (0, 0)]
                }
                (Rotation::West, Rotation::East) => {
                    [(0, 0), (-1, 0), (-1, 2), (-1, 1), (0, 2), (0, 1)]
                }
                (Rotation::West, Rotation::South) => {
                    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2), (0, 0)]
                }
                (Rotation::North, Rotation::East) => {
                    [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2), (0, 0)]
                }
                (Rotation::North, Rotation::West) => {
                    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2), (0, 0)]
                }
                (Rotation::North, Rotation::South) => {
                    [(0, 0), (0, 1), (1, 1), (-1, 1), (1, 0), (-1, 0)]
                }
                _ => unreachable!(),
            },
        }
    }
}

pub const BG_COLOR: (u8, u8, u8) = (20, 20, 20);
// const DONE_COLOR: (u8, u8, u8) = (106, 106, 106);
pub const LOST_COLOR: (u8, u8, u8) = (106, 106, 106); // TODO: differentiate from DONE

pub trait Color {
    fn color(self) -> (u8, u8, u8);
}

impl Color for Piece {
    fn color(self) -> (u8, u8, u8) {
        match self {
            Piece::I => (15, 155, 215),
            Piece::J => (33, 65, 198),
            Piece::L => (227, 91, 2),
            Piece::O => (227, 159, 2),
            Piece::S => (89, 177, 1),
            Piece::T => (175, 41, 138),
            Piece::Z => (215, 15, 55),
        }
    }
}

impl Color for Cell {
    fn color(self) -> (u8, u8, u8) {
        match self {
            Cell::Piece(piece) => piece.color(),
            Cell::Garbage => LOST_COLOR,
            Cell::Empty => (0, 0, 0),
        }
    }
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
