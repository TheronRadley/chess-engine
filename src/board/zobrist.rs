//! Deterministic Zobrist keys. A fixed SplitMix64 seed is intentional: it
//! makes analysis reproducible for a given hash-table state.

use std::sync::OnceLock;

pub struct Keys {
    pub piece: [[u64; 64]; 12],
    pub side: u64,
    pub castle: [u64; 16],
    pub ep_file: [u64; 8],
}

fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn build() -> Keys {
    let mut state = 0x4d59_5df4_d0f3_3173_u64;
    let mut piece = [[0_u64; 64]; 12];
    for row in &mut piece {
        for key in row {
            *key = next(&mut state);
        }
    }
    let side = next(&mut state);
    let mut castle = [0_u64; 16];
    for key in &mut castle {
        *key = next(&mut state);
    }
    let mut ep_file = [0_u64; 8];
    for key in &mut ep_file {
        *key = next(&mut state);
    }
    Keys { piece, side, castle, ep_file }
}

pub fn keys() -> &'static Keys {
    static KEYS: OnceLock<Keys> = OnceLock::new();
    KEYS.get_or_init(build)
}
