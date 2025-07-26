use log::error;
use tetris::{Cell, Game, GameState, Piece, PieceLocation, Rotation};

use anyhow::{anyhow, Result};
use termios::*;

use std::{
    io::{self, Read as _, StdoutLock, Write},
    os::unix::prelude::AsRawFd,
    process::exit,
    sync::{
        atomic::{AtomicU8, Ordering::Relaxed},
        mpsc::RecvTimeoutError,
    },
    thread,
    time::{Duration, Instant},
};

macro_rules! csi {
    ($( $x:expr ),*) => { concat!("\x1b[", $( $x ),*) };
}

const BG_COLOR: (u8, u8, u8) = (20, 20, 20);
// const DONE_COLOR: (u8, u8, u8) = (106, 106, 106);
const LOST_COLOR: (u8, u8, u8) = (106, 106, 106); // TODO: differentiate from DONE

trait Color {
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

    set_color(o, BG_COLOR)?;
    write!(o, csi!("2J"))?;

    // Origin is top left of drawing area
    let (ox, oy) = (width / 2 - 19, height / 2 - 11);
    draw_board(o, game, (ox + 10, oy))?;
    if let Some(hold) = game.hold {
        draw_piece(o, hold, (ox, oy + 4))?;
    }
    for i in 0..5 {
        draw_piece(
            o,
            *game.upcomming.get(i).ok_or(anyhow!("piece queue empty"))?,
            (ox + 32, oy + 4 + 3 * i as i16),
        )?;
    }
    let text_color = (255, 255, 255);
    if let Some(target) = game.target_lines {
        set_color(o, BG_COLOR)?;
        draw_text(
            o,
            (ox + 34, oy + 20),
            text_color,
            &(target.saturating_sub(game.lines)).to_string(),
        )?;
    }
    let now = Instant::now();
    let time = match game.state {
        GameState::Startup => Duration::ZERO,
        GameState::Running => now.duration_since(game.start_time.unwrap_or(now)),
        GameState::Done => game.time.duration_since(game.start_time.unwrap_or(now)),
    };
    let mins = time.as_secs() / 60;
    let secs = time.as_secs() % 60;
    let decis = time.as_millis() % 1000 / 100;
    let time = if mins != 0 {
        format!("{mins:2}:{secs:02}.{decis:01} ")
    } else {
        format!("{secs:2}.{decis:01} ")
    };
    set_color(o, BG_COLOR)?;
    draw_text(o, (ox + 1, oy + 20), text_color, &time)?;
    draw_spins(o, game, (ox, oy))?;
    Ok(o.flush()?)
}

fn draw_spins(o: &mut StdoutLock, game: &Game, (ox, oy): (i16, i16)) -> Result<()> {
    let text_color = (255, 255, 255);
    set_color(o, BG_COLOR)?;
    let spins = Game::spin_shortlist(&game.spins);
    if let Some((suggestion, solution)) = &game.solution {
        // render selected solution
        let mut game = solution.clone();
        let last = suggestion.moves.len() - 1;
        log::info!("{}", last);
        for (id, (m, i)) in suggestion.moves.iter().enumerate() {
            let b2b = if m.spun && i.lines_cleared > 0 {
                " +1 b2b"
            } else {
                ""
            };
            let m = format!("{:?} x:{} y:{} {:?}{}", m.piece, m.x, m.y, m.rotation, b2b);

            draw_text(o, (ox - 30, oy + 5 + id as i16), text_color, &m)?;
            if !b2b.is_empty() {
                break;
            }
        }
        for (ind, (loc, placement_info)) in suggestion.moves.iter().enumerate() {
            log::info!("hold: {:?}", game.hold);
            log::info!("current: {:?}", game.current);
            log::info!("upcoming: {:?}", game.upcomming);
            if game.hold.is_none() {
                game.hold();
                game.can_hold = true;
            }
            if Some(loc.piece) == game.hold.map(Into::into) {
                game.hold();
            } else {
                assert_eq!(loc.piece, game.current.piece.into());
            }
            game.current.rot = loc.rotation.into();
            game.current.pos = (loc.x, loc.y);
            log::info!("current pre-lock: {:?}", game.current);
            draw_board(o, &game, (ox + 40, oy))?;
            o.flush()?;
            std::thread::sleep(Duration::from_millis(400));
            if ind == last || (loc.spun && placement_info.lines_cleared > 0) {
                break;
            }
            game.lock();
            draw_board(o, &game, (ox + 40, oy))?;
            o.flush()?;
            std::thread::sleep(Duration::from_millis(400));
        }
        log::info!("drawing solution board");
    } else {
        if game.history.is_empty() {
            return Ok(());
        }
        let prev = game.history.last().unwrap();
        // render available solutions
        for (id, spin) in spins.iter().enumerate() {
            let spin_move_position = spin
                .moves
                .iter()
                .position(|m| m.0.spun && m.1.lines_cleared > 0)
                .unwrap();
            let spin_move = spin.moves[spin_move_position];
            let spin_summary = format!(
                "{}: {:?} spin in {} pieces clearing {} lines",
                id + 1,
                spin_move.0.piece,
                spin_move_position,
                spin_move.1.lines_cleared,
            );
            let prev_moves = prev
                .spins
                .iter()
                .map(|node| &node.moves)
                .collect::<Vec<_>>();

            let matches_previous_move = prev_moves
                .iter()
                .find(|&&prev_moveset| {
                    let prev_spin_move_position = prev_moveset
                        .iter()
                        .position(|m| m.0.spun && m.1.lines_cleared > 0)
                        .unwrap();
                    let prev_spin_move = prev_moveset[prev_spin_move_position];
                    prev_spin_move.0.piece == spin_move.0.piece
                        && prev_spin_move_position > spin_move_position
                })
                .is_some();
            let solution_color = if matches_previous_move {
                (0x03, 0xc0, 0x4a) // parakeet
            } else {
                BG_COLOR
            };
            set_color(o, solution_color)?;
            draw_text(o, (ox - 30, oy + 5 + id as i16), text_color, &spin_summary)?;
        }
    }
    Ok(())
}

