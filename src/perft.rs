//! Perft is a raw legal-move-generation validator, not a search benchmark.

use crate::board::Position;
use crate::moves::Move;

pub fn perft(position: &mut Position, depth: u8) -> u64 {
    if depth == 0 { return 1; }
    let moves = position.legal_moves_mut();
    if depth == 1 { return moves.len() as u64; }
    let mut nodes = 0;
    for mv in moves.iter().copied() { let undo = position.make_move_unchecked(mv); nodes = nodes.saturating_add(perft(position, depth - 1)); position.unmake_move(mv, undo); }
    nodes
}

pub fn divide(position: &mut Position, depth: u8) -> Vec<(Move, u64)> {
    let moves = position.legal_moves_mut(); let mut result = Vec::with_capacity(moves.len());
    for mv in moves.iter().copied() { let undo = position.make_move_unchecked(mv); let nodes = if depth == 0 { 1 } else { perft(position, depth.saturating_sub(1)) }; position.unmake_move(mv, undo); result.push((mv, nodes)); }
    result
}
