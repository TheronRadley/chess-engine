use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};
use chess_engine::{Engine, Position, SearchLimits, STARTPOS_FEN};
use chess_engine::search::MATE_THRESHOLD;

#[test]
fn finds_a_forced_mate_and_legal_pv() {
    let mut p = Position::from_fen("7k/5K2/6Q1/8/8/8/8/8 w - - 0 1").unwrap();
    let mut engine = Engine::new(4); let result = engine.search(&mut p, SearchLimits { depth: Some(3), movetime: None, nodes: None, multipv: 1, infinite: false }, None);
    let line = result.lines.first().expect("a legal root move"); assert!(line.score > 29_000, "score was {}", line.score);
    let mut replay = p.clone(); for &mv in &line.pv { replay.try_make_move(mv).expect("PV must replay legally"); }
    // Both Qg7# and Qh5# are valid mate-in-one moves in this position; check
    // the chess property rather than hard-coding an arbitrary move-order tie.
    assert!(replay.is_checkmate(), "PV did not end in checkmate: {:?}", line.pv.iter().map(|m| m.to_uci()).collect::<Vec<_>>());
}

#[test]
fn multipv_is_ordered_unique_and_agrees_with_single_pv() {
    let mut a = Position::from_fen(STARTPOS_FEN).unwrap(); let mut b = a.clone(); let mut engine_a = Engine::new(4); let mut engine_b = Engine::new(4);
    let one = engine_a.search(&mut a, SearchLimits { depth: Some(2), movetime: None, nodes: None, multipv: 1, infinite: false }, None);
    let many = engine_b.search(&mut b, SearchLimits { depth: Some(2), movetime: None, nodes: None, multipv: 3, infinite: false }, None);
    assert_eq!(one.best_move(), many.best_move()); assert!(many.lines.windows(2).all(|w| w[0].score >= w[1].score));
    let mut roots = many.lines.iter().map(|l| l.pv[0]).collect::<Vec<_>>(); roots.sort_by_key(|m| m.raw()); roots.dedup(); assert_eq!(roots.len(), many.lines.len());
}

#[test]
fn time_limited_search_returns_with_bounded_overshoot() {
    let mut p = Position::from_fen(STARTPOS_FEN).unwrap(); let mut e = Engine::new(4); let start = Instant::now(); let result = e.search(&mut p, SearchLimits { depth: None, movetime: Some(Duration::from_millis(30)), nodes: None, multipv: 1, infinite: false }, None); assert!(start.elapsed() < Duration::from_millis(150), "elapsed {:?}", start.elapsed()); assert!(result.best_move().is_some());
}

#[test]
fn tt_mate_scores_are_ply_normalized() {
    // A second search gets to reuse an exact mating entry at a different root
    // traversal ply without changing the chosen mating move.
    let mut p = Position::from_fen("7k/5K2/6Q1/8/8/8/8/8 w - - 0 1").unwrap(); let mut e = Engine::new(4);
    let first = e.search(&mut p, SearchLimits { depth: Some(3), movetime: None, nodes: None, multipv: 1, infinite: false }, None);
    let second = e.search(&mut p, SearchLimits { depth: Some(3), movetime: None, nodes: None, multipv: 1, infinite: false }, None);
    assert_eq!(first.best_move(), second.best_move()); assert!(second.lines[0].score > 29_000);
}

/// A stalemate at the root scores zero (a draw), never a mate or nonzero eval.
#[test]
fn stalemate_at_root_scores_zero() {
    let mut p = Position::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").expect("stalemate FEN");
    assert!(p.is_stalemate() && !p.in_check(chess_engine::Color::Black));
    let mut e = Engine::new(4);
    let result = e.search(&mut p, SearchLimits { depth: Some(2), movetime: None, nodes: None, multipv: 1, infinite: false }, None);
    let score = result.lines.first().expect("a root line").score;
    assert_eq!(score, 0, "stalemate must score zero, got {score}");
}

/// Checkmate takes precedence over the fifty-move clock at the root: a mated
/// side with the halfmove clock at 100 is still mated, not drawn.
#[test]
fn checkmate_at_halfmove_100_returns_mate() {
    let mut p = Position::from_fen("7k/6Q1/6K1/8/8/8/8/8 b - - 100 1").expect("mate FEN");
    assert!(p.is_checkmate() && p.is_fifty_move_draw());
    let mut e = Engine::new(4);
    let result = e.search(&mut p, SearchLimits { depth: Some(2), movetime: None, nodes: None, multipv: 1, infinite: false }, None);
    let score = result.lines.first().expect("a root line").score;
    assert!(score <= -MATE_THRESHOLD, "checkmate must score as mate, got {score}");
}

/// The search honors an external stop signal and reports itself as aborted.
#[test]
fn search_aborts_on_stop() {
    let mut p = Position::from_fen(STARTPOS_FEN).expect("startpos");
    let mut e = Engine::new(4);
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    // Stop partway through by flipping the flag on a short timer.
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1));
        stop_clone.store(true, Ordering::Relaxed);
    });
    let result = e.search(&mut p, SearchLimits { depth: Some(64), movetime: None, nodes: None, multipv: 1, infinite: false }, Some(stop));
    assert!(result.aborted, "search should report aborted when stopped");
    assert!(result.best_move().is_some(), "aborted search must still yield a legal bestmove");
}

/// Repeated go/stop cycles leave the engine able to start a fresh search.
#[test]
fn repeated_gostop_leaves_engine_usable() {
    let mut p = Position::from_fen(STARTPOS_FEN).expect("startpos");
    let mut e = Engine::new(4);
    let stop = Arc::new(AtomicBool::new(false));
    for _ in 0..3 {
        let stop_clone = Arc::clone(&stop);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1));
            stop_clone.store(true, Ordering::Relaxed);
        });
        let result = e.search(&mut p, SearchLimits { depth: Some(64), movetime: None, nodes: None, multipv: 1, infinite: false }, Some(Arc::clone(&stop)));
        assert!(result.aborted);
        stop.store(false, Ordering::Relaxed);
    }
    // A fresh, unstopped search must still complete and produce a move.
    let result = e.search(&mut p, SearchLimits { depth: Some(2), movetime: None, nodes: None, multipv: 1, infinite: false }, None);
    assert!(!result.aborted);
    assert!(result.best_move().is_some());
}
