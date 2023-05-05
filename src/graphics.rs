use crate::game::{Game, GameState, Piece, Rotation};

use anyhow::{anyhow, Result};
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

fn move_cursor(o: &mut StdoutLock, (x, y): (i16, i16)) -> Result<()> {
    Ok(write!(o, "{}{};{}H", csi!(), y + 1, x + 1)?)
}

pub fn draw(width: i16, height: i16, game: &Game) -> Result<()> {
    let mut lock = io::stdout().lock();
    let o = &mut lock;
    let (ox, oy) = (width / 2 - 19, height / 2 - 11);
    draw_board(o, game, (ox + 10, oy))?;
    if let Some(hold) = game.hold {
        draw_piece(o, hold, (ox, oy + 2))?;
    }
    for i in 0..5 {
        draw_piece(
            o,
            *game.upcomming.get(i).ok_or(anyhow!("piece queue empty"))?,
            (ox + 32, oy + 2 + 3 * i as i16),
        )?;
    }
    let text_color = (255, 255, 255);
    draw_text(
        o,
        (ox + 34, oy + 20),
        text_color,
        &(40 - game.lines as i32).max(0).to_string(),
    )?;
    let frames = game.current_frame.saturating_sub(120);
    let mins = frames / 3600;
    let secs = frames % 3600 / 60;
    let millis = frames % 60 * 10 / 6;
    let time = if mins != 0 {
        format!("{mins}:{secs:02}.{millis:02} ")
    } else {
        format!("{secs}.{millis:02} ")
    };
    draw_text(o, (ox + 1, oy + 20), text_color, &time)?;
    Ok(o.flush()?)
}

fn draw_piece(o: &mut StdoutLock, piece: Piece, origin: (i16, i16)) -> Result<()> {
    let pos = piece.get_pos(Rotation::North, (origin.0 as i8, origin.1 as i8));
    let (x, y) = origin;
    for dy in 0..4 {
        move_cursor(o, (x, y + dy))?;
        for dx in 0..4 {
            set_color(
                o,
                if pos.contains(&(x as i8 + dx, (y - dy) as i8)) {
                    piece.color()
                } else {
                    BG_COLOR
                },
            )?;
            write!(o, "  ")?;
        }
    }
    Ok(())
}

fn draw_text(
    o: &mut StdoutLock,
    origin: (i16, i16),
    (r, g, b): (u8, u8, u8),
    content: &str,
) -> Result<()> {
    move_cursor(o, origin)?;
    Ok(write!(o, "\x1b[38;2;{r};{g};{b}m{content}")?)
}

fn draw_board(o: &mut StdoutLock, g: &Game, origin: (i16, i16)) -> Result<()> {
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

    set_color(o, BG_COLOR)?;
    write!(o, csi!("2J"))?;
    move_cursor(o, (ox, oy))?;
    for y in 0..20i8 {
        move_cursor(o, (ox, oy + y as i16 + 1))?;
        for x in 0..10i8 {
            let y = 19 - y;
            let mut color = g.board[y as usize][x as usize]
                .map(|p| {
                    if g.state == GameState::Lost {
                        LOST_COLOR
                    } else {
                        p.color()
                    }
                })
                .unwrap_or((0, 0, 0));
            if current_pos.contains(&(x, y)) && g.state == GameState::Running {
                color = piece.color()
            } else if g.config.ghost && ghost.contains(&(x, y)) && g.state == GameState::Running {
                let (r, g, b) = piece.color();
                color = (r / 3, g / 3, b / 3);
            }
            set_color(o, color)?;
            write!(o, "  ")?;
        }
    }
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
