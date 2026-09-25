//! Tapered classical evaluation. It returns a score from the side to move,
//! matching negamax. NNUE is intentionally not approximated here; it is a
//! future extension point behind this module's small public interface.

pub mod params;

use crate::board::{file_of, rank_of, Color, PieceKind, Position, Square};
use crate::movegen::{bishop_attacks, king_attacks, knight_attacks, pawn_attacks, queen_attacks, rook_attacks};
use params::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct EvalBreakdown { pub mg: i32, pub eg: i32, pub phase: i32 }

#[inline] pub fn piece_value(kind: PieceKind) -> i32 { MG_VALUE[kind as usize] }

pub fn evaluate(position: &Position) -> i32 {
    if position.is_insufficient_material() { return 0; }
    let b = evaluate_breakdown(position);
    let blended = (b.mg * b.phase + b.eg * (MAX_PHASE - b.phase)) / MAX_PHASE;
    if position.side_to_move() == Color::White { blended } else { -blended }
}

pub fn evaluate_breakdown(position: &Position) -> EvalBreakdown {
    let mut result = EvalBreakdown::default();
    result.phase = phase(position);
    for color in [Color::White, Color::Black] {
        let sign = if color == Color::White { 1 } else { -1 };
        let (mg, eg) = material_and_pst(position, color); result.mg += sign * mg; result.eg += sign * eg;
        let (mg, eg) = pawn_structure(position, color); result.mg += sign * mg; result.eg += sign * eg;
        let (mg, eg) = mobility(position, color); result.mg += sign * mg; result.eg += sign * eg;
        let (mg, eg) = rook_files(position, color); result.mg += sign * mg; result.eg += sign * eg;
        let (mg, eg) = bishop_pair(position, color); result.mg += sign * mg; result.eg += sign * eg;
        let (mg, eg) = king_safety(position, color); result.mg += sign * mg; result.eg += sign * eg;
        result.mg += sign * space(position, color);
        result.eg += sign * king_activity(position, color);
    }
    result
}

pub fn phase(position: &Position) -> i32 {
    let mut p = 0;
    for color in [Color::White, Color::Black] { for kind in [PieceKind::Knight, PieceKind::Bishop, PieceKind::Rook, PieceKind::Queen] { p += position.pieces(color, kind).count_ones() as i32 * PHASE_VALUE[kind as usize]; } }
    p.min(MAX_PHASE)
}

/// Material and separate MG/EG PST contribution.
pub fn material_and_pst(position: &Position, color: Color) -> (i32, i32) {
    let mut mg = 0; let mut eg = 0;
    for kind in PieceKind::ALL {
        let mut pieces = position.pieces(color, kind);
        while pieces != 0 { let sq = pop_lsb(&mut pieces); mg += MG_VALUE[kind as usize] + pst(kind, sq, color, false); eg += EG_VALUE[kind as usize] + pst(kind, sq, color, true); }
    }
    (mg, eg)
}

fn pst(kind: PieceKind, sq: Square, color: Color, endgame: bool) -> i32 {
    let rank = (if color == Color::White { rank_of(sq) } else { 7 - rank_of(sq) }) as usize;
    let file = file_of(sq) as usize;
    if endgame { EG_RANK[kind as usize][rank] + EG_FILE[file] } else { MG_RANK[kind as usize][rank] + MG_FILE[file] }
}

/// Isolated, doubled, connected/supported, backward, and passed pawn terms.
pub fn pawn_structure(position: &Position, color: Color) -> (i32, i32) {
    let pawns = position.pieces(color, PieceKind::Pawn); let enemy = position.pieces(color.opposite(), PieceKind::Pawn);
    let mut mg = 0; let mut eg = 0;
    for file in 0..8 { let count = (pawns & file_mask(file)).count_ones() as i32; if count > 1 { mg -= DOUBLED_PAWN * (count - 1); eg -= (DOUBLED_PAWN + 3) * (count - 1); } }
    let mut work = pawns;
    while work != 0 {
        let sq = pop_lsb(&mut work); let file = file_of(sq); let rank = rank_of(sq);
        let adjacent = adjacent_file_mask(file);
        if pawns & adjacent == 0 { mg -= ISOLATED_PAWN; eg -= ISOLATED_PAWN / 2; }
        if pawn_attacks(color.opposite(), sq) & pawns != 0 { mg += CONNECTED_PAWN; eg += CONNECTED_PAWN + 2; }
        if is_passed(sq, color, enemy) { let advance = (if color == Color::White { rank } else { 7 - rank }) as usize; mg += PASSED_MG[advance]; eg += PASSED_EG[advance]; if pawn_attacks(color.opposite(), sq) & pawns != 0 { mg += 4; eg += 8; } }
        if is_backward(sq, color, pawns, enemy) { mg -= BACKWARD_PAWN; eg -= BACKWARD_PAWN / 2; }
    }
    (mg, eg)
}
fn is_passed(sq: Square, color: Color, enemy_pawns: u64) -> bool {
    let file = file_of(sq); let rank = rank_of(sq); let files = file_mask(file) | adjacent_file_mask(file);
    let mut candidates = enemy_pawns & files;
    while candidates != 0 { let ep = pop_lsb(&mut candidates); if (color == Color::White && rank_of(ep) > rank) || (color == Color::Black && rank_of(ep) < rank) { return false; } }
    true
}
fn is_backward(sq: Square, color: Color, own: u64, enemy: u64) -> bool {
    if pawn_attacks(color.opposite(), sq) & own != 0 { return false; }
    let forward = if color == Color::White { sq.checked_add(8) } else { sq.checked_sub(8) };
    match forward { Some(front) if enemy & pawn_attacks(color, front) != 0 => true, _ => false }
}

