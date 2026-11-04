//! Upstream's compact layout of solid quads (one 24-byte record per whole cube face) needs
//! its 24-byte float-position `TerrainVertex`; Perforizon's vertex is 20 bytes with
//! quantised positions, so no record is ever made and every mesh keeps its vertices.
use super::TerrainVertex;
use std::sync::atomic::{AtomicBool, Ordering};

pub const UNKNOWN_TINT: u32 = u32::MAX;
pub const WHITE: u32 = 0xFF_FFFF;
pub const FULL: u32 = 1 << 31;
pub const PACKED_BASE: u32 = 1 << 28;

static PACK: AtomicBool = AtomicBool::new(false);

pub fn pack_quads(on: bool) {
    PACK.store(on, Ordering::Relaxed);
}

pub fn packing() -> bool {
    PACK.load(Ordering::Relaxed)
}

pub fn layout(_solid: &[TerrainVertex], _tints: &[u32]) -> Vec<[u32; 6]> {
    Vec::new()
}

pub fn packed_quads(layout: &[[u32; 6]], quads: usize) -> usize {
    layout
        .iter()
        .take(quads)
        .filter(|w| w[5] & FULL == 0)
        .count()
}
