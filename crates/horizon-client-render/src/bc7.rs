//! BC7 (mode 6) compression of the block sprites' mip chains, for the Texture Compression
//! option: one 16-byte block per 4x4 texels, two RGBA endpoints with sixteen steps between
//! them. Every layer and every level is encoded on its own, so no block ever holds texels of
//! two sprites; levels smaller than a block repeat their edge texels.

use std::sync::atomic::{AtomicBool, Ordering};

static REQUESTED: AtomicBool = AtomicBool::new(false);

/// Sets the Texture Compression option; true if that changes it, when the textures must load
/// again.
pub fn set_requested(on: bool) -> bool {
    REQUESTED.swap(on, Ordering::Relaxed) != on
}

/// The Texture Compression option.
pub fn requested() -> bool {
    REQUESTED.load(Ordering::Relaxed)
}

pub const BLOCK_BYTES: usize = 16;
const WEIGHTS: [i32; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];
const CHANNEL: [i32; 4] = [1, 1, 1, 4];

/// Bytes of a `width` by `height` level once compressed.
pub fn level_bytes(width: u32, height: u32) -> usize {
    width.div_ceil(4) as usize * height.div_ceil(4) as usize * BLOCK_BYTES
}

/// Bytes of a square texture's whole chain, `size` down to one texel.
pub fn chain_bytes(size: u32) -> usize {
    (0..=size.trailing_zeros())
        .map(|l| level_bytes(size >> l, size >> l))
        .sum()
}

#[derive(Clone, Copy)]
struct Endpoint {
    q: [i32; 4],
    p: i32,
}

impl Endpoint {
    fn value(&self, c: usize) -> i32 {
        self.q[c] * 2 + self.p
    }
}

fn quantize(v: [f32; 4]) -> Endpoint {
    let mut best = (f32::MAX, Endpoint { q: [0; 4], p: 0 });
    for p in 0..2 {
        let mut q = [0; 4];
        let mut err = 0.0;
        for c in 0..4 {
            q[c] = (((v[c] - p as f32) / 2.0).round() as i32).clamp(0, 127);
            let d = (q[c] * 2 + p) as f32 - v[c];
            err += CHANNEL[c] as f32 * d * d;
        }
        if err < best.0 {
            best = (err, Endpoint { q, p });
        }
    }
    best.1
}

fn palette(a: &Endpoint, b: &Endpoint) -> [[i32; 4]; 16] {
    std::array::from_fn(|i| {
        std::array::from_fn(|c| {
            ((64 - WEIGHTS[i]) * a.value(c) + WEIGHTS[i] * b.value(c) + 32) >> 6
        })
    })
}

fn assign(px: &[[u8; 4]; 16], palette: &[[i32; 4]; 16]) -> ([usize; 16], i32) {
    let mut indices = [0; 16];
    let mut total = 0;
    for (i, p) in px.iter().enumerate() {
        let (index, err) = palette
            .iter()
            .enumerate()
            .map(|(k, e)| {
                let err: i32 = (0..4)
                    .map(|c| {
                        let d = e[c] - i32::from(p[c]);
                        CHANNEL[c] * d * d
                    })
                    .sum();
                (k, err)
            })
            .min_by_key(|&(_, err)| err)
            .unwrap_or((0, 0));
        indices[i] = index;
        total += err;
    }
    (indices, total)
}

fn fit(px: &[[u8; 4]; 16], indices: &[usize; 16]) -> Option<([f32; 4], [f32; 4])> {
    let (mut saa, mut sab, mut sbb) = (0.0f32, 0.0f32, 0.0f32);
    let (mut xa, mut xb) = ([0.0f32; 4], [0.0f32; 4]);
    for (p, &i) in px.iter().zip(indices) {
        let t = WEIGHTS[i] as f32 / 64.0;
        let s = 1.0 - t;
        saa += s * s;
        sab += s * t;
        sbb += t * t;
        for c in 0..4 {
            xa[c] += s * f32::from(p[c]);
            xb[c] += t * f32::from(p[c]);
        }
    }
    let det = saa * sbb - sab * sab;
    if det < 1e-3 {
        return None;
    }
    let (mut a, mut b) = ([0.0; 4], [0.0; 4]);
    for c in 0..4 {
        a[c] = ((xa[c] * sbb - xb[c] * sab) / det).clamp(0.0, 255.0);
        b[c] = ((xb[c] * saa - xa[c] * sab) / det).clamp(0.0, 255.0);
    }
    Some((a, b))
}

