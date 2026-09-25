use crate::board::{square_name, PieceKind, Square};

/// Four bits fit all move classes. Capture-ness is normally derived from the
/// board, so promoted captures need no redundant flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MoveKind {
    Quiet = 0,
    Capture = 1,
    DoublePawn = 2,
    EnPassant = 3,
    CastleKing = 4,
    CastleQueen = 5,
    PromoteKnight = 6,
    PromoteBishop = 7,
    PromoteRook = 8,
    PromoteQueen = 9,
}

impl MoveKind {
    pub fn from_bits(bits: u32) -> Option<Self> { match bits { 0 => Some(Self::Quiet), 1 => Some(Self::Capture), 2 => Some(Self::DoublePawn), 3 => Some(Self::EnPassant), 4 => Some(Self::CastleKing), 5 => Some(Self::CastleQueen), 6 => Some(Self::PromoteKnight), 7 => Some(Self::PromoteBishop), 8 => Some(Self::PromoteRook), 9 => Some(Self::PromoteQueen), _ => None } }
}

/// A 16-bit value: from [0..5], to [6..11], and kind [12..15].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Move(u16);

impl Move {
    pub const NULL: Self = Self(0xffff);
    #[inline] pub const fn new(from: Square, to: Square, kind: MoveKind) -> Self { Self((from as u16) | ((to as u16) << 6) | ((kind as u16) << 12)) }
    #[inline] pub const fn from(self) -> Square { (self.0 & 63) as Square }
    #[inline] pub const fn to(self) -> Square { ((self.0 >> 6) & 63) as Square }
    #[inline] pub fn kind(self) -> MoveKind { MoveKind::from_bits((self.0 >> 12) as u32).unwrap_or(MoveKind::Quiet) }
    #[inline] pub const fn raw(self) -> u16 { self.0 }
    #[inline] pub fn is_null(self) -> bool { self == Self::NULL }
    #[inline] pub fn is_castle(self) -> bool { matches!(self.kind(), MoveKind::CastleKing | MoveKind::CastleQueen) }
    #[inline] pub fn is_capture_kind(self) -> bool { matches!(self.kind(), MoveKind::Capture | MoveKind::EnPassant) }
    #[inline] pub fn promotion(self) -> Option<PieceKind> { match self.kind() { MoveKind::PromoteKnight => Some(PieceKind::Knight), MoveKind::PromoteBishop => Some(PieceKind::Bishop), MoveKind::PromoteRook => Some(PieceKind::Rook), MoveKind::PromoteQueen => Some(PieceKind::Queen), _ => None } }
    #[inline] pub fn is_promotion(self) -> bool { self.promotion().is_some() }
    pub fn to_uci(self) -> String {
        if self.is_null() { return "0000".into(); }
        let mut out = format!("{}{}", square_name(self.from()), square_name(self.to()));
        if let Some(p) = self.promotion() { out.push(p.fen_letter()); }
        out
    }
}
