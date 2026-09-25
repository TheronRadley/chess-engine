use std::fmt;

use crate::board::keys;
use crate::moves::{Move, MoveKind};

pub type Square = u8;

pub const CASTLE_WK: u8 = 1;
pub const CASTLE_WQ: u8 = 2;
pub const CASTLE_BK: u8 = 4;
pub const CASTLE_BQ: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Color { White = 0, Black = 1 }

impl Color {
    #[inline] pub const fn opposite(self) -> Self { match self { Self::White => Self::Black, Self::Black => Self::White } }
    #[inline] pub const fn index(self) -> usize { self as usize }
    #[inline] pub const fn pawn_push(self) -> i8 { match self { Self::White => 8, Self::Black => -8 } }
    #[inline] pub const fn home_rank(self) -> u8 { match self { Self::White => 0, Self::Black => 7 } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PieceKind { Pawn = 0, Knight = 1, Bishop = 2, Rook = 3, Queen = 4, King = 5 }

impl PieceKind {
    pub const ALL: [PieceKind; 6] = [Self::Pawn, Self::Knight, Self::Bishop, Self::Rook, Self::Queen, Self::King];
    pub fn from_promotion(ch: char) -> Option<Self> {
        match ch.to_ascii_lowercase() { 'q' => Some(Self::Queen), 'r' => Some(Self::Rook), 'b' => Some(Self::Bishop), 'n' => Some(Self::Knight), _ => None }
    }
    pub const fn san_letter(self) -> char { match self { Self::Pawn => ' ', Self::Knight => 'N', Self::Bishop => 'B', Self::Rook => 'R', Self::Queen => 'Q', Self::King => 'K' } }
    pub const fn fen_letter(self) -> char { match self { Self::Pawn => 'p', Self::Knight => 'n', Self::Bishop => 'b', Self::Rook => 'r', Self::Queen => 'q', Self::King => 'k' } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Piece { pub color: Color, pub kind: PieceKind }

impl Piece {
    #[inline] pub const fn index(self) -> usize { self.color as usize * 6 + self.kind as usize }
    pub fn from_fen(ch: char) -> Option<Self> {
        let color = if ch.is_ascii_uppercase() { Color::White } else { Color::Black };
        let kind = match ch.to_ascii_lowercase() { 'p' => PieceKind::Pawn, 'n' => PieceKind::Knight, 'b' => PieceKind::Bishop, 'r' => PieceKind::Rook, 'q' => PieceKind::Queen, 'k' => PieceKind::King, _ => return None };
        Some(Self { color, kind })
    }
    pub fn fen_char(self) -> char {
        let c = self.kind.fen_letter();
        if self.color == Color::White { c.to_ascii_uppercase() } else { c }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenError(pub String);
impl fmt::Display for FenError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "invalid FEN: {}", self.0) } }
impl std::error::Error for FenError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionError(pub String);
impl fmt::Display for PositionError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "invalid position: {}", self.0) } }
impl std::error::Error for PositionError {}

/// State needed to restore a move. It intentionally stores no heap-owned board
/// copy: search mutates one position and reverses moves, avoiding allocations
/// at every node while keeping restoration explicit and auditable.
#[derive(Clone, Copy, Debug)]
pub struct Undo {
    captured: Option<Piece>,
    captured_square: Option<Square>,
    castling: u8,
    ep: Option<Square>,
    halfmove: u16,
    fullmove: u16,
    key: u64,
    history_len: usize,
}

#[derive(Clone)]
pub struct Position {
    pieces: [u64; 12],
    occupancy: [u64; 2],
    all: u64,
    mailbox: [Option<Piece>; 64],
    side_to_move: Color,
    castling: u8,
    ep: Option<Square>,
    halfmove: u16,
    fullmove: u16,
    key: u64,
    /// Includes the current position. The game/`position ... moves` caller
    /// retains this vector so repetition is judged over the entire known game.
    history: Vec<u64>,
}

impl Default for Position {
    fn default() -> Self { Self::from_fen(crate::STARTPOS_FEN).expect("built-in start FEN is valid") }
}