fn extremes(px: &[[u8; 4]; 16]) -> ([f32; 4], [f32; 4]) {
    let mut mean = [0.0f32; 4];
    for p in px {
        for c in 0..4 {
            mean[c] += f32::from(p[c]) / 16.0;
        }
    }
    let mut axis = [0.0f32; 4];
    let mut far = 0.0f32;
    for p in px {
        let d: [f32; 4] = std::array::from_fn(|c| f32::from(p[c]) - mean[c]);
        let norm: f32 = d.iter().map(|v| v * v).sum();
        if norm > far {
            (far, axis) = (norm, d);
        }
    }
    for _ in 0..6 {
        let mut next = [0.0f32; 4];
        for p in px {
            let d: [f32; 4] = std::array::from_fn(|c| f32::from(p[c]) - mean[c]);
            let dot: f32 = (0..4).map(|c| d[c] * axis[c]).sum();
            for c in 0..4 {
                next[c] += d[c] * dot;
            }
        }
        let len = next.iter().map(|v| v * v).sum::<f32>().sqrt();
        if len < 1e-6 {
            break;
        }
        axis = next.map(|v| v / len);
    }
    let along =
        |p: &[u8; 4]| -> f32 { (0..4).map(|c| (f32::from(p[c]) - mean[c]) * axis[c]).sum() };
    let (mut lo, mut hi) = (px[0], px[0]);
    for p in px {
        if along(p) < along(&lo) {
            lo = *p;
        }
        if along(p) > along(&hi) {
            hi = *p;
        }
    }
    (lo.map(f32::from), hi.map(f32::from))
}

/// One block of 16 texels (row major) as BC7 mode 6.
pub fn encode_block(px: &[[u8; 4]; 16]) -> [u8; BLOCK_BYTES] {
    let (lo, hi) = extremes(px);
    let mut ends = (quantize(lo), quantize(hi));
    let (mut indices, mut err) = assign(px, &palette(&ends.0, &ends.1));
    for _ in 0..2 {
        if err == 0 {
            break;
        }
        let Some((a, b)) = fit(px, &indices) else {
            break;
        };
        let trial = (quantize(a), quantize(b));
        let (trial_indices, trial_err) = assign(px, &palette(&trial.0, &trial.1));
        if trial_err >= err {
            break;
        }
        (ends, indices, err) = (trial, trial_indices, trial_err);
    }
    if indices[0] >= 8 {
        ends = (ends.1, ends.0);
        indices = indices.map(|i| 15 - i);
    }
    let mut bits: u128 = 0;
    let mut at = 0;
    let mut put = |value: u128, count: u32| {
        bits |= (value & ((1u128 << count) - 1)) << at;
        at += count;
    };
    put(1 << 6, 7);
    for c in 0..4 {
        put(ends.0.q[c] as u128, 7);
        put(ends.1.q[c] as u128, 7);
    }
    put(ends.0.p as u128, 1);
    put(ends.1.p as u128, 1);
    for (i, &index) in indices.iter().enumerate() {
        put(index as u128, if i == 0 { 3 } else { 4 });
    }
    bits.to_le_bytes()
}

/// A `width` by `height` RGBA level as BC7 blocks, rows of blocks top to bottom; texels past
/// the edge repeat the edge.
pub fn encode_level(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(level_bytes(width, height));
    for by in 0..height.div_ceil(4) {
        for bx in 0..width.div_ceil(4) {
            let px: [[u8; 4]; 16] = std::array::from_fn(|i| {
                let x = (bx * 4 + i as u32 % 4).min(width - 1);
                let y = (by * 4 + i as u32 / 4).min(height - 1);
                let at = ((y * width + x) * 4) as usize;
                [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]]
            });
            out.extend_from_slice(&encode_block(&px));
        }
    }
    out
}

/// One square layer's whole RGBA mip chain (`size` down to one texel, level after level) as
/// BC7 blocks in the same order.
pub fn encode_chain(rgba: &[u8], size: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(chain_bytes(size));
    let mut offset = 0;
    for level in 0..=size.trailing_zeros() {
        let edge = size >> level;
        let bytes = (edge * edge * 4) as usize;
        out.extend(encode_level(&rgba[offset..offset + bytes], edge, edge));
        offset += bytes;
    }
    out
}