pub fn mobility(position: &Position, color: Color) -> (i32, i32) {
    let own = position.occupancy(color); let occ = position.all_occupancy(); let mut mg = 0; let mut eg = 0;
    for kind in [PieceKind::Knight, PieceKind::Bishop, PieceKind::Rook, PieceKind::Queen] {
        let mut work = position.pieces(color, kind); while work != 0 { let sq = pop_lsb(&mut work); let n = (match kind { PieceKind::Knight => knight_attacks(sq), PieceKind::Bishop => bishop_attacks(sq, occ), PieceKind::Rook => rook_attacks(sq, occ), PieceKind::Queen => queen_attacks(sq, occ), _ => 0 } & !own).count_ones() as i32; mg += n * MOBILITY_MG[kind as usize]; eg += n * MOBILITY_EG[kind as usize]; }
    }
    (mg, eg)
}

pub fn rook_files(position: &Position, color: Color) -> (i32, i32) {
    let own_pawns = position.pieces(color, PieceKind::Pawn); let all_pawns = own_pawns | position.pieces(color.opposite(), PieceKind::Pawn); let mut work = position.pieces(color, PieceKind::Rook); let mut mg = 0;
    while work != 0 { let sq = pop_lsb(&mut work); let file = file_of(sq); if all_pawns & file_mask(file) == 0 { mg += ROOK_OPEN_FILE; } else if own_pawns & file_mask(file) == 0 { mg += ROOK_SEMIOPEN_FILE; } }
    (mg, mg / 2)
}
pub fn bishop_pair(position: &Position, color: Color) -> (i32, i32) { if position.pieces(color, PieceKind::Bishop).count_ones() >= 2 { (BISHOP_PAIR_MG, BISHOP_PAIR_EG) } else { (0, 0) } }

pub fn king_safety(position: &Position, color: Color) -> (i32, i32) {
    let king = match position.king_square(color) { Some(s) => s, None => return (-10_000, -10_000) };
    let pawns = position.pieces(color, PieceKind::Pawn); let enemy = color.opposite(); let mut mg = 0;
    for df in [-1_i8, 0, 1] { let f = file_of(king) as i8 + df; let r = rank_of(king) as i8 + if color == Color::White { 1 } else { -1 }; if (0..8).contains(&f) && (0..8).contains(&r) && pawns & (1_u64 << (r * 8 + f)) != 0 { mg += KING_SHIELD; } }
    for df in [-1_i8, 0, 1] { let f = file_of(king) as i8 + df; if (0..8).contains(&f) && pawns & file_mask(f as u8) == 0 { mg -= KING_OPEN_FILE; } }
    let zone = king_attacks(king) | (1_u64 << king); let mut attacks = 0;
    for kind in [PieceKind::Pawn, PieceKind::Knight, PieceKind::Bishop, PieceKind::Rook, PieceKind::Queen] { let mut work = position.pieces(enemy, kind); while work != 0 { let sq = pop_lsb(&mut work); let a = match kind { PieceKind::Pawn => pawn_attacks(enemy, sq), PieceKind::Knight => knight_attacks(sq), PieceKind::Bishop => bishop_attacks(sq, position.all_occupancy()), PieceKind::Rook => rook_attacks(sq, position.all_occupancy()), PieceKind::Queen => queen_attacks(sq, position.all_occupancy()), _ => 0 }; if a & zone != 0 { attacks += match kind { PieceKind::Queen => 4, PieceKind::Rook => 3, PieceKind::Bishop | PieceKind::Knight => 2, _ => 1 }; } } }
    mg -= attacks * KING_ATTACK_UNIT; (mg, 0)
}

pub fn space(position: &Position, color: Color) -> i32 { let mut result = 0; for sq in [18_u8, 19, 20, 21, 26, 27, 28, 29, 34, 35, 36, 37, 42, 43, 44, 45] { if square_attacked_by(position, sq, color) { result += SPACE_MG; } } result }
pub fn king_activity(position: &Position, color: Color) -> i32 { match position.king_square(color) { Some(sq) => { let d = (file_of(sq) as i32 - 3).abs().min((file_of(sq) as i32 - 4).abs()) + (rank_of(sq) as i32 - 3).abs().min((rank_of(sq) as i32 - 4).abs()); (4 - d).max(0) * KING_CENTER_EG }, None => -10_000 } }

fn square_attacked_by(position: &Position, sq: Square, color: Color) -> bool {
    if pawn_attacks(color.opposite(), sq) & position.pieces(color, PieceKind::Pawn) != 0 { return true; }
    if knight_attacks(sq) & position.pieces(color, PieceKind::Knight) != 0 { return true; }
    if bishop_attacks(sq, position.all_occupancy()) & (position.pieces(color, PieceKind::Bishop) | position.pieces(color, PieceKind::Queen)) != 0 { return true; }
    rook_attacks(sq, position.all_occupancy()) & (position.pieces(color, PieceKind::Rook) | position.pieces(color, PieceKind::Queen)) != 0
}
#[inline] fn pop_lsb(bb: &mut u64) -> Square { let sq = bb.trailing_zeros() as Square; *bb &= *bb - 1; sq }
#[inline] fn file_mask(file: u8) -> u64 { 0x0101_0101_0101_0101_u64 << file }
#[inline] fn adjacent_file_mask(file: u8) -> u64 { (if file > 0 { file_mask(file - 1) } else { 0 }) | (if file < 7 { file_mask(file + 1) } else { 0 }) }