impl Position {
    pub fn empty() -> Self {
        Self { pieces: [0; 12], occupancy: [0; 2], all: 0, mailbox: [None; 64], side_to_move: Color::White, castling: 0, ep: None, halfmove: 0, fullmove: 1, key: 0, history: Vec::new() }
    }

    pub fn from_fen(fen: &str) -> Result<Self, FenError> {
        let fields: Vec<&str> = fen.split_ascii_whitespace().collect();
        if fields.len() != 6 { return Err(FenError("expected exactly six fields".into())); }
        let mut position = Self::empty();
        let ranks: Vec<&str> = fields[0].split('/').collect();
        if ranks.len() != 8 { return Err(FenError("piece placement must contain eight ranks".into())); }
        for (rank_from_top, rank_text) in ranks.iter().enumerate() {
            let rank = 7_u8.saturating_sub(rank_from_top as u8);
            let mut file = 0_u8;
            for ch in rank_text.chars() {
                if ch.is_ascii_digit() {
                    let n = ch.to_digit(10).unwrap_or(0) as u8;
                    if n == 0 || n > 8 { return Err(FenError(format!("invalid empty-square count `{ch}`"))); }
                    file = file.saturating_add(n);
                } else if let Some(piece) = Piece::from_fen(ch) {
                    if file >= 8 { return Err(FenError(format!("rank {} has more than eight files", 8 - rank_from_top))); }
                    position.add_piece(piece, rank * 8 + file);
                    file += 1;
                } else { return Err(FenError(format!("invalid piece character `{ch}`"))); }
                if file > 8 { return Err(FenError(format!("rank {} has more than eight files", 8 - rank_from_top))); }
            }
            if file != 8 { return Err(FenError(format!("rank {} does not contain eight files", 8 - rank_from_top))); }
        }
        position.side_to_move = match fields[1] { "w" => Color::White, "b" => Color::Black, _ => return Err(FenError("active color must be `w` or `b`".into())) };
        position.castling = Self::parse_castling(fields[2])?;
        position.ep = if fields[3] == "-" { None } else { Some(parse_square(fields[3]).map_err(FenError)?) };
        position.halfmove = fields[4].parse::<u16>().map_err(|_| FenError("halfmove clock must be a non-negative u16".into()))?;
        position.fullmove = fields[5].parse::<u16>().map_err(|_| FenError("fullmove number must be a positive u16".into()))?;
        if position.fullmove == 0 { return Err(FenError("fullmove number must be at least 1".into())); }
        position.validate().map_err(|e| FenError(e.0))?;
        position.key = position.recompute_key();
        position.history.push(position.key);
        Ok(position)
    }

    fn parse_castling(text: &str) -> Result<u8, FenError> {
        if text == "-" { return Ok(0); }
        let mut rights = 0;
        for ch in text.chars() {
            let bit = match ch { 'K' => CASTLE_WK, 'Q' => CASTLE_WQ, 'k' => CASTLE_BK, 'q' => CASTLE_BQ, _ => return Err(FenError("castling rights must use KQkq or -".into())) };
            if rights & bit != 0 { return Err(FenError("duplicate castling right".into())); }
            rights |= bit;
        }
        Ok(rights)
    }

