//! Single-threaded iterative-deepening PVS search.
//!
//! Search state is owned by `Engine`, not global, so a future Lazy-SMP layer
//! can give each worker its own stack/history while sharing or sharding a TT.

use std::sync::{Arc, atomic::{AtomicBool, Ordering as AtomicOrdering}};
use std::time::{Duration, Instant};

use crate::board::{Color, PieceKind, Position};
use crate::eval::{evaluate, piece_value};
use crate::movegen::MoveList;
use crate::moves::{Move, MoveKind};
use crate::tt::{Bound, TranspositionTable};

pub const MATE_SCORE: i32 = 30_000;
pub const MATE_THRESHOLD: i32 = 29_000;
const INF: i32 = 31_000;
const MAX_PLY: usize = 128;
const ASPIRATION: i32 = 40;
const FUTILITY_MARGIN_PER_PLY: i32 = 95;

#[derive(Clone, Debug)]
pub struct SearchLimits {
    pub depth: Option<u8>,
    pub movetime: Option<Duration>,
    pub nodes: Option<u64>,
    pub multipv: usize,
}
impl Default for SearchLimits { fn default() -> Self { Self { depth: Some(8), movetime: None, nodes: None, multipv: 1 } } }

#[derive(Clone, Debug)]
pub struct SearchLine { pub rank: usize, pub score: i32, pub depth: u8, pub pv: Vec<Move> }
#[derive(Clone, Debug)]
pub struct SearchResult { pub lines: Vec<SearchLine>, pub depth: u8, pub seldepth: u8, pub nodes: u64, pub elapsed: Duration, pub nps: u64, pub hashfull: u16, pub aborted: bool, pub tt_hits: u64 }
impl SearchResult { pub fn best_move(&self) -> Option<Move> { self.lines.first().and_then(|l| l.pv.first()).copied() } }

pub struct Engine { tt: TranspositionTable, hash_mb: usize, default_multipv: usize }
impl Default for Engine { fn default() -> Self { Self::new(16) } }
impl Engine {
    pub fn new(hash_mb: usize) -> Self { let mb = hash_mb.clamp(1, 4096); Self { tt: TranspositionTable::new(mb), hash_mb: mb, default_multipv: 1 } }
    pub fn set_hash_mb(&mut self, mb: usize) { self.hash_mb = mb.clamp(1, 4096); self.tt.resize(self.hash_mb); }
    pub fn hash_mb(&self) -> usize { self.hash_mb }
    pub fn set_default_multipv(&mut self, multipv: usize) { self.default_multipv = multipv.clamp(1, 5); }
    pub fn clear_hash(&mut self) { self.tt.clear(); }
    pub fn search(&mut self, position: &mut Position, mut limits: SearchLimits, stop: Option<Arc<AtomicBool>>) -> SearchResult {
        if limits.multipv == 0 { limits.multipv = self.default_multipv; }
        limits.multipv = limits.multipv.clamp(1, 5);
        let max_depth = limits.depth.unwrap_or(64).min(64);
        self.tt.new_search();
        let start = Instant::now();
        // Two milliseconds protects the requested hard wall on ordinary clocks.
        let deadline = limits.movetime.map(|d| start + d.saturating_sub(Duration::from_millis(2)));
        let mut searcher = Searcher::new(&mut self.tt, deadline, limits.nodes, stop);
        let mut completed: Vec<SearchLine> = Vec::new(); let mut completed_depth = 0_u8; let mut previous = 0;
        for depth in 1..=max_depth {
            if searcher.should_abort() { break; }
            let lines = if limits.multipv == 1 {
                let mut window = if completed_depth == 0 { INF } else { ASPIRATION };
                let mut alpha = if completed_depth == 0 { -INF } else { previous - window };
                let mut beta = if completed_depth == 0 { INF } else { previous + window };
                loop {
                    let line = searcher.root_single(position, depth as i32, alpha, beta);
                    if searcher.aborted { break None; }
                    let line = match line { Some(v) => v, None => break None };
                    if line.score <= alpha && alpha > -INF + 1 { window = window.saturating_mul(2); alpha = (previous - window).max(-INF); continue; }
                    if line.score >= beta && beta < INF - 1 { window = window.saturating_mul(2); beta = (previous + window).min(INF); continue; }
                    break Some(vec![line]);
                }
            } else { searcher.root_multi(position, depth as i32, limits.multipv) };
            if searcher.aborted { break; }
            if let Some(mut lines) = lines {
                for (i, line) in lines.iter_mut().enumerate() { line.rank = i + 1; line.depth = depth; }
                if let Some(top) = lines.first() { previous = top.score; }
                completed = lines; completed_depth = depth;
            } else { break; }
        }
        // At depth zero (e.g. movetime=0), still return a legal deterministic
        // fallback instead of a corrupt/empty arbitrary move.
        if completed.is_empty() {
            let fallback = position.legal_moves().iter().copied().next();
            if let Some(mv) = fallback { completed.push(SearchLine { rank: 1, score: 0, depth: 0, pv: vec![mv] }); }
        }
        let elapsed = start.elapsed(); let nodes = searcher.nodes; let seldepth = searcher.seldepth as u8; let aborted = searcher.aborted; let tt_hits = searcher.tt_hits;
        // Release the mutable TT borrow before sampling occupancy for UCI.
        drop(searcher); let hashfull = self.tt.hashfull();
        SearchResult { lines: completed, depth: completed_depth, seldepth, nodes, elapsed, nps: if elapsed.as_nanos() == 0 { 0 } else { (nodes as u128 * 1_000_000_000 / elapsed.as_nanos()) as u64 }, hashfull, aborted, tt_hits }
    }
}

