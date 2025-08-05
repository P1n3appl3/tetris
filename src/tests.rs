use std::time::Instant;

use fumen::{CellColor, Fumen, PieceType, RotationState};

// use pretty_assertions::assert_eq;
use super::{InputEvent::*, *};
use crate::Event;

impl From<PieceType> for Piece {
    fn from(value: PieceType) -> Self {
        use Piece::*;
        match value {
            PieceType::I => I,
            PieceType::L => L,
            PieceType::O => O,
            PieceType::Z => Z,
            PieceType::T => T,
            PieceType::J => J,
            PieceType::S => S,
        }
    }
}

impl From<RotationState> for Rotation {
    fn from(value: RotationState) -> Self {
        use Rotation::*;
        match value {
            RotationState::South => South,
            RotationState::East => East,
            RotationState::North => North,
            RotationState::West => West,
        }
    }
}

impl From<CellColor> for Cell {
    fn from(value: CellColor) -> Self {
        use Piece::*;
        match value {
            CellColor::Empty => Cell::Empty,
            CellColor::I => Cell::Piece(I),
            CellColor::L => Cell::Piece(L),
            CellColor::O => Cell::Piece(O),
            CellColor::Z => Cell::Piece(Z),
            CellColor::T => Cell::Piece(T),
            CellColor::J => Cell::Piece(J),
            CellColor::S => Cell::Piece(S),
            CellColor::Grey => Cell::Garbage,
        }
    }
}

fn get_board(page: &fumen::Page) -> game::Board {
    let mut board: game::Board = [[Cell::Empty; 10]; 50];
    for (row, from) in board.iter_mut().zip(page.field) {
        *row = from.map(Into::into);
    }
    board
}

fn get_piece(page: &fumen::Page) -> (Piece, (i8, i8), Rotation) {
    page.piece
        .map(|p| (p.kind.into(), (p.x as i8 - 1, p.y as i8 + 1), p.rotation.into()))
        .unwrap_or((Piece::I, (3, 20), Rotation::North))
}

#[derive(Eq, PartialEq, Clone)]
struct BoardString(pub String);

impl std::fmt::Display for BoardString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::fmt::Debug for BoardString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn render(board: game::Board, (piece, pos, rotation): (Piece, (i8, i8), Rotation)) -> BoardString {
    let mut s = BoardString("\n----------------------\n".to_owned());
    for y in (0..20).rev() {
        s.0.push('|');
        for x in 0..10 {
            if piece.get_pos(rotation, pos).contains(&(x as i8, y as i8)) {
                s.0.push_str("..");
                continue;
            }
            let next = match board[y][x] {
                Cell::Piece(piece) => format!("{piece:?}{piece:?}"),
                Cell::Garbage => "X ".to_owned(),
                Cell::Empty => "  ".to_owned(),
            };
            s.0.push_str(next.as_str());
        }
        s.0.push_str("|\n");
    }
    s.0.push_str("----------------------");
    s
}

// TODO: show comments in assertion failures?
// TODO: render both boards side by side in tui? with diff? detect tty to change from color to letter based skin
fn run_fumen<T, U, V>(data: &str, events: T)
where
    T: Iterator<Item = U> + ExactSizeIterator,
    U: IntoIterator<Item = V>,
    V: Into<Event>,
{
    let f = Fumen::decode(data).unwrap();
    let c = Config::default();
    let mut g = Game::new(c);
    let t = Instant::now();
    let first = f.pages.first().unwrap();
    g.board = get_board(first);
    let temp = g.current.0;
    g.current = get_piece(first);
    g.upcomming.push_back(temp); // so we don't run out when we harddrop
    println!("{}", render(g.board, g.current));
    assert_eq!(events.len(), f.pages.len() - 1);
    for (i, (events, page)) in events.zip(f.pages[1..].iter()).enumerate() {
        for e in events.into_iter() {
            g.handle(e.into(), t, &NullPlayer);
        }
        assert_eq!(
            render(get_board(page), get_piece(page)),
            render(g.board, g.current),
            "Issue on page {i}:"
        );
        println!("{}", render(g.board, g.current));
    }
}

// use https://fumen.zui.jp/ to open any of the following data strings
#[test]
fn test_fumen() {
    let data = "v115@fgh0RpFeg0Q4RpBeglBewhg0R4CeilxhR4AeBtilxh?R4BeBtwwglxhg0Q4AeBtxwRpwhi0AeBtwwRpJe93mvhCVcf?tlBAAA";
    #[rustfmt::skip]
    let events = [
        [Cw],
        [Cw],
        [Hard],
    ];
    run_fumen(data, events.into_iter());
}

#[test]
fn test_basic() {
    // all in n/e/s/w order
    let strings = [
        // I
        "v115@vhBRQJAAA",
        "v115@vhBJGJAAA",
        "v115@vhBBQJAAA",
        "v115@vhB5GJAAA",
    ];
    for s in strings {
        run_fumen(s, [[Hard]].into_iter());
    }
}

#[test]
fn test_right_side_i_kick() {
    run_fumen("v115@WhR4GeR4Ne5InvhBhxBAAA", [[Ccw], [Hard]].into_iter());
}