    /// Reject impossible position metadata instead of silently repairing it.
    /// It intentionally does not try to prove that every legal-looking study
    /// position is reachable from the initial game.
    pub fn validate(&self) -> Result<(), PositionError> {
        for color in [Color::White, Color::Black] {
            if self.pieces[color.index() * 6 + PieceKind::King as usize].count_ones() != 1 { return Err(PositionError(format!("{color:?} must have exactly one king"))); }
            if self.pieces[color.index() * 6 + PieceKind::Pawn as usize].count_ones() > 8 { return Err(PositionError(format!("{color:?} has more than eight pawns"))); }
            let pawns = self.pieces[color.index() * 6 + PieceKind::Pawn as usize];
            if pawns & 0xff00_0000_0000_00ff != 0 { return Err(PositionError("pawns may not be on first or eighth rank".into())); }
        }
        let wk = self.king_square(Color::White).ok_or_else(|| PositionError("missing white king".into()))?;
        let bk = self.king_square(Color::Black).ok_or_else(|| PositionError("missing black king".into()))?;
        if (file_of(wk) as i8 - file_of(bk) as i8).abs() <= 1 && (rank_of(wk) as i8 - rank_of(bk) as i8).abs() <= 1 { return Err(PositionError("kings may not be adjacent".into())); }
        let checks = [(CASTLE_WK, 4, 7), (CASTLE_WQ, 4, 0), (CASTLE_BK, 60, 63), (CASTLE_BQ, 60, 56)];
        for (right, king, rook) in checks {
            if self.castling & right != 0 {
                let c = if right & (CASTLE_WK | CASTLE_WQ) != 0 { Color::White } else { Color::Black };
                if self.piece_at(king) != Some(Piece { color: c, kind: PieceKind::King }) || self.piece_at(rook) != Some(Piece { color: c, kind: PieceKind::Rook }) { return Err(PositionError("castling right is inconsistent with king/rook home squares".into())); }
            }
        }
        if let Some(ep) = self.ep {
            let valid_rank = match self.side_to_move { Color::White => rank_of(ep) == 5, Color::Black => rank_of(ep) == 2 };
            if !valid_rank { return Err(PositionError("en-passant target rank is inconsistent with active color".into())); }
            if self.piece_at(ep).is_some() { return Err(PositionError("en-passant target square must be empty".into())); }
            let pawn_square = match self.side_to_move { Color::White => ep.checked_sub(8), Color::Black => ep.checked_add(8) };
            let moved_color = self.side_to_move.opposite();
            match pawn_square.and_then(|s| self.piece_at(s)) {
                Some(Piece { color, kind: PieceKind::Pawn }) if color == moved_color => {},
                _ => return Err(PositionError("en-passant target lacks the pawn that made the double push".into())),
            }
        }
        // The side that just moved cannot have left its own king in check.
        if crate::movegen::is_square_attacked(self, self.king_square(self.side_to_move.opposite()).unwrap_or(0), self.side_to_move) {
            return Err(PositionError("side that just moved is in check".into()));
        }
        Ok(())
    }

    pub fn to_fen(&self) -> String {
        let mut placement = String::new();
        for rank in (0..8).rev() {
            let mut empty = 0;
            for file in 0..8 {
                match self.mailbox[(rank * 8 + file) as usize] {
                    Some(piece) => { if empty != 0 { placement.push(char::from(b'0' + empty)); empty = 0; } placement.push(piece.fen_char()); }
                    None => empty += 1,
                }
            }
            if empty != 0 { placement.push(char::from(b'0' + empty)); }
            if rank != 0 { placement.push('/'); }
        }
        let mut c = String::new();
        if self.castling & CASTLE_WK != 0 { c.push('K'); }
        if self.castling & CASTLE_WQ != 0 { c.push('Q'); }
        if self.castling & CASTLE_BK != 0 { c.push('k'); }
        if self.castling & CASTLE_BQ != 0 { c.push('q'); }
        if c.is_empty() { c.push('-'); }
        format!("{} {} {} {} {} {}", placement, if self.side_to_move == Color::White { "w" } else { "b" }, c, self.ep.map(square_name).unwrap_or_else(|| "-".into()), self.halfmove, self.fullmove)
    }

    #[inline] pub fn pieces(&self, color: Color, kind: PieceKind) -> u64 { self.pieces[color.index() * 6 + kind as usize] }
    #[inline] pub fn occupancy(&self, color: Color) -> u64 { self.occupancy[color.index()] }
    #[inline] pub fn all_occupancy(&self) -> u64 { self.all }
    #[inline] pub fn side_to_move(&self) -> Color { self.side_to_move }
    #[inline] pub fn castling_rights(&self) -> u8 { self.castling }
    #[inline] pub fn en_passant(&self) -> Option<Square> { self.ep }
    #[inline] pub fn halfmove_clock(&self) -> u16 { self.halfmove }
    #[inline] pub fn fullmove_number(&self) -> u16 { self.fullmove }
    #[inline] pub fn key(&self) -> u64 { self.key }
    #[inline] pub fn piece_at(&self, sq: Square) -> Option<Piece> { self.mailbox.get(sq as usize).copied().flatten() }
    #[inline] pub fn king_square(&self, color: Color) -> Option<Square> { let bb = self.pieces(color, PieceKind::King); if bb == 0 { None } else { Some(bb.trailing_zeros() as Square) } }
    #[inline] pub fn history(&self) -> &[u64] { &self.history }