/// `layers` mip chains of `size`, layer after layer, as BC7, encoded on all the cores.
pub fn encode_layers(rgba: &[u8], size: u32, layers: u32) -> Vec<u8> {
    let layer_bytes = rgba.len() / layers.max(1) as usize;
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let per_thread = (layers as usize).div_ceil(threads).max(1);
    let mut out = Vec::with_capacity(chain_bytes(size) * layers as usize);
    std::thread::scope(|scope| {
        let jobs: Vec<_> = rgba
            .chunks(layer_bytes * per_thread)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .chunks(layer_bytes)
                        .flat_map(|layer| encode_chain(layer, size))
                        .collect::<Vec<u8>>()
                })
            })
            .collect();
        for job in jobs {
            out.extend(job.join().unwrap_or_default());
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Bits(u128, u32);
    impl Bits {
        fn take(&mut self, count: u32) -> i32 {
            let v = (self.0 >> self.1) & ((1u128 << count) - 1);
            self.1 += count;
            v as i32
        }
    }

    fn decode(block: &[u8; 16]) -> [[i32; 4]; 16] {
        let mut bits = Bits(u128::from_le_bytes(*block), 0);
        assert_eq!(bits.take(7), 0x40, "mode 6");
        let mut q = [[0; 2]; 4];
        for channel in &mut q {
            for end in channel.iter_mut() {
                *end = bits.take(7);
            }
        }
        let p = [bits.take(1), bits.take(1)];
        let end = |e: usize, c: usize| (q[c][e] << 1) | p[e];
        std::array::from_fn(|i| {
            let index = bits.take(if i == 0 { 3 } else { 4 }) as usize;
            let w = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64][index];
            std::array::from_fn(|c| ((64 - w) * end(0, c) + w * end(1, c) + 32) >> 6)
        })
    }

    fn within(block: &[[u8; 4]; 16], color_slack: i32, alpha_slack: i32) {
        let decoded = decode(&encode_block(block));
        for (want, got) in block.iter().zip(&decoded) {
            for c in 0..3 {
                assert!(
                    (i32::from(want[c]) - got[c]).abs() <= color_slack,
                    "{want:?} became {got:?}"
                );
            }
            assert!((i32::from(want[3]) - got[3]).abs() <= alpha_slack);
        }
    }

    #[test]
    fn transparent_black_is_the_mode_bit_alone() {
        let mut expected = [0u8; 16];
        expected[0] = 0x40;
        assert_eq!(encode_block(&[[0; 4]; 16]), expected);
    }

    #[test]
    fn flat_blocks_keep_their_alpha_and_colors_within_one() {
        within(&[[200, 11, 64, 255]; 16], 1, 0);
        within(&[[7, 8, 9, 0]; 16], 1, 0);
        within(&[[255, 255, 255, 255]; 16], 0, 0);
    }

    #[test]
    fn two_colour_cutouts_keep_every_alpha() {
        let mut block = [[34, 139, 34, 255]; 16];
        for i in [0, 3, 5, 6, 10, 15] {
            block[i] = [0, 0, 0, 0];
        }
        within(&block, 1, 0);
        let mut grass = [[90, 200, 60, 255]; 16];
        grass[0] = [30, 90, 20, 255];
        grass[9] = [30, 90, 20, 255];
        within(&grass, 1, 0);
    }

    #[test]
    fn a_noisy_block_stays_close_and_cutout_edges_hold() {
        let mut state = 12345u32;
        let mut block = [[0u8; 4]; 16];
        for (i, p) in block.iter_mut().enumerate() {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let v = 100 + (state >> 24) as u8 % 24;
            *p = [v, v / 2 + 10, 90, if i % 5 == 0 { 0 } else { 255 }];
        }
        let decoded = decode(&encode_block(&block));
        let mut worst = 0;
        for (want, got) in block.iter().zip(&decoded) {
            for c in 0..3 {
                worst = worst.max((i32::from(want[c]) - got[c]).abs());
            }
            assert_eq!(i32::from(want[3]), got[3]);
        }
        assert!(worst <= 12, "worst channel error {worst}");
    }

    #[test]
    fn a_gradient_is_reproduced_within_the_step_size() {
        let block: [[u8; 4]; 16] = std::array::from_fn(|i| {
            let v = (i * 16) as u8;
            [v, 255 - v, 128, 255]
        });
        within(&block, 9, 0);
    }

    #[test]
    fn levels_pad_small_sizes_to_whole_blocks_and_repeat_edges() {
        assert_eq!(level_bytes(16, 16), 16 * 16);
        assert_eq!(level_bytes(2, 2), 16);
        assert_eq!(level_bytes(1, 1), 16);
        let level = [10, 20, 30, 255];
        let blocks = encode_level(&level, 1, 1);
        assert_eq!(blocks.len(), 16);
        let decoded = decode(blocks.as_slice().try_into().unwrap());
        for p in decoded {
            assert!((p[0] - 10).abs() <= 1 && (p[1] - 20).abs() <= 1 && p[3] == 255);
        }
    }

    #[test]
    fn a_chain_has_the_size_of_its_blocks_and_layers_do_not_mix() {
        assert_eq!(chain_bytes(16), (16 + 4 + 1 + 1 + 1) * 16);
        let layer = |rgba: [u8; 4]| -> Vec<u8> {
            (0..=4)
                .flat_map(|l| std::iter::repeat_n(rgba, 1usize << (8 - 2 * l)))
                .flatten()
                .collect()
        };
        let red = layer([250, 0, 0, 255]);
        let blue = layer([0, 0, 250, 255]);
        let both = [red.clone(), blue.clone()].concat();
        let packed = encode_layers(&both, 16, 2);
        assert_eq!(packed.len(), 2 * chain_bytes(16));
        assert_eq!(packed[..chain_bytes(16)], encode_chain(&red, 16)[..]);
        assert_eq!(packed[chain_bytes(16)..], encode_chain(&blue, 16)[..]);
        let first: [u8; 16] = packed[..16].try_into().unwrap();
        assert!(decode(&first).iter().all(|p| p[2] <= 1 && p[0] >= 249));
    }
}
