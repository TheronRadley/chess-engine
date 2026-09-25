use std::time::Instant;
use chess_engine::{perft, Position, STARTPOS_FEN};
fn main() { let mut p = Position::from_fen(STARTPOS_FEN).expect("start FEN"); let start = Instant::now(); let nodes = perft::perft(&mut p, 5); let elapsed = start.elapsed(); println!("perft startpos depth 5: {nodes} nodes in {:?} ({:.0} nps)", elapsed, nodes as f64 / elapsed.as_secs_f64()); }