    pub fn in_check(&self, color: Color) -> bool { self.king_square(color).map(|sq| crate::movegen::is_square_attacked(self, sq, color.opposite())).unwrap_or(true) }
    pub fn is_checkmate(&self) -> bool { self.in_check(self.side_to_move) && self.legal_moves().is_empty() }
    pub fn is_stalemate(&self) -> bool { !self.in_check(self.side_to_move) && self.legal_moves().is_empty() }

    pub fn is_fifty_move_draw(&self) -> bool { self.halfmove >= 100 }
    pub fn is_threefold_repetition(&self) -> bool { self.history.iter().filter(|&&k| k == self.key).count() >= 3 }
    /// FIDE dead-position cases that can be decided solely from material:
    /// K/K, K+B/K, K+N/K, and K+B/K+B when every bishop is on the same color.
    /// More exotic dead positions are intentionally not guessed; search can
    /// still find their lack of progress, rather than incorrectly declaring a draw.
    pub fn is_insufficient_material(&self) -> bool {
        if self.pieces(Color::White, PieceKind::Pawn) | self.pieces(Color::Black, PieceKind::Pawn) | self.pieces(Color::White, PieceKind::Rook) | self.pieces(Color::Black, PieceKind::Rook) | self.pieces(Color::White, PieceKind::Queen) | self.pieces(Color::Black, PieceKind::Queen) != 0 { return false; }
        let wn = self.pieces(Color::White, PieceKind::Knight).count_ones();
        let bn = self.pieces(Color::Black, PieceKind::Knight).count_ones();
        let wb = self.pieces(Color::White, PieceKind::Bishop);
        let bb = self.pieces(Color::Black, PieceKind::Bishop);
        let minor = wn + bn + wb.count_ones() + bb.count_ones();
        if minor == 0 { return true; }
        if minor == 1 { return true; }
        if wn == 0 && bn == 0 && wb.count_ones() == 1 && bb.count_ones() == 1 {
            let ws = wb.trailing_zeros() as Square;
            let bs = bb.trailing_zeros() as Square;
            return square_color(ws) == square_color(bs);
        }
        false
    }
    pub fn is_draw(&self) -> bool { self.is_fifty_move_draw() || self.is_threefold_repetition() || self.is_insufficient_material() }

    /// A public immutable convenience wrapper. Performance-sensitive search and
    /// perft use `legal_moves_mut`, which makes/unmakes a single board.
    pub fn legal_moves(&self) -> crate::movegen::MoveList { let mut copy = self.clone(); copy.legal_moves_mut() }
    pub fn legal_moves_mut(&mut self) -> crate::movegen::MoveList { crate::movegen::generate_legal(self) }

    /// Applies a move only after checking it against the strictly legal move
    /// list. This is the API used for FEN/UCI game input; recursive search uses
    /// the crate-private unchecked form after obtaining that same list.
    pub fn try_make_move(&mut self, mv: Move) -> Result<Undo, PositionError> {
        if !self.legal_moves().iter().any(|&m| m == mv) { return Err(PositionError("move is not legal in this position".into())); }
        Ok(self.make_move_unchecked(mv))
    }

