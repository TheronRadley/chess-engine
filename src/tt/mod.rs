//! Fixed-size, depth-preferred transposition table.

use crate::moves::Move;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound { Exact, Lower, Upper }

#[derive(Clone, Copy, Debug)]
pub struct TTEntry { pub key: u64, pub depth: i16, pub score: i32, pub bound: Bound, pub best: Move, pub generation: u8, pub valid: bool }
impl Default for TTEntry { fn default() -> Self { Self { key: 0, depth: -1, score: 0, bound: Bound::Upper, best: Move::NULL, generation: 0, valid: false } } }

pub struct TranspositionTable { entries: Vec<TTEntry>, mask: usize, generation: u8 }
impl TranspositionTable {
    /// Defaults to 16 MiB when constructed by `Engine`. A power-of-two table
    /// permits a fast, deterministic index; sizes are rounded down, never up.
    pub fn new(megabytes: usize) -> Self {
        let bytes = megabytes.max(1).saturating_mul(1024 * 1024); let requested = (bytes / std::mem::size_of::<TTEntry>()).max(1); let count = requested.next_power_of_two() >> if requested.is_power_of_two() { 0 } else { 1 };
        Self { entries: vec![TTEntry::default(); count], mask: count - 1, generation: 0 }
    }
    pub fn resize(&mut self, megabytes: usize) { *self = Self::new(megabytes); }
    pub fn clear(&mut self) { self.entries.fill(TTEntry::default()); self.generation = 0; }
    pub fn new_search(&mut self) { self.generation = self.generation.wrapping_add(1); }
    #[inline] fn index(&self, key: u64) -> usize { (key as usize) & self.mask }
    pub fn probe(&self, key: u64) -> Option<TTEntry> { let entry = self.entries[self.index(key)]; if entry.valid && entry.key == key { Some(entry) } else { None } }
    pub fn store(&mut self, key: u64, depth: i16, score: i32, bound: Bound, best: Move) {
        let idx = self.index(key); let old = self.entries[idx];
        // Same key always refreshes. Otherwise retain a sufficiently deeper,
        // current-generation entry; generation makes stale shallow positions replaceable.
        if !old.valid || old.key == key || old.generation != self.generation || depth >= old.depth {
            self.entries[idx] = TTEntry { key, depth, score, bound, best, generation: self.generation, valid: true };
        }
    }
    /// Permille occupancy sampling matches UCI's `hashfull` convention.
    pub fn hashfull(&self) -> u16 { let sample = self.entries.len().min(1000); if sample == 0 { return 0; } let used = self.entries.iter().take(sample).filter(|e| e.valid).count(); ((used * 1000) / sample) as u16 }
    pub fn megabytes(&self) -> usize { self.entries.len() * std::mem::size_of::<TTEntry>() / (1024 * 1024) }
}
