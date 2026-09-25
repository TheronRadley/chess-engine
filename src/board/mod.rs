//! Board state, FEN, and deterministic Zobrist hashing.
//!
//! The board deliberately keeps both bitboards and a mailbox. Bitboards are
//! the fast representation for attacks; the mailbox makes captures, notation,
//! and defensive validation unambiguous. Every mutation goes through
//! `add_piece`/`remove_piece`, which maintain both representations together.

mod position;
mod zobrist;

pub use position::{
    Color, FenError, Piece, PieceKind, Position, PositionError, Square, Undo,
    CASTLE_BK, CASTLE_BQ, CASTLE_WK, CASTLE_WQ,
};
pub(crate) use zobrist::keys;