    pub(crate) fn make_move_unchecked(&mut self, mv: Move) -> Undo {
        let from = mv.from(); let to = mv.to();
        let moving = self.mailbox[from as usize].expect("generated move has a moving piece");
        let undo = Undo { captured: None, captured_square: None, castling: self.castling, ep: self.ep, halfmove: self.halfmove, fullmove: self.fullmove, key: self.key, history_len: self.history.len() };
        self.xor_state_keys();
        self.remove_piece(moving, from);
        let mut captured = self.mailbox[to as usize];
        let mut captured_square = captured.map(|_| to);
        if mv.kind() == MoveKind::EnPassant {
            let cap_sq = if moving.color == Color::White { to - 8 } else { to + 8 };
            captured = self.mailbox[cap_sq as usize];
            captured_square = Some(cap_sq);
        }
        if let (Some(piece), Some(sq)) = (captured, captured_square) { self.remove_piece(piece, sq); }
        let placed = match mv.promotion() { Some(kind) => Piece { color: moving.color, kind }, None => moving };
        self.add_piece(placed, to);
        match mv.kind() {
            MoveKind::CastleKing => {
                let (rf, rt) = if moving.color == Color::White { (7, 5) } else { (63, 61) };
                let rook = self.mailbox[rf].expect("legal castle has rook"); self.remove_piece(rook, rf as Square); self.add_piece(rook, rt as Square);
            }
            MoveKind::CastleQueen => {
                let (rf, rt) = if moving.color == Color::White { (0, 3) } else { (56, 59) };
                let rook = self.mailbox[rf].expect("legal castle has rook"); self.remove_piece(rook, rf as Square); self.add_piece(rook, rt as Square);
            }
            _ => {}
        }
        self.update_castling_rights(from, to, moving, captured);
        self.ep = if mv.kind() == MoveKind::DoublePawn { Some(if moving.color == Color::White { from + 8 } else { from - 8 }) } else { None };
        self.halfmove = if moving.kind == PieceKind::Pawn || captured.is_some() { 0 } else { self.halfmove.saturating_add(1) };
        if moving.color == Color::Black { self.fullmove = self.fullmove.saturating_add(1); }
        self.side_to_move = self.side_to_move.opposite();
        self.xor_state_keys();
        self.history.push(self.key);
        Undo { captured, captured_square, ..undo }
    }

    pub(crate) fn unmake_move(&mut self, mv: Move, undo: Undo) {
        self.history.truncate(undo.history_len);
        self.side_to_move = self.side_to_move.opposite();
        let from = mv.from(); let to = mv.to();
        let moved_color = self.side_to_move;
        let placed = self.mailbox[to as usize].expect("unmake destination has piece");
        self.remove_piece(placed, to);
        let original_kind = if mv.promotion().is_some() { PieceKind::Pawn } else { placed.kind };
        self.add_piece(Piece { color: moved_color, kind: original_kind }, from);
        match mv.kind() {
            MoveKind::CastleKing => { let (rf, rt) = if moved_color == Color::White { (7, 5) } else { (63, 61) }; let rook = self.mailbox[rt].expect("castle rook is present on unmake"); self.remove_piece(rook, rt as Square); self.add_piece(rook, rf as Square); }
            MoveKind::CastleQueen => { let (rf, rt) = if moved_color == Color::White { (0, 3) } else { (56, 59) }; let rook = self.mailbox[rt].expect("castle rook is present on unmake"); self.remove_piece(rook, rt as Square); self.add_piece(rook, rf as Square); }
            _ => {}
        }
        if let (Some(piece), Some(sq)) = (undo.captured, undo.captured_square) { self.add_piece(piece, sq); }
        self.castling = undo.castling; self.ep = undo.ep; self.halfmove = undo.halfmove; self.fullmove = undo.fullmove; self.key = undo.key;
    }

