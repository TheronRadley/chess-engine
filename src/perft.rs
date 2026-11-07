//! Perft is a raw legal-move-generation validator, not a search benchmark.

use crate::board::Position;
use crate::moves::Move;

pub fn perft(position: &mut Position, depth: u8) -> u64 {
    if depth == 0 { return 1; }
    let moves = position.legal_moves_mut();
    if depth == 1 { return moves.len() as u64; }
    let mut nodes: u64 = 0;
    for mv in moves.iter().copied() { let undo = position.make_move_unchecked(mv); nodes = nodes.saturating_add(perft(position, depth - 1)); position.unmake_move(mv, undo); }
    nodes
}

/// Breaks the perft count down by root move. At depth zero there are no root
/// moves to divide (the position itself is the single `perft(0) == 1` node),
/// so the breakdown is empty rather than a list of `1`s that would sum to the
/// move count instead of to one.
pub fn divide(position: &mut Position, depth: u8) -> Vec<(Move, u64)> {
    let moves = position.legal_moves_mut(); let mut result = Vec::with_capacity(moves.len());
    if depth == 0 { return result; }
    for mv in moves.iter().copied() { let undo = position.make_move_unchecked(mv); let nodes = perft(position, depth.saturating_sub(1)); position.unmake_move(mv, undo); result.push((mv, nodes)); }
    result
}
