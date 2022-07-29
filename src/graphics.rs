use crate::game::{Game, GameState, Piece, Rotation};

use anyhow::Result;
use termios::*;

use std::{
    io::{self, StdoutLock, Write},
    os::unix::prelude::AsRawFd,
    sync::atomic::{AtomicU8, Ordering::Relaxed},
};

macro_rules! csi {
    ($( $x:expr ),*) => { concat!("\x1b[", $( $x ),*) };
}

const COLORS: [(u8, u8, u8); 7] = [
    (15, 155, 215),
    (33, 65, 198),
    (227, 91, 2),
    (227, 159, 2),
    (89, 177, 1),
    (175, 41, 138),
    (215, 15, 55),
];

const BG_COLOR: (u8, u8, u8) = (20, 20, 20);
const LOST_COLOR: (u8, u8, u8) = (106, 106, 106);

impl Piece {
    pub fn color(self) -> (u8, u8, u8) {
        COLORS[self as usize]
    }
}

fn set_color(o: &mut StdoutLock, (r, g, b): (u8, u8, u8)) -> Result<()> {
    static CURRENT_R: AtomicU8 = AtomicU8::new(0);
    static CURRENT_G: AtomicU8 = AtomicU8::new(0);
    static CURRENT_B: AtomicU8 = AtomicU8::new(0);
    if CURRENT_R.load(Relaxed) != r || CURRENT_G.load(Relaxed) != g || CURRENT_B.load(Relaxed) != b
    {
        CURRENT_R.store(r, Relaxed);
        CURRENT_G.store(g, Relaxed);
        CURRENT_B.store(b, Relaxed);
        Ok(write!(o, "{};{r};{g};{b}m", csi!("48;2"))?)
    } else {
        Ok(())
    }
}

fn move_cursor(o: &mut StdoutLock, (x, y): (i8, i8)) -> Result<()> {
    Ok(write!(o, "{}{};{}H", csi!(), y + 1, x + 1)?)
}

pub fn draw_piece(piece: Piece, origin: (i8, i8)) -> Result<()> {
    let mut lock = io::stdout().lock();
    let o = &mut lock;
    let pos = piece.get_pos(Rotation::North, origin);
    let (x, y) = origin;
    for dy in 0..4 {
        move_cursor(o, (x, y + dy))?;
        for dx in 0..4 {
            set_color(
                o,
                if pos.contains(&(x + dx, y - dy)) {
                    piece.color()
                } else {
                    BG_COLOR
                },
            )?;
            write!(o, "  ")?;
        }
    }
    Ok(o.flush()?)
}

pub fn draw_text(origin: (i8, i8), (r, g, b): (u8, u8, u8), content: &str) -> Result<()> {
    let mut lock = io::stdout().lock();
    move_cursor(&mut lock, origin)?;
    write!(lock, "\x1b[38;2;{r};{g};{b}m{content}")?;
    Ok(lock.flush()?)
}

pub fn draw_board(g: &Game, origin: (i8, i8)) -> Result<()> {
    let (ox, oy) = origin;
    let (piece, pos, rot) = g.current;
    let current_pos = piece.get_pos(rot, pos);
    let mut ghost = current_pos;
    loop {
        let next = ghost.map(|(x, y)| (x, y - 1));
        if !g.check_valid(next) {
            break;
        }
        ghost = next;
    }

    let mut lock = io::stdout().lock();
    let o = &mut lock;
    set_color(o, BG_COLOR)?;
    write!(o, csi!("2J"))?;
    move_cursor(o, (ox, oy))?;
    for y in 0..20 {
        move_cursor(o, (ox, oy + y as i8 + 1))?;
        for x in 0..10 {
            let y = 19 - y;
            let mut color = g.board[y][x]
                .map(|p| {
                    if g.state == GameState::Lost {
                        LOST_COLOR
                    } else {
                        p.color()
                    }
                })
                .unwrap_or((0, 0, 0));
            if current_pos.contains(&(x as i8, y as i8)) && g.state == GameState::Running {
                color = piece.color()
            } else if g.config.ghost
                && ghost.contains(&(x as i8, y as i8))
                && g.state == GameState::Running
            {
                let (r, g, b) = piece.color();
                color = (r / 3, g / 3, b / 3);
            }
            set_color(o, color)?;
            write!(o, "  ")?;
        }
    }
    o.flush()?;
    Ok(())
}

pub struct RawMode {
    original: Termios,
}

impl RawMode {
    pub fn enter() -> Result<Self> {
        let mut lock = io::stdout().lock();
        let fd = lock.as_raw_fd();
        let mut terminfo = Termios::from_fd(fd)?;
        let original = terminfo;
        cfmakeraw(&mut terminfo);
        write!(lock, csi!("?1049h"))?; // switch to alternate screen
        write!(lock, csi!("?25l"))?; // hide cursor

        // TODO detect if this is supported and otherwise error out:
        // https://sw.kovidgoyal.net/kitty/keyboard-protocol/#detection-of-support-for-this-protocol
        write!(lock, csi!(">15u"))?; // change keyboard mode
        lock.flush().unwrap();
        tcsetattr(fd, TCSADRAIN, &terminfo)?;
        Ok(Self { original })
    }
}

// set an atexit handler for this because sometimes it doesnt seem to run?
impl Drop for RawMode {
    fn drop(&mut self) {
        fn reset_state(orig: Termios) -> Result<()> {
            let mut lock = io::stdout().lock();
            tcsetattr(lock.as_raw_fd(), TCSADRAIN, &orig)?;
            write!(lock, csi!("<u"))?; // restore keyboard mode
            write!(lock, csi!("?1049l"))?; // switch back from alternate screen
            write!(lock, csi!("?25h"))?; // show cursor
            Ok(lock.flush()?)
        }
        reset_state(self.original).unwrap();
    }
}