    /// Null moves are search-only and never exposed through game APIs. The
    /// `history` update keeps repetition detection well-defined during a null
    /// subtree; callers must restore with `unmake_null_move`.
    pub(crate) fn make_null_move(&mut self) -> Undo {
        let undo = Undo { captured: None, captured_square: None, castling: self.castling, ep: self.ep, halfmove: self.halfmove, fullmove: self.fullmove, key: self.key, history_len: self.history.len() };
        self.xor_state_keys();
        self.ep = None; self.halfmove = self.halfmove.saturating_add(1);
        if self.side_to_move == Color::Black { self.fullmove = self.fullmove.saturating_add(1); }
        self.side_to_move = self.side_to_move.opposite(); self.xor_state_keys(); self.history.push(self.key);
        undo
    }
    pub(crate) fn unmake_null_move(&mut self, undo: Undo) { self.history.truncate(undo.history_len); self.side_to_move = self.side_to_move.opposite(); self.castling = undo.castling; self.ep = undo.ep; self.halfmove = undo.halfmove; self.fullmove = undo.fullmove; self.key = undo.key; }

    pub(crate) fn has_non_pawn_material(&self, color: Color) -> bool {
        self.pieces(color, PieceKind::Knight) | self.pieces(color, PieceKind::Bishop) | self.pieces(color, PieceKind::Rook) | self.pieces(color, PieceKind::Queen) != 0
    }

    fn update_castling_rights(&mut self, from: Square, to: Square, moving: Piece, captured: Option<Piece>) {
        if moving.kind == PieceKind::King { self.castling &= if moving.color == Color::White { !(CASTLE_WK | CASTLE_WQ) } else { !(CASTLE_BK | CASTLE_BQ) }; }
        if moving.kind == PieceKind::Rook { self.clear_rook_right(from); }
        if let Some(Piece { kind: PieceKind::Rook, .. }) = captured { self.clear_rook_right(to); }
    }
    fn clear_rook_right(&mut self, sq: Square) { match sq { 0 => self.castling &= !CASTLE_WQ, 7 => self.castling &= !CASTLE_WK, 56 => self.castling &= !CASTLE_BQ, 63 => self.castling &= !CASTLE_BK, _ => {} } }
    fn xor_state_keys(&mut self) { let z = keys(); self.key ^= z.castle[self.castling as usize]; if let Some(ep) = self.ep { self.key ^= z.ep_file[file_of(ep) as usize]; } if self.side_to_move == Color::Black { self.key ^= z.side; } }
    fn recompute_key(&self) -> u64 {
        let z = keys(); let mut k = z.castle[self.castling as usize];
        for sq in 0..64 { if let Some(p) = self.mailbox[sq] { k ^= z.piece[p.index()][sq]; } }
        if let Some(ep) = self.ep { k ^= z.ep_file[file_of(ep) as usize]; }
        if self.side_to_move == Color::Black { k ^= z.side; } k
    }
    fn add_piece(&mut self, piece: Piece, sq: Square) { debug_assert!(self.mailbox[sq as usize].is_none()); let bit = 1_u64 << sq; self.mailbox[sq as usize] = Some(piece); self.pieces[piece.index()] |= bit; self.occupancy[piece.color.index()] |= bit; self.all |= bit; self.key ^= keys().piece[piece.index()][sq as usize]; }
    fn remove_piece(&mut self, piece: Piece, sq: Square) { debug_assert_eq!(self.mailbox[sq as usize], Some(piece)); let bit = 1_u64 << sq; self.mailbox[sq as usize] = None; self.pieces[piece.index()] &= !bit; self.occupancy[piece.color.index()] &= !bit; self.all &= !bit; self.key ^= keys().piece[piece.index()][sq as usize]; }
}

#[inline] pub const fn file_of(sq: Square) -> u8 { sq & 7 }
#[inline] pub const fn rank_of(sq: Square) -> u8 { sq >> 3 }
#[inline] pub const fn square_color(sq: Square) -> bool { ((file_of(sq) + rank_of(sq)) & 1) != 0 }
pub fn square_name(sq: Square) -> String { let mut out = String::with_capacity(2); out.push((b'a' + file_of(sq)) as char); out.push((b'1' + rank_of(sq)) as char); out }
pub fn parse_square(text: &str) -> Result<Square, String> { let b = text.as_bytes(); if b.len() != 2 || !(b'a'..=b'h').contains(&b[0]) || !(b'1'..=b'8').contains(&b[1]) { return Err(format!("invalid square `{text}`")); } Ok((b[0] - b'a') + 8 * (b[1] - b'1')) }
