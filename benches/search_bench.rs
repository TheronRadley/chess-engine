use std::time::Instant;
use chess_engine::{Engine, Position, SearchLimits};
fn main() { let mut p = Position::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").expect("Kiwipete FEN"); let mut e = Engine::new(64); let start = Instant::now(); let r = e.search(&mut p, SearchLimits { depth: Some(8), movetime: None, nodes: None, multipv: 1 }, None); println!("search Kiwipete depth {}: {} nodes in {:?} ({} nps), best {:?}", r.depth, r.nodes, start.elapsed(), r.nps, r.best_move().map(|m| m.to_uci())); }
