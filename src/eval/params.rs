//! All hand-tuned evaluation constants are centralized here. They are modest
//! classical values intended as stable defaults rather than claims of optimal
//! tuning; changing them does not alter move legality or search semantics.

pub const MG_VALUE: [i32; 6] = [100, 320, 335, 500, 950, 0];
pub const EG_VALUE: [i32; 6] = [120, 305, 325, 525, 925, 0];
pub const PHASE_VALUE: [i32; 6] = [0, 1, 1, 2, 4, 0];
pub const MAX_PHASE: i32 = 24;

pub const DOUBLED_PAWN: i32 = 12;
pub const ISOLATED_PAWN: i32 = 14;
pub const CONNECTED_PAWN: i32 = 7;
pub const BACKWARD_PAWN: i32 = 8;
pub const PASSED_MG: [i32; 8] = [0, 0, 6, 13, 25, 45, 72, 0];
pub const PASSED_EG: [i32; 8] = [0, 0, 12, 28, 55, 90, 145, 0];
pub const BISHOP_PAIR_MG: i32 = 28;
pub const BISHOP_PAIR_EG: i32 = 40;
pub const ROOK_OPEN_FILE: i32 = 22;
pub const ROOK_SEMIOPEN_FILE: i32 = 11;
pub const KING_SHIELD: i32 = 12;
pub const KING_OPEN_FILE: i32 = 18;
pub const KING_ATTACK_UNIT: i32 = 7;
pub const MOBILITY_MG: [i32; 6] = [0, 4, 4, 2, 1, 0];
pub const MOBILITY_EG: [i32; 6] = [0, 3, 3, 3, 2, 0];
pub const SPACE_MG: i32 = 5;
pub const KING_CENTER_EG: i32 = 7;

/// PST values are generated from symmetric file/rank coefficients rather than
/// stored as six opaque 64-entry literals. This remains a genuine distinct
/// MG/EG piece-square table while making orientation and tuning auditable.
pub const MG_FILE: [i32; 8] = [-12, -5, 2, 8, 8, 2, -5, -12];
pub const EG_FILE: [i32; 8] = [-7, -2, 3, 6, 6, 3, -2, -7];
pub const MG_RANK: [[i32; 8]; 6] = [
    [0, 4, 8, 14, 21, 30, 0, 0], // pawn
    [-12, -4, 2, 7, 8, 4, -3, -12],
    [-7, -1, 4, 7, 8, 5, 0, -8],
    [0, 2, 4, 7, 9, 7, 4, 1],
    [-3, 0, 3, 5, 6, 4, 1, -4],
    [16, 8, 1, -10, -16, -18, -10, 2],
];
pub const EG_RANK: [[i32; 8]; 6] = [
    [0, 5, 12, 22, 38, 60, 0, 0],
    [-5, -1, 3, 5, 6, 5, 1, -5],
    [-4, 0, 3, 5, 6, 4, 1, -4],
    [1, 3, 5, 7, 8, 7, 5, 3],
    [0, 2, 4, 6, 7, 6, 4, 2],
    [-18, -8, 2, 12, 18, 20, 12, 2],
];
