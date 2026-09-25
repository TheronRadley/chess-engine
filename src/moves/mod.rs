//! Compact move encoding plus strict UCI and SAN conversion.

mod encoding;
mod san;
mod uci_move;

pub use encoding::{Move, MoveKind};
pub use san::{from_san, to_san};
pub use uci_move::{parse_uci_move, MoveError};
