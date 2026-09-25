//! Attack tables and strictly legal move generation.
//!
//! Sliders use precomputed occupancy-indexed attack tables. This is the same
//! established table-lookup family as magic bitboards, but uses a small,
//! portable software-PEXT index instead of architecture-specific magic/PEXT
//! instructions. It is deterministic on every Rust target and leaves a clean
//! replacement point for BMI2/magic indexing without changing callers.

use std::sync::OnceLock;

use crate::board::{file_of, rank_of, Color, Piece, PieceKind, Position, Square, CASTLE_BK, CASTLE_BQ, CASTLE_WK, CASTLE_WQ};
use crate::moves::{Move, MoveKind};

/// Maximum legal moves is below 256 (the orthodox maximum is 218).
#[derive(Clone)]
pub struct MoveList { moves: [Move; 256], len: usize }
impl Default for MoveList { fn default() -> Self { Self { moves: [Move::NULL; 256], len: 0 } } }
impl MoveList {
    #[inline] pub fn push(&mut self, mv: Move) { debug_assert!(self.len < self.moves.len()); if self.len < self.moves.len() { self.moves[self.len] = mv; self.len += 1; } }
    #[inline] pub fn len(&self) -> usize { self.len }
    #[inline] pub fn is_empty(&self) -> bool { self.len == 0 }
    #[inline] pub fn iter(&self) -> std::slice::Iter<'_, Move> { self.moves[..self.len].iter() }
    #[inline] pub(crate) fn as_mut_slice(&mut self) -> &mut [Move] { &mut self.moves[..self.len] }
}

struct AttackTables {
    knight: [u64; 64], king: [u64; 64], pawn: [[u64; 64]; 2],
    rook_mask: [u64; 64], bishop_mask: [u64; 64],
    rook: Vec<Vec<u64>>, bishop: Vec<Vec<u64>>,
}
fn tables() -> &'static AttackTables { static TABLES: OnceLock<AttackTables> = OnceLock::new(); TABLES.get_or_init(build_tables) }

fn build_tables() -> AttackTables {
    let mut knight = [0; 64]; let mut king = [0; 64]; let mut pawn = [[0; 64]; 2];
    let mut rook_mask = [0; 64]; let mut bishop_mask = [0; 64]; let mut rook = Vec::with_capacity(64); let mut bishop = Vec::with_capacity(64);
    for sq in 0..64_u8 {
        let f = file_of(sq) as i8; let r = rank_of(sq) as i8;
        for (df, dr) in [(1,2),(2,1),(2,-1),(1,-2),(-1,-2),(-2,-1),(-2,1),(-1,2)] { if let Some(t) = offset(f, r, df, dr) { knight[sq as usize] |= bit(t); } }
        for (df, dr) in [(1,1),(1,0),(1,-1),(0,-1),(-1,-1),(-1,0),(-1,1),(0,1)] { if let Some(t) = offset(f, r, df, dr) { king[sq as usize] |= bit(t); } }
        for (df, dr) in [(-1,1),(1,1)] { if let Some(t) = offset(f,r,df,dr) { pawn[Color::White.index()][sq as usize] |= bit(t); } }
        for (df, dr) in [(-1,-1),(1,-1)] { if let Some(t) = offset(f,r,df,dr) { pawn[Color::Black.index()][sq as usize] |= bit(t); } }
        rook_mask[sq as usize] = relevant_mask(sq, &[(1,0),(-1,0),(0,1),(0,-1)]);
        bishop_mask[sq as usize] = relevant_mask(sq, &[(1,1),(1,-1),(-1,1),(-1,-1)]);
        let rm = rook_mask[sq as usize]; let bm = bishop_mask[sq as usize];
        let mut rt = vec![0; 1usize << rm.count_ones()];
        for idx in 0..rt.len() { rt[idx] = ray_attacks(sq, index_occupancy(idx, rm), &[(1,0),(-1,0),(0,1),(0,-1)]); }
        let mut bt = vec![0; 1usize << bm.count_ones()];
        for idx in 0..bt.len() { bt[idx] = ray_attacks(sq, index_occupancy(idx, bm), &[(1,1),(1,-1),(-1,1),(-1,-1)]); }
        rook.push(rt); bishop.push(bt);
    }
    AttackTables { knight, king, pawn, rook_mask, bishop_mask, rook, bishop }
}

