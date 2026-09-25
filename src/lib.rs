//! `chess_engine` is the shared, allocation-conscious core used by both binaries.
//!
//! Squares are numbered little-endian rank-file: `a1 == 0`, `h1 == 7`, and
//! `a8 == 56`.  This makes White pawn pushes `+8` and Black pawn pushes `-8`.

pub mod board;
pub mod eval;
pub mod movegen;
pub mod moves;
pub mod perft;
pub mod search;
pub mod tt;
pub mod uci;

pub use board::{Color, FenError, Piece, PieceKind, Position, PositionError, Square};
pub use moves::{Move, MoveError};
pub use search::{Engine, SearchLimits, SearchResult};

/// Standard initial chess position.
pub const STARTPOS_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
