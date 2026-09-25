use std::time::Instant;
use chess_engine::{Engine, Position, SearchLimits};

fn main() {
    // A compact forced-mate tactical position verifies that iterative search can
    // reach depth 12 without a broad opening-tree benchmark dominating CI time.
    let mut p = Position::from_fen("7k/5K2/6Q1/8/8/8/8/8 w - - 0 1").expect("tactical FEN");
    let mut e = Engine::new(64); let start = Instant::now();
    let r = e.search(&mut p, SearchLimits { depth: Some(12), movetime: None, nodes: None, multipv: 1 }, None);
    println!("search tactical mate depth {}: {} nodes in {:?} ({} nps), best {:?}", r.depth, r.nodes, start.elapsed(), r.nps, r.best_move().map(|m| m.to_uci()));
}