struct Searcher<'a> {
    tt: &'a mut TranspositionTable, deadline: Option<Instant>, node_limit: Option<u64>, stop: Option<Arc<AtomicBool>>, aborted: bool,
    nodes: u64, seldepth: usize, tt_hits: u64,
    killers: [[Move; 2]; MAX_PLY], history: Box<[[[i32; 64]; 64]; 2]>,
    pv: [[Move; MAX_PLY]; MAX_PLY], pv_len: [usize; MAX_PLY],
}
impl<'a> Searcher<'a> {
    fn new(tt: &'a mut TranspositionTable, deadline: Option<Instant>, node_limit: Option<u64>, stop: Option<Arc<AtomicBool>>) -> Self {
        Self { tt, deadline, node_limit, stop, aborted: false, nodes: 0, seldepth: 0, tt_hits: 0, killers: [[Move::NULL; 2]; MAX_PLY], history: Box::new([[[0; 64]; 64]; 2]), pv: [[Move::NULL; MAX_PLY]; MAX_PLY], pv_len: [0; MAX_PLY] }
    }
    fn should_abort(&mut self) -> bool {
        if self.aborted { return true; }
        if let Some(limit) = self.node_limit { if self.nodes >= limit { self.aborted = true; return true; } }
        // Periodic checks make the branch essentially free at shallow nodes,
        // while checking at root ensures zero-movetime exits immediately.
        if self.nodes & 2047 == 0 {
            if self.stop.as_ref().map(|s| s.load(AtomicOrdering::Relaxed)).unwrap_or(false) || self.deadline.map(|d| Instant::now() >= d).unwrap_or(false) { self.aborted = true; return true; }
        }
        false
    }
    fn root_single(&mut self, position: &mut Position, depth: i32, mut alpha: i32, beta: i32) -> Option<SearchLine> {
        self.pv_len[0] = 0;
        if position.is_draw() { return Some(SearchLine { rank: 1, score: 0, depth: depth as u8, pv: position.legal_moves().iter().copied().take(1).collect() }); }
        let mut moves = position.legal_moves_mut();
        if moves.is_empty() { return Some(SearchLine { rank: 1, score: if position.in_check(position.side_to_move()) { -MATE_SCORE } else { 0 }, depth: depth as u8, pv: Vec::new() }); }
        let key = position.key(); let tt_move = self.tt.probe(key).map(|e| e.best).unwrap_or(Move::NULL); self.order_moves(position, &mut moves, tt_move, 0);
        let original_alpha = alpha; let mut best = Move::NULL; let mut best_pv = Vec::new();
        for (index, mv) in moves.iter().copied().enumerate() {
            if self.should_abort() { return None; }
            let undo = position.make_move_unchecked(mv);
            let mut score;
            if index == 0 { score = -self.negamax(position, depth - 1, -beta, -alpha, 1, true, true); }
            else { score = -self.negamax(position, depth - 1, -alpha - 1, -alpha, 1, false, true); if score > alpha && score < beta && !self.aborted { score = -self.negamax(position, depth - 1, -beta, -alpha, 1, true, true); } }
            let child = self.pv_at(1); position.unmake_move(mv, undo);
            if self.aborted { return None; }
            if score > alpha { alpha = score; best = mv; best_pv.clear(); best_pv.push(mv); best_pv.extend(child); if alpha >= beta { break; } }
        }
        if best.is_null() { // fail-low window: retain a legal line for aspiration widening.
            let mv = *moves.iter().next()?; best = mv; best_pv.push(mv);
        }
        self.tt.store(key, depth as i16, score_to_tt(alpha, 0), if alpha <= original_alpha { Bound::Upper } else if alpha >= beta { Bound::Lower } else { Bound::Exact }, best);
        Some(SearchLine { rank: 1, score: alpha, depth: depth as u8, pv: best_pv })
    }
    fn root_multi(&mut self, position: &mut Position, depth: i32, count: usize) -> Option<Vec<SearchLine>> {
        if position.is_draw() { return Some(vec![SearchLine { rank: 1, score: 0, depth: depth as u8, pv: position.legal_moves().iter().copied().take(1).collect() }]); }
        let mut moves = position.legal_moves_mut(); if moves.is_empty() { return Some(vec![SearchLine { rank: 1, score: if position.in_check(position.side_to_move()) { -MATE_SCORE } else { 0 }, depth: depth as u8, pv: Vec::new() }]); }
        let tt_move = self.tt.probe(position.key()).map(|e| e.best).unwrap_or(Move::NULL); self.order_moves(position, &mut moves, tt_move, 0);
        let mut lines = Vec::with_capacity(moves.len());
        for mv in moves.iter().copied() {
            if self.should_abort() { return None; }
            let undo = position.make_move_unchecked(mv); let score = -self.negamax(position, depth - 1, -INF, INF, 1, true, true); let child = self.pv_at(1); position.unmake_move(mv, undo);
            if self.aborted { return None; } let mut pv = Vec::with_capacity(child.len() + 1); pv.push(mv); pv.extend(child); lines.push(SearchLine { rank: 0, score, depth: depth as u8, pv });
        }
        lines.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.pv[0].raw().cmp(&b.pv[0].raw()))); lines.truncate(count.min(lines.len())); Some(lines)
    }
    fn negamax(&mut self, position: &mut Position, mut depth: i32, mut alpha: i32, beta: i32, ply: usize, pv_node: bool, allow_null: bool) -> i32 {
        self.pv_len[ply] = 0; self.nodes = self.nodes.saturating_add(1); self.seldepth = self.seldepth.max(ply);
        if self.should_abort() { return 0; }
        if ply >= MAX_PLY - 1 { return evaluate(position); }
        if ply > 0 && position.is_draw() { return 0; }
        let in_check = position.in_check(position.side_to_move()); if in_check { depth += 1; }
        if depth <= 0 { return self.quiescence(position, alpha, beta, ply); }
        let key = position.key(); let original_alpha = alpha; let mut tt_move = Move::NULL;
        if let Some(entry) = self.tt.probe(key) {
            self.tt_hits += 1; tt_move = entry.best; let score = score_from_tt(entry.score, ply);
            if entry.depth as i32 >= depth && !pv_node { match entry.bound { Bound::Exact => return score, Bound::Lower if score >= beta => return score, Bound::Upper if score <= alpha => return score, _ => {} } }
        }
        // Null-move pruning is disabled in pawn-only endings to avoid the
        // standard zugzwang failure mode. A verification search is implicit in
        // the reduced null window; no game state is exposed to callers.
        if allow_null && !pv_node && !in_check && depth >= 3 && position.has_non_pawn_material(position.side_to_move()) {
            let undo = position.make_null_move(); let reduction = 2 + depth / 6; let score = -self.negamax(position, depth - 1 - reduction, -beta, -beta + 1, ply + 1, false, false); position.unmake_null_move(undo);
            if self.aborted { return 0; }
            if score >= beta {
                // Verification without another null move is important in
                // zugzwang-like cases that slipped through the material guard.
                let verified = self.negamax(position, depth - 1 - reduction, beta - 1, beta, ply, false, false);
                if self.aborted { return 0; }
                if verified >= beta { return verified; }
            }
        }
        let static_eval = evaluate(position); let mut moves = position.legal_moves_mut();
        if moves.is_empty() { return if in_check { -MATE_SCORE + ply as i32 } else { 0 }; }
        self.order_moves(position, &mut moves, tt_move, ply); let mut best = Move::NULL;
        for (index, mv) in moves.iter().copied().enumerate() {
            let capture = is_capture(position, mv); let quiet = !capture && !mv.is_promotion();
            if !pv_node && !in_check && depth <= 2 && quiet && static_eval + FUTILITY_MARGIN_PER_PLY * depth <= alpha { continue; }
            let undo = position.make_move_unchecked(mv); let child_depth = depth - 1; let mut score;
            let reduce = quiet && !in_check && depth >= 3 && index >= 4 && mv != self.killers[ply][0] && mv != self.killers[ply][1];
            if index == 0 { score = -self.negamax(position, child_depth, -beta, -alpha, ply + 1, pv_node, true); }
            else {
                if reduce { let reduction = 1 + (index / 12) as i32; score = -self.negamax(position, (child_depth - reduction).max(0), -alpha - 1, -alpha, ply + 1, false, true); }
                else { score = -self.negamax(position, child_depth, -alpha - 1, -alpha, ply + 1, false, true); }
                if score > alpha && !self.aborted { score = -self.negamax(position, child_depth, -beta, -alpha, ply + 1, pv_node, true); }
            }
            let child = self.pv_at(ply + 1); position.unmake_move(mv, undo);
            if self.aborted { return 0; }
            if score > alpha { alpha = score; best = mv; self.set_pv(ply, mv, &child); if alpha >= beta { if quiet { self.record_cutoff(position.side_to_move(), mv, ply, depth); } break; } }
        }
        let bound = if alpha <= original_alpha { Bound::Upper } else if alpha >= beta { Bound::Lower } else { Bound::Exact }; self.tt.store(key, depth as i16, score_to_tt(alpha, ply), bound, best); alpha
    }
    fn quiescence(&mut self, position: &mut Position, mut alpha: i32, beta: i32, ply: usize) -> i32 {
        self.pv_len[ply] = 0; self.nodes = self.nodes.saturating_add(1); self.seldepth = self.seldepth.max(ply); if self.should_abort() { return 0; }
        if ply >= MAX_PLY - 1 || position.is_draw() { return if position.is_draw() { 0 } else { evaluate(position) }; }
        let in_check = position.in_check(position.side_to_move()); let stand = evaluate(position);
        if !in_check { if stand >= beta { return stand; } if stand > alpha { alpha = stand; } }
        let mut moves = position.legal_moves_mut(); if in_check && moves.is_empty() { return -MATE_SCORE + ply as i32; }
        self.order_moves(position, &mut moves, Move::NULL, ply);
        for mv in moves.iter().copied() {
            let capture = is_capture(position, mv); if !in_check && !capture && !mv.is_promotion() { continue; }
            // Delta pruning can only discard non-promotion captures when even a
            // queen swing cannot lift stand-pat to alpha.
            if !in_check && !mv.is_promotion() && stand + 975 < alpha { continue; }
            let undo = position.make_move_unchecked(mv); let score = -self.quiescence(position, -beta, -alpha, ply + 1); let child = self.pv_at(ply + 1); position.unmake_move(mv, undo); if self.aborted { return 0; }
            if score > alpha { alpha = score; self.set_pv(ply, mv, &child); if alpha >= beta { break; } }
        }
        alpha
    }
    fn order_moves(&mut self, position: &Position, moves: &mut MoveList, tt_move: Move, ply: usize) {
        // Insertion sort has low overhead for chess's short lists and keeps tie
        // handling deterministic across platforms.
        let slice = moves.as_mut_slice(); let mut scores = [i32::MIN; 256];
        for (i, &mv) in slice.iter().enumerate() { scores[i] = self.move_score(position, mv, tt_move, ply); }
        for i in 1..slice.len() { let mv = slice[i]; let score = scores[i]; let mut j = i; while j > 0 && (score > scores[j - 1] || (score == scores[j - 1] && mv.raw() < slice[j - 1].raw())) { slice[j] = slice[j - 1]; scores[j] = scores[j - 1]; j -= 1; } slice[j] = mv; scores[j] = score; }
    }
    fn move_score(&self, position: &Position, mv: Move, tt_move: Move, ply: usize) -> i32 {
        if mv == tt_move { return 2_000_000; }
        if is_capture(position, mv) { let attacker = position.piece_at(mv.from()).map(|p| piece_value(p.kind)).unwrap_or(0); let victim = if mv.kind() == MoveKind::EnPassant { piece_value(PieceKind::Pawn) } else { position.piece_at(mv.to()).map(|p| piece_value(p.kind)).unwrap_or(0) }; // Exact legal SEE is intentionally limited to the upper tree where it has the largest ordering payoff; deeper nodes retain the MVV-LVA fallback.
            let see_good = if ply < 2 { crate::movegen::see_ge(position, mv, 0) } else { victim >= attacker }; return 1_000_000 + victim * 16 - attacker + if see_good { 12_000 } else { -12_000 }; }
        if mv == self.killers[ply][0] { return 800_000; } if mv == self.killers[ply][1] { return 799_000; }
        let color = position.side_to_move().index(); 100_000 + self.history[color][mv.from() as usize][mv.to() as usize]
    }
    fn record_cutoff(&mut self, color: Color, mv: Move, ply: usize, depth: i32) { if self.killers[ply][0] != mv { self.killers[ply][1] = self.killers[ply][0]; self.killers[ply][0] = mv; } let h = &mut self.history[color.index()][mv.from() as usize][mv.to() as usize]; *h = (*h + depth * depth).min(30_000); }
    fn pv_at(&self, ply: usize) -> Vec<Move> { self.pv[ply][..self.pv_len[ply]].to_vec() }
    fn set_pv(&mut self, ply: usize, mv: Move, child: &[Move]) { self.pv[ply][0] = mv; let n = child.len().min(MAX_PLY - ply - 1); self.pv[ply][1..=n].copy_from_slice(&child[..n]); self.pv_len[ply] = n + 1; }
}

fn is_capture(position: &Position, mv: Move) -> bool { mv.kind() == MoveKind::EnPassant || position.piece_at(mv.to()).is_some() }
fn score_to_tt(score: i32, ply: usize) -> i32 { if score > MATE_THRESHOLD { score + ply as i32 } else if score < -MATE_THRESHOLD { score - ply as i32 } else { score } }
fn score_from_tt(score: i32, ply: usize) -> i32 { if score > MATE_THRESHOLD { score - ply as i32 } else if score < -MATE_THRESHOLD { score + ply as i32 } else { score } }
pub fn format_score(score: i32) -> String { if score >= MATE_THRESHOLD { format!("mate {}", (MATE_SCORE - score + 1) / 2) } else if score <= -MATE_THRESHOLD { format!("mate -{}", (MATE_SCORE + score + 1) / 2) } else { format!("cp {score}") } }
