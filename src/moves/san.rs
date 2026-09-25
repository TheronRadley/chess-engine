use crate::board::{file_of, rank_of, square_name, PieceKind, Position};
use crate::moves::{Move, MoveError, MoveKind};

/// Formats a legal move in Standard Algebraic Notation, including ambiguity and
/// check/checkmate suffixes. A cloned post-move position is used only for
/// notation; the hot search path never formats SAN.
pub fn to_san(position: &Position, mv: Move) -> Result<String, MoveError> {
    let legal = position.legal_moves();
    if !legal.iter().any(|&candidate| candidate == mv) { return Err(MoveError("cannot format an illegal move as SAN".into())); }
    let piece = position.piece_at(mv.from()).ok_or_else(|| MoveError("move origin is empty".into()))?;
    let mut san = match mv.kind() {
        MoveKind::CastleKing => "O-O".to_owned(),
        MoveKind::CastleQueen => "O-O-O".to_owned(),
        _ => {
            let capture = mv.kind() == MoveKind::EnPassant || position.piece_at(mv.to()).is_some();
            let mut s = String::new();
            if piece.kind != PieceKind::Pawn {
                s.push(piece.kind.san_letter());
                let rivals: Vec<Move> = legal.iter().copied().filter(|other| {
                    *other != mv && other.to() == mv.to() && position.piece_at(other.from()).map(|p| p.kind == piece.kind && p.color == piece.color).unwrap_or(false)
                }).collect();
                if !rivals.is_empty() {
                    let same_file = rivals.iter().any(|other| file_of(other.from()) == file_of(mv.from()));
                    let same_rank = rivals.iter().any(|other| rank_of(other.from()) == rank_of(mv.from()));
                    if !same_file { s.push((b'a' + file_of(mv.from())) as char); }
                    else if !same_rank { s.push((b'1' + rank_of(mv.from())) as char); }
                    else { s.push((b'a' + file_of(mv.from())) as char); s.push((b'1' + rank_of(mv.from())) as char); }
                }
            } else if capture { s.push((b'a' + file_of(mv.from())) as char); }
            if capture { s.push('x'); }
            s.push_str(&square_name(mv.to()));
            if let Some(promoted) = mv.promotion() { s.push('='); s.push(promoted.san_letter()); }
            s
        }
    };
    let mut after = position.clone();
    let _undo = after.try_make_move(mv).map_err(|e| MoveError(e.0))?;
    if after.in_check(after.side_to_move()) {
        if after.legal_moves().is_empty() { san.push('#'); } else { san.push('+'); }
    }
    Ok(san)
}

/// Strict SAN parsing is intentionally implemented by matching the canonical
/// SAN forms of legal moves. This avoids accepting ambiguous shorthand or a
/// notation string describing an illegal check.
pub fn from_san(position: &Position, input: &str) -> Result<Move, MoveError> {
    let normalized = input.trim().replace('0', "O");
    let mut candidate = None;
    for mv in position.legal_moves().iter().copied() {
        let san = to_san(position, mv)?;
        if san == normalized {
            if candidate.replace(mv).is_some() { return Err(MoveError("SAN is ambiguous".into())); }
        }
    }
    candidate.ok_or_else(|| MoveError(format!("`{input}` is not canonical legal SAN in this position")))
}