fn draw_piece(o: &mut StdoutLock, piece: Piece, origin: (i16, i16)) -> Result<()> {
    let p = PieceLocation::new(piece, (origin.0 as _, origin.1 as _), Rotation::North);
    let pos = p.blocks();
    let (x, y) = origin;
    for dy in -1..=0 {
        move_cursor(o, (x, y + dy))?;
        for dx in -1..=2 {
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
    let current_pos = g.current.blocks();
    let mut ghost = current_pos;
    loop {
        let next = ghost.map(|(x, y)| (x, y - 1));
        if !g.check_valid(next) {
            break;
        }
        ghost = next;
    }
    move_cursor(o, (ox, oy))?;
    for y in 0..22i8 {
        move_cursor(o, (ox, oy + y as i16 + 1))?;
        for x in 0..10i8 {
            let y = 21 - y;
            let mut color = g.board[y as usize][x as usize].color();
            if g.state == GameState::Done && color != Default::default() {
                color = LOST_COLOR;
            } else if current_pos.contains(&(x, y)) && g.state == GameState::Running {
                color = g.current.piece.color()
            } else if g.config.ghost && ghost.contains(&(x, y)) && g.state == GameState::Running {
                let (r, g, b) = g.current.piece.color();
                color = (r / 3, g / 3, b / 3);
            } else if y > 19 {
                color = BG_COLOR;
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
        let (tx, rx) = std::sync::mpsc::channel();
        thread::spawn(move || -> Result<()> {
            {
                let mut output = io::stdout().lock();
                write!(output, csi!("?u"))?; // query keyboard mode
            }
            let mut buf = [0; 32];
            {
                let mut input = io::stdin().lock();
                let _ = input.read(&mut buf);
            }
            // TODO: debug, this doesn't print until after the recv_timeout happens for some reason?
            // info!("{buf:?}");
            tx.send(buf.contains(&b'u'))?;
            Ok(())
        });
        match rx.recv_timeout(Duration::from_millis(500)) {
            Err(RecvTimeoutError::Timeout) => {
                error!(
                    "feature detection took too long to respond, lets just assume your terminal supports the input protocol..."
                );
            }
            Ok(false) => {
                error!("Your terminal doesn't support the 'kitty input protocol'");
                exit(1);
            }
            _ => {}
        }

        let mut lock = io::stdout().lock();
        let fd = lock.as_raw_fd();
        let mut terminfo = Termios::from_fd(fd)?;
        let original = terminfo;
        cfmakeraw(&mut terminfo);
        write!(lock, csi!("?1049h"))?; // switch to alternate screen
        write!(lock, csi!("?25l"))?; // hide cursor
        write!(lock, csi!(">15u"))?; // change keyboard mode
        lock.flush().unwrap();
        tcsetattr(fd, TCSADRAIN, &terminfo)?;
        Ok(Self { original })
    }
}

// TODO: set an atexit handler for this because sometimes it doesnt seem to run?
// TODO: panic hook?
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
