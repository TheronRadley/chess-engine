use std::time::{Duration, Instant};
use chess_engine::{Engine, Position, SearchLimits, STARTPOS_FEN};

#[test]
fn finds_a_forced_mate_and_legal_pv() {
    let mut p = Position::from_fen("7k/5K2/6Q1/8/8/8/8/8 w - - 0 1").unwrap();
    let mut engine = Engine::new(4); let result = engine.search(&mut p, SearchLimits { depth: Some(3), movetime: None, nodes: None, multipv: 1 }, None);
    let line = result.lines.first().expect("a legal root move"); assert!(line.score > 29_000, "score was {}", line.score); assert_eq!(line.pv[0].to_uci(), "g6g7");
    let mut replay = p.clone(); for &mv in &line.pv { replay.try_make_move(mv).expect("PV must replay legally"); }
}

#[test]
fn multipv_is_ordered_unique_and_agrees_with_single_pv() {
    let mut a = Position::from_fen(STARTPOS_FEN).unwrap(); let mut b = a.clone(); let mut engine_a = Engine::new(4); let mut engine_b = Engine::new(4);
    let one = engine_a.search(&mut a, SearchLimits { depth: Some(2), movetime: None, nodes: None, multipv: 1 }, None);
    let many = engine_b.search(&mut b, SearchLimits { depth: Some(2), movetime: None, nodes: None, multipv: 3 }, None);
    assert_eq!(one.best_move(), many.best_move()); assert!(many.lines.windows(2).all(|w| w[0].score >= w[1].score));
    let mut roots = many.lines.iter().map(|l| l.pv[0]).collect::<Vec<_>>(); roots.sort_by_key(|m| m.raw()); roots.dedup(); assert_eq!(roots.len(), many.lines.len());
}

#[test]
fn time_limited_search_returns_with_bounded_overshoot() {
    let mut p = Position::from_fen(STARTPOS_FEN).unwrap(); let mut e = Engine::new(4); let start = Instant::now(); let result = e.search(&mut p, SearchLimits { depth: None, movetime: Some(Duration::from_millis(30)), nodes: None, multipv: 1 }, None); assert!(start.elapsed() < Duration::from_millis(150), "elapsed {:?}", start.elapsed()); assert!(result.best_move().is_some());
}

#[test]
fn tt_mate_scores_are_ply_normalized() {
    // A second search gets to reuse an exact mating entry at a different root
    // traversal ply without changing the chosen mating move.
    let mut p = Position::from_fen("7k/5K2/6Q1/8/8/8/8/8 w - - 0 1").unwrap(); let mut e = Engine::new(4);
    let first = e.search(&mut p, SearchLimits { depth: Some(3), movetime: None, nodes: None, multipv: 1 }, None);
    let second = e.search(&mut p, SearchLimits { depth: Some(3), movetime: None, nodes: None, multipv: 1 }, None);
    assert_eq!(first.best_move(), second.best_move()); assert!(second.lines[0].score > 29_000);
}