#[inline] fn bit(sq: Square) -> u64 { 1_u64 << sq }
fn offset(f: i8, r: i8, df: i8, dr: i8) -> Option<Square> { let nf = f + df; let nr = r + dr; if (0..8).contains(&nf) && (0..8).contains(&nr) { Some((nr * 8 + nf) as Square) } else { None } }
fn relevant_mask(sq: Square, dirs: &[(i8, i8)]) -> u64 {
    // Include every ray square except its final board-edge square. The final
    // square can never be a relevant blocker because no ray continues beyond
    // it. This formulation also handles a rook travelling along rank one.
    let mut mask = 0; let f = file_of(sq) as i8; let r = rank_of(sq) as i8;
    for &(df, dr) in dirs {
        let mut nf = f + df; let mut nr = r + dr;
        while (0..8).contains(&nf) && (0..8).contains(&nr) {
            let next_f = nf + df; let next_r = nr + dr;
            if !(0..8).contains(&next_f) || !(0..8).contains(&next_r) { break; }
            mask |= bit((nr * 8 + nf) as Square);
            nf = next_f; nr = next_r;
        }
    }
    mask
}
fn ray_attacks(sq: Square, blockers: u64, dirs: &[(i8, i8)]) -> u64 {
    let mut attacks = 0; let f = file_of(sq) as i8; let r = rank_of(sq) as i8;
    for &(df, dr) in dirs { let mut nf = f + df; let mut nr = r + dr; while (0..8).contains(&nf) && (0..8).contains(&nr) { let to = (nr * 8 + nf) as Square; attacks |= bit(to); if blockers & bit(to) != 0 { break; } nf += df; nr += dr; } }
    attacks
}
fn index_occupancy(mut index: usize, mut mask: u64) -> u64 { let mut occ = 0; while mask != 0 { let sq = mask.trailing_zeros(); mask &= mask - 1; if index & 1 != 0 { occ |= 1_u64 << sq; } index >>= 1; } occ }
fn compress_occupancy(occ: u64, mut mask: u64) -> usize { let mut index = 0; let mut out_bit = 1; while mask != 0 { let sq = mask.trailing_zeros(); mask &= mask - 1; if occ & (1_u64 << sq) != 0 { index |= out_bit; } out_bit <<= 1; } index }

#[inline] pub fn knight_attacks(sq: Square) -> u64 { tables().knight[sq as usize] }
#[inline] pub fn king_attacks(sq: Square) -> u64 { tables().king[sq as usize] }
#[inline] pub fn pawn_attacks(color: Color, sq: Square) -> u64 { tables().pawn[color.index()][sq as usize] }
#[inline] pub fn rook_attacks(sq: Square, occupied: u64) -> u64 { let t = tables(); t.rook[sq as usize][compress_occupancy(occupied & t.rook_mask[sq as usize], t.rook_mask[sq as usize])] }
#[inline] pub fn bishop_attacks(sq: Square, occupied: u64) -> u64 { let t = tables(); t.bishop[sq as usize][compress_occupancy(occupied & t.bishop_mask[sq as usize], t.bishop_mask[sq as usize])] }
#[inline] pub fn queen_attacks(sq: Square, occupied: u64) -> u64 { rook_attacks(sq, occupied) | bishop_attacks(sq, occupied) }

/// Whether `by` attacks `square`, using the actual occupancy in `position`.
pub fn is_square_attacked(position: &Position, square: Square, by: Color) -> bool {
    let occ = position.all_occupancy();
    if pawn_attacks(by.opposite(), square) & position.pieces(by, PieceKind::Pawn) != 0 { return true; }
    if knight_attacks(square) & position.pieces(by, PieceKind::Knight) != 0 { return true; }
    if king_attacks(square) & position.pieces(by, PieceKind::King) != 0 { return true; }
    if bishop_attacks(square, occ) & (position.pieces(by, PieceKind::Bishop) | position.pieces(by, PieceKind::Queen)) != 0 { return true; }
    rook_attacks(square, occ) & (position.pieces(by, PieceKind::Rook) | position.pieces(by, PieceKind::Queen)) != 0
}

/// Bitboard of pieces of either color attacking `square`.
pub fn attackers_to(position: &Position, square: Square) -> u64 {
    let occ = position.all_occupancy(); let mut result = 0;
    for by in [Color::White, Color::Black] {
        result |= pawn_attacks(by.opposite(), square) & position.pieces(by, PieceKind::Pawn);
        result |= knight_attacks(square) & position.pieces(by, PieceKind::Knight);
        result |= king_attacks(square) & position.pieces(by, PieceKind::King);
        result |= bishop_attacks(square, occ) & (position.pieces(by, PieceKind::Bishop) | position.pieces(by, PieceKind::Queen));
        result |= rook_attacks(square, occ) & (position.pieces(by, PieceKind::Rook) | position.pieces(by, PieceKind::Queen));
    }
    result
}

