use super::TerrainVertex;
use super::greedy::{CELL, GRID, MAX_RUN, build, shape, shape_of, signature};
use std::cell::RefCell;

const INDEX_BITS: u32 = 20;
const AXIS_SHIFT: u32 = 32 + 16;
const PLANE_SHIFT: u32 = 32;
const T_SHIFT: u32 = 26;
const S_SHIFT: u32 = 20;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Attr {
    sig: u16,
    layer: u16,
    light: u16,
    color: u32,
}

#[derive(Default)]
struct Scratch {
    keys: Vec<u64>,
    attrs: Vec<Attr>,
    out: Vec<TerrainVertex>,
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

pub fn merge(solid: &mut Vec<TerrainVertex>) {
    SCRATCH.with(|scratch| merge_with(solid, &mut scratch.borrow_mut()));
}

fn merge_with(solid: &mut Vec<TerrainVertex>, scratch: &mut Scratch) {
    let Scratch { keys, attrs, out } = scratch;
    let (quads, rest) = solid.as_chunks::<4>();
    if !rest.is_empty() || quads.len() < 2 || quads.len() >= 1 << INDEX_BITS {
        return;
    }
    keys.clear();
    attrs.clear();
    out.clear();
    for (index, quad) in quads.iter().enumerate() {
        let placed = shape(quad, false).and_then(|shape| {
            let (s, t) = ((shape.axis + 1) % 3, (shape.axis + 2) % 3);
            let (cs, ct) = (shape.low[s] / CELL, shape.low[t] / CELL);
            (usize::from(cs) < GRID && usize::from(ct) < GRID).then_some((shape, cs, ct))
        });
        let Some((shape, cs, ct)) = placed else {
            out.extend_from_slice(quad);
            attrs.push(Attr {
                sig: 0,
                layer: 0,
                light: 0,
                color: 0,
            });
            continue;
        };
        attrs.push(Attr {
            sig: signature(&shape),
            layer: quad[0].layer,
            light: quad[0].light,
            color: quad[0].color,
        });
        keys.push(
            (shape.axis as u64) << AXIS_SHIFT
                | u64::from(quad[0].pos[shape.axis]) << PLANE_SHIFT
                | u64::from(ct) << T_SHIFT
                | u64::from(cs) << S_SHIFT
                | index as u64,
        );
    }
    if keys.len() < 2 {
        return;
    }
    keys.sort_unstable();
    let mut rows = [0u64; GRID];
    let mut slots = [0u32; GRID * GRID];
    let mut start = 0;
    while start < keys.len() {
        let group = keys[start] >> PLANE_SHIFT;
        let end = start + keys[start..].partition_point(|k| k >> PLANE_SHIFT == group);
        rows.fill(0);
        for &key in &keys[start..end] {
            let index = (key & ((1 << INDEX_BITS) - 1)) as u32;
            let t = (key >> T_SHIFT & 63) as usize;
            let s = (key >> S_SHIFT & 63) as usize;
            if rows[t] >> s & 1 == 1 {
                out.extend_from_slice(&quads[index as usize]);
                continue;
            }
            rows[t] |= 1 << s;
            slots[t * GRID + s] = index;
        }
        start = end;
        for t in 0..GRID {
            while rows[t] != 0 {
                let s = rows[t].trailing_zeros() as usize;
                let anchor = slots[t * GRID + s];
                let attr = attrs[anchor as usize];
                let reach = ((rows[t] >> s).trailing_ones() as usize).min(MAX_RUN);
                let mut width = 1;
                while width < reach && attrs[slots[t * GRID + s + width] as usize] == attr {
                    width += 1;
                }
                let mask = (u64::MAX >> (GRID - width)) << s;
                let mut height = 1;
                while height < MAX_RUN
                    && t + height < GRID
                    && rows[t + height] & mask == mask
                    && (s..s + width)
                        .all(|c| attrs[slots[(t + height) * GRID + c] as usize] == attr)
                {
                    height += 1;
                }
                for row in &mut rows[t..t + height] {
                    *row &= !mask;
                }
                let template = &quads[anchor as usize];
                if width == 1 && height == 1 {
                    out.extend_from_slice(template);
                    continue;
                }
                let shape = shape_of(template);
                let (a, b) = ((shape.axis + 1) % 3, (shape.axis + 2) % 3);
                let origin = [shape.low[a], shape.low[b]];
                out.extend_from_slice(&build(template, &shape, origin, [width, height], true));
            }
        }
    }
    solid.clear();
    solid.extend_from_slice(out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesher::{TILED_MODE, greedy};

    fn face(axis: usize, plane: i32, a: i32, b: i32, kind: u32) -> [TerrainVertex; 4] {
        let corner = |da: i32, db: i32, u: f32, v: f32| {
            let mut p = [0.0f32; 3];
            p[axis] = plane as f32;
            p[(axis + 1) % 3] = (a + da) as f32;
            p[(axis + 2) % 3] = (b + db) as f32;
            TerrainVertex::new(
                p,
                [u, v],
                (kind % 3) as u16 + 1,
                0xFF00_0000 | ((kind / 3 % 2) * 0x40),
                ((kind / 6) % 2) as u16 * 0x10,
            )
        };
        [
            corner(0, 0, 0.0, 1.0),
            corner(0, 1, 0.0, 0.0),
            corner(1, 1, 1.0, 0.0),
            corner(1, 0, 1.0, 1.0),
        ]
    }

    fn units(solid: &[TerrainVertex]) -> Vec<[(u16, u16, u16, u16, u16, u16, u32, u16); 4]> {
        let mut all = Vec::new();
        for quad in solid.as_chunks::<4>().0 {
            greedy::split(quad, |q| {
                all.push(q.map(|v| {
                    (
                        v.pos[0], v.pos[1], v.pos[2], v.uv[0], v.uv[1], v.layer, v.color, v.light,
                    )
                }))
            });
        }
        all.sort_unstable();
        all
    }

    fn quad_set(
        solid: &[TerrainVertex],
    ) -> Vec<Vec<(u16, u16, u16, u16, u16, u16, u32, u16, u16)>> {
        let mut all: Vec<_> = solid
            .as_chunks::<4>()
            .0
            .iter()
            .map(|q| {
                q.iter()
                    .map(|v| {
                        (
                            v.pos[0], v.pos[1], v.pos[2], v.uv[0], v.uv[1], v.layer, v.color,
                            v.light, v.mode,
                        )
                    })
                    .collect()
            })
            .collect();
        all.sort_unstable();
        all
    }

    fn random_section(seed: u64, density: u64, kinds: u32) -> Vec<TerrainVertex> {
        let mut state = seed;
        let mut next = move || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            state >> 33
        };
        let mut mesh = Vec::new();
        for axis in 0..3 {
            for plane in 0..=16 {
                for b in 0..16 {
                    for a in 0..16 {
                        if next() % 100 < density {
                            let kind = (next() % u64::from(kinds)) as u32;
                            mesh.extend(face(axis, plane, a, b, kind));
                        }
                    }
                }
            }
        }
        mesh
    }

    #[test]
    #[ignore = "timing, run with --profile fast -- --ignored --nocapture"]
    fn merge_throughput_against_the_sorting_path() {
        for (density, kinds) in [(95, 1), (60, 3), (30, 6)] {
            let source = random_section(9, density, kinds);
            let time = |merge: fn(&mut Vec<TerrainVertex>)| {
                let started = std::time::Instant::now();
                let mut quads = 0;
                for _ in 0..300 {
                    let mut mesh = source.clone();
                    merge(&mut mesh);
                    quads = mesh.len() / 4;
                }
                (300.0 / started.elapsed().as_secs_f64(), quads)
            };
            let (sorted, q1) = time(greedy::merge);
            let (binary, q2) = time(merge);
            println!(
                "density {density} kinds {kinds}: in {} quads, out {q1}/{q2}, sorting {sorted:.0}/s, binary {binary:.0}/s",
                source.len() / 4
            );
        }
    }

    #[test]
    fn a_flat_wall_is_one_tiled_quad() {
        let mut solid: Vec<_> = (0..5)
            .flat_map(|y| (0..7).flat_map(move |x| face(2, 3, x, y, 0)))
            .collect();
        let before = units(&solid);
        merge(&mut solid);
        assert_eq!(solid.len(), 4);
        assert_eq!(solid[0].mode, TILED_MODE);
        let uv: Vec<_> = solid.iter().map(|v| v.uv()).collect();
        assert!(uv.contains(&[0.0, 5.0]) && uv.contains(&[7.0, 0.0]));
        assert_eq!(units(&solid), before);
    }

    #[test]
    fn runs_stop_at_the_section_width() {
        let mut solid: Vec<_> = (0..40).flat_map(|x| face(2, 3, x, 0, 0)).collect();
        let before = units(&solid);
        merge(&mut solid);
        assert_eq!(solid.len(), 12);
        assert_eq!(units(&solid), before);
    }

    #[test]
    fn matches_the_sorting_merge_on_random_sections() {
        for (seed, density, kinds) in [
            (1, 100, 1),
            (2, 90, 2),
            (3, 70, 4),
            (4, 50, 12),
            (5, 20, 3),
            (6, 97, 12),
        ] {
            let source = random_section(seed, density, kinds);
            let mut sorted = source.clone();
            let mut binary = source.clone();
            greedy::merge(&mut sorted);
            merge(&mut binary);
            assert_eq!(units(&binary), units(&source), "coverage, seed {seed}");
            assert_eq!(quad_set(&binary), quad_set(&sorted), "quads, seed {seed}");
            assert!(binary.len() <= source.len());
        }
    }

    #[test]
    fn overlapping_quads_with_different_attributes_both_survive() {
        let mut solid: Vec<_> = face(2, 3, 4, 4, 0)
            .into_iter()
            .chain(face(2, 3, 4, 4, 1))
            .chain(face(2, 3, 5, 4, 0))
            .collect();
        let before = units(&solid);
        merge(&mut solid);
        assert_eq!(units(&solid), before);
        assert_eq!(solid.len(), 8);
    }

    #[test]
    fn identical_overlapping_quads_are_kept() {
        let mut solid: Vec<_> = face(0, 2, 1, 1, 0)
            .into_iter()
            .chain(face(0, 2, 1, 1, 0))
            .collect();
        merge(&mut solid);
        assert_eq!(solid.len(), 8);
    }

    #[test]
    fn partial_and_half_height_planes_merge_like_the_sorting_path() {
        let mut half = face(1, 4, 0, 0, 0);
        let mut other = face(1, 4, 1, 0, 0);
        for v in half.iter_mut().chain(other.iter_mut()) {
            v.pos[1] -= 512;
        }
        let mut sorted: Vec<_> = half.into_iter().chain(other).collect();
        let mut binary = sorted.clone();
        greedy::merge(&mut sorted);
        merge(&mut binary);
        assert_eq!(sorted.len(), 4);
        assert_eq!(quad_set(&binary), quad_set(&sorted));
    }
}
