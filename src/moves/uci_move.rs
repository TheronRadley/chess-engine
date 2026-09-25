use std::fmt;

use crate::board::{PieceKind, Position};
use crate::moves::Move;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveError(pub String);
impl fmt::Display for MoveError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "invalid move: {}", self.0) } }
impl std::error::Error for MoveError {}

/// Parses UCI long algebraic notation by matching against legal moves. This is
/// deliberately stricter than decoding coordinates alone: malformed promotion
/// suffixes and geometrically plausible but illegal moves are rejected.
pub fn parse_uci_move(position: &Position, text: &str) -> Result<Move, MoveError> {
    let b = text.as_bytes();
    if b.len() != 4 && b.len() != 5 { return Err(MoveError("UCI move must have four coordinates plus an optional promotion letter".into())); }
    // UCI is ASCII. Work on bytes so invalid UTF-8 boundaries can never panic.
    let square_from_bytes = |x: &[u8]| -> Result<u8, MoveError> {
        if x.len() != 2 || !(b'a'..=b'h').contains(&x[0]) || !(b'1'..=b'8').contains(&x[1]) {
            return Err(MoveError("coordinates must be lower-case squares such as e2".into()));
        }
        Ok((x[0] - b'a') + 8 * (x[1] - b'1'))
    };
    let from = square_from_bytes(&b[0..2])?;
    let to = square_from_bytes(&b[2..4])?;
    let promotion = if b.len() == 5 {
        let c = b[4] as char;
        PieceKind::from_promotion(c).ok_or_else(|| MoveError("promotion suffix must be q, r, b, or n".into()))?
    } else { PieceKind::King }; // sentinel that cannot be a promotion
    let mut found = None;
    for mv in position.legal_moves().iter().copied() {
        if mv.from() == from && mv.to() == to {
            let matches = match mv.promotion() { Some(p) => b.len() == 5 && p == promotion, None => b.len() == 4 };
            if matches { found = Some(mv); break; }
        }
    }
    found.ok_or_else(|| MoveError(format!("`{text}` is not legal in the supplied position")))
}