/// Pseudo-legal generation is kept separate from legal filtering so the simple
/// reference generator below can cross-check the production path. Legal moves
/// are obtained by make/unmake and a king-attack test. This is deliberately
/// conservative: it handles pins, double checks, discovered checks, and the
/// rare illegal en-passant discovered check through one shared invariant.
pub(crate) fn generate_pseudo(position: &Position) -> MoveList {
    let us = position.side_to_move(); let them = us.opposite(); let own = position.occupancy(us); let enemy = position.occupancy(them); let occ = position.all_occupancy();
    let mut list = MoveList::default();
    let mut pawns = position.pieces(us, PieceKind::Pawn);
    while pawns != 0 { let from = pop_lsb(&mut pawns); generate_pawn(position, from, us, enemy, occ, &mut list); }
    for (kind, attacks) in [(PieceKind::Knight, knight_attacks as fn(Square) -> u64), (PieceKind::King, king_attacks as fn(Square) -> u64)] {
        let mut pieces = position.pieces(us, kind);
        while pieces != 0 { let from = pop_lsb(&mut pieces); let mut targets = attacks(from) & !own & !position.pieces(them, PieceKind::King); while targets != 0 { let to = pop_lsb(&mut targets); list.push(Move::new(from, to, if enemy & bit(to) != 0 { MoveKind::Capture } else { MoveKind::Quiet })); } }
    }
    for kind in [PieceKind::Bishop, PieceKind::Rook, PieceKind::Queen] {
        let mut pieces = position.pieces(us, kind);
        while pieces != 0 { let from = pop_lsb(&mut pieces); let mut targets = match kind { PieceKind::Bishop => bishop_attacks(from, occ), PieceKind::Rook => rook_attacks(from, occ), PieceKind::Queen => queen_attacks(from, occ), _ => 0 } & !own & !position.pieces(them, PieceKind::King); while targets != 0 { let to = pop_lsb(&mut targets); list.push(Move::new(from, to, if enemy & bit(to) != 0 { MoveKind::Capture } else { MoveKind::Quiet })); } }
    }
    generate_castles(position, &mut list);
    list
}

fn generate_pawn(position: &Position, from: Square, color: Color, enemy: u64, occupied: u64, list: &mut MoveList) {
    let rank = rank_of(from); let push = color.pawn_push(); let one_i = from as i16 + push as i16;
    if (0..64).contains(&one_i) { let one = one_i as Square; if occupied & bit(one) == 0 { if rank_of(one) == if color == Color::White { 7 } else { 0 } { add_promotions(from, one, list); } else { list.push(Move::new(from, one, MoveKind::Quiet)); if rank == if color == Color::White { 1 } else { 6 } { let two = (from as i16 + 2 * push as i16) as Square; if occupied & bit(two) == 0 { list.push(Move::new(from, two, MoveKind::DoublePawn)); } } } } }
    let mut targets = pawn_attacks(color, from) & enemy;
    while targets != 0 { let to = pop_lsb(&mut targets); if rank_of(to) == if color == Color::White { 7 } else { 0 } { add_promotions(from, to, list); } else { list.push(Move::new(from, to, MoveKind::Capture)); } }
    if let Some(ep) = position.en_passant() { if pawn_attacks(color, from) & bit(ep) != 0 { list.push(Move::new(from, ep, MoveKind::EnPassant)); } }
}
fn add_promotions(from: Square, to: Square, list: &mut MoveList) { for kind in [MoveKind::PromoteQueen, MoveKind::PromoteRook, MoveKind::PromoteBishop, MoveKind::PromoteKnight] { list.push(Move::new(from, to, kind)); } }

fn generate_castles(position: &Position, list: &mut MoveList) {
    let us = position.side_to_move(); let them = us.opposite(); let rights = position.castling_rights(); let occ = position.all_occupancy();
    let (king_sq, k_right, q_right, rook_k, rook_q, between_k, between_q, pass_k, pass_q) = if us == Color::White { (4, CASTLE_WK, CASTLE_WQ, 7, 0, bit(5) | bit(6), bit(1) | bit(2) | bit(3), [4,5,6], [4,3,2]) } else { (60, CASTLE_BK, CASTLE_BQ, 63, 56, bit(61) | bit(62), bit(57) | bit(58) | bit(59), [60,61,62], [60,59,58]) };
    if position.piece_at(king_sq) != Some(Piece { color: us, kind: PieceKind::King }) { return; }
    if rights & k_right != 0 && position.piece_at(rook_k) == Some(Piece { color: us, kind: PieceKind::Rook }) && occ & between_k == 0 && pass_k.iter().all(|&sq| !is_square_attacked(position, sq, them)) { list.push(Move::new(king_sq, king_sq + 2, MoveKind::CastleKing)); }
    if rights & q_right != 0 && position.piece_at(rook_q) == Some(Piece { color: us, kind: PieceKind::Rook }) && occ & between_q == 0 && pass_q.iter().all(|&sq| !is_square_attacked(position, sq, them)) { list.push(Move::new(king_sq, king_sq - 2, MoveKind::CastleQueen)); }
}

/// Production legal generator. A move is never returned unless applying it
/// leaves the moving side's king unattacked. The mutation is fully undone for
/// every candidate, so state-dependent rules such as en-passant are tested in
/// their post-move occupancy rather than by error-prone special cases.
pub fn generate_legal(position: &mut Position) -> MoveList {
    let pseudo = generate_pseudo(position); let side = position.side_to_move(); let mut legal = MoveList::default();
    for mv in pseudo.iter().copied() { let undo = position.make_move_unchecked(mv); let okay = !position.in_check(side); position.unmake_move(mv, undo); if okay { legal.push(mv); } }
    legal
}

/// Deliberately slow test oracle. It copy-makes each pseudo move independently
/// and is used by tests to ensure the allocation-free production make/unmake
/// filter never diverges. It is not called from search/perft.
pub fn reference_legal_moves(position: &Position) -> MoveList {
    let pseudo = generate_pseudo(position); let side = position.side_to_move(); let mut legal = MoveList::default();
    for mv in pseudo.iter().copied() { let mut copy = position.clone(); let _ = copy.make_move_unchecked(mv); if !copy.in_check(side) { legal.push(mv); } }
    legal
}

/// Static exchange evaluation of a legal capture. This deliberately uses legal
/// recaptures (rather than assuming pinned pieces can recapture), which is a
/// correctness-first SEE implementation. The recursive exchange is bounded by
/// the number of pieces that can occupy one square; callers normally use the
/// threshold helper for ordering/pruning rather than evaluating every move.
pub fn see(position: &Position, mv: Move) -> i32 {
    if !position.legal_moves().iter().any(|&candidate| candidate == mv) { return 0; }
    if !mv.is_capture_kind() && position.piece_at(mv.to()).is_none() { return 0; }
    let victim = if mv.kind() == MoveKind::EnPassant { PieceKind::Pawn } else { match position.piece_at(mv.to()) { Some(p) => p.kind, None => return 0 } };
    let mut copy = position.clone(); let _undo = copy.make_move_unchecked(mv); let side = copy.side_to_move();
    piece_value(victim) - see_recapture(&mut copy, mv.to(), side, 0)
}
pub fn see_ge(position: &Position, mv: Move, threshold: i32) -> bool { see(position, mv) >= threshold }
fn see_recapture(position: &mut Position, target: Square, side: Color, ply: u8) -> i32 {
    if ply >= 12 || position.side_to_move() != side { return 0; }
    let target_piece = match position.piece_at(target) { Some(p) => p, None => return 0 };
    let moves = position.legal_moves_mut(); let mut best = 0;
    for mv in moves.iter().copied() {
        if mv.to() != target || !is_capture_move(position, mv) { continue; }
        let undo = position.make_move_unchecked(mv);
        let gain = piece_value(target_piece.kind) - see_recapture(position, target, side.opposite(), ply + 1);
        position.unmake_move(mv, undo); if gain > best { best = gain; }
    }
    best
}
fn is_capture_move(position: &Position, mv: Move) -> bool { mv.kind() == MoveKind::EnPassant || position.piece_at(mv.to()).is_some() }
fn piece_value(kind: PieceKind) -> i32 { match kind { PieceKind::Pawn => 100, PieceKind::Knight => 320, PieceKind::Bishop => 335, PieceKind::Rook => 500, PieceKind::Queen => 950, PieceKind::King => 20_000 } }

#[inline] fn pop_lsb(bb: &mut u64) -> Square { let sq = bb.trailing_zeros() as Square; *bb &= *bb - 1; sq }
