use super::{TILED_MODE, TILED_UV_STEPS, TerrainVertex, UV_STEPS};
use std::cell::RefCell;

pub(super) const CELL: u16 = 1024;
pub(super) const MAX_RUN: usize = 16;
pub(super) const GRID: usize = 64;
const FULL_UV: u16 = UV_STEPS as u16;
const TILE_UV: u16 = TILED_UV_STEPS as u16;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    axis: u8,
    plane: u16,
    sig: u16,
    layer: u16,
    color: u32,
    light: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Cell {
    key: Key,
    t: u8,
    s: u8,
    index: u32,
}

#[derive(Default)]
struct Scratch {
    cells: Vec<Cell>,
    out: Vec<TerrainVertex>,
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

pub(super) struct Shape {
    pub(super) axis: usize,
    pub(super) low: [u16; 3],
    bits: [[bool; 2]; 4],
    uv_bits: [[bool; 2]; 4],
    follow: [usize; 2],
}

pub(super) fn shape(quad: &[TerrainVertex; 4], tiled: bool) -> Option<Shape> {
    let first = &quad[0];
    let uv_one = if tiled { 0 } else { FULL_UV };
    if quad.iter().any(|v| {
        v.layer != first.layer
            || v.color != first.color
            || v.light != first.light
            || (v.mode == TILED_MODE) != tiled
            || (v.mode & !TILED_MODE) != 0
    }) {
        return None;
    }
    let mut axes = (0..3).filter(|&a| quad.iter().all(|v| v.pos[a] == first.pos[a]));
    let axis = axes.next()?;
    if axes.next().is_some() {
        return None;
    }
    let (s, t) = ((axis + 1) % 3, (axis + 2) % 3);
    let mut low = first.pos;
    let mut high = first.pos;
    for v in quad {
        for a in 0..3 {
            low[a] = low[a].min(v.pos[a]);
            high[a] = high[a].max(v.pos[a]);
        }
    }
    if !low[s].is_multiple_of(CELL) || !low[t].is_multiple_of(CELL) {
        return None;
    }
    if !tiled && (high[s] - low[s] != CELL || high[t] - low[t] != CELL) {
        return None;
    }
    let mut bits = [[false; 2]; 4];
    let mut seen = 0u8;
    for (i, v) in quad.iter().enumerate() {
        bits[i] = [v.pos[s] == high[s], v.pos[t] == high[t]];
        if (v.pos[s] != low[s] && v.pos[s] != high[s])
            || (v.pos[t] != low[t] && v.pos[t] != high[t])
        {
            return None;
        }
        seen |= 1 << (usize::from(bits[i][0]) * 2 + usize::from(bits[i][1]));
    }
    if seen != 0xF {
        return None;
    }
    let mut uv_bits = [[false; 2]; 4];
    for (i, v) in quad.iter().enumerate() {
        for k in 0..2 {
            let raw = v.uv[k];
            if raw != 0 && !(tiled && raw > 0) && raw != uv_one {
                return None;
            }
            uv_bits[i][k] = raw != 0;
        }
    }
    let mut follow = [0usize; 2];
    for (k, f) in follow.iter_mut().enumerate() {
        let tracks = |axis: usize| {
            let same = (0..4).all(|i| uv_bits[i][k] == bits[i][axis]);
            let inverse = (0..4).all(|i| uv_bits[i][k] != bits[i][axis]);
            same || inverse
        };
        *f = match (tracks(0), tracks(1)) {
            (true, false) => 0,
            (false, true) => 1,
            _ => return None,
        };
    }
    Some(Shape {
        axis,
        low,
        bits,
        uv_bits,
        follow,
    })
}

pub(super) fn signature(shape: &Shape) -> u16 {
    (0..4).fold(0, |sig, i| {
        let b = shape.bits[i];
        let u = shape.uv_bits[i];
        let nibble =
            u16::from(b[0]) | u16::from(b[1]) << 1 | u16::from(u[0]) << 2 | u16::from(u[1]) << 3;
        sig | nibble << (4 * i)
    })
}

pub(super) fn build(
    template: &[TerrainVertex; 4],
    shape: &Shape,
    origin: [u16; 2],
    count: [usize; 2],
    tiled: bool,
) -> [TerrainVertex; 4] {
    let (s, t) = ((shape.axis + 1) % 3, (shape.axis + 2) % 3);
    let mut quad = *template;
    for (i, v) in quad.iter_mut().enumerate() {
        let b = shape.bits[i];
        v.pos[s] = origin[0] + u16::from(b[0]) * count[0] as u16 * CELL;
        v.pos[t] = origin[1] + u16::from(b[1]) * count[1] as u16 * CELL;
        for k in 0..2 {
            v.uv[k] = if !shape.uv_bits[i][k] {
                0
            } else if tiled {
                TILE_UV * count[shape.follow[k]] as u16
            } else {
                FULL_UV
            };
        }
        v.mode = if tiled { TILED_MODE } else { 0 };
    }
    quad
}

pub fn merge(solid: &mut Vec<TerrainVertex>) {
    SCRATCH.with(|scratch| {
        let scratch = &mut *scratch.borrow_mut();
        merge_with(solid, scratch);
    });
}

fn merge_with(solid: &mut Vec<TerrainVertex>, scratch: &mut Scratch) {
    let Scratch { cells, out } = scratch;
    let (quads, rest) = solid.as_chunks::<4>();
    if !rest.is_empty() || quads.len() < 2 {
        return;
    }
    cells.clear();
    out.clear();
    for (index, quad) in quads.iter().enumerate() {
        let Some(shape) = shape(quad, false) else {
            out.extend_from_slice(quad);
            continue;
        };
        let (s, t) = ((shape.axis + 1) % 3, (shape.axis + 2) % 3);
        cells.push(Cell {
            key: Key {
                axis: shape.axis as u8,
                plane: quad[0].pos[shape.axis],
                sig: signature(&shape),
                layer: quad[0].layer,
                color: quad[0].color,
                light: quad[0].light,
            },
            t: (shape.low[t] / CELL) as u8,
            s: (shape.low[s] / CELL) as u8,
            index: index as u32,
        });
    }
    if cells.len() < 2 {
        return;
    }
    cells.sort_unstable();
    let mut rows = [0u64; GRID];
    let mut slots = [0u32; GRID * GRID];
    let mut start = 0;
    while start < cells.len() {
        let key = cells[start].key;
        let end = start + cells[start..].partition_point(|c| c.key == key);
        let group = &cells[start..end];
        start = end;
        rows.fill(0);
        for cell in group {
            let (s, t) = (usize::from(cell.s), usize::from(cell.t));
            if rows[t] >> s & 1 == 1 {
                out.extend_from_slice(&quads[cell.index as usize]);
                continue;
            }
            rows[t] |= 1 << s;
            slots[t * GRID + s] = cell.index;
        }
        for t in 0..GRID {
            while rows[t] != 0 {
                let s = rows[t].trailing_zeros() as usize;
                let width = ((rows[t] >> s).trailing_ones() as usize).min(MAX_RUN);
                let mask = (u64::MAX >> (GRID - width)) << s;
                let mut height = 1;
                while height < MAX_RUN && t + height < GRID && rows[t + height] & mask == mask {
                    height += 1;
                }
                for row in &mut rows[t..t + height] {
                    *row &= !mask;
                }
                let template = &quads[slots[t * GRID + s] as usize];
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

pub(super) fn shape_of(quad: &[TerrainVertex; 4]) -> Shape {
    shape(quad, false).expect("a merged cell has a shape")
}

pub fn split(quad: &[TerrainVertex; 4], mut emit: impl FnMut([TerrainVertex; 4])) {
    if quad[0].mode != TILED_MODE {
        emit(*quad);
        return;
    }
    let Some(shape) = shape(quad, true) else {
        emit(*quad);
        return;
    };
    let (s, t) = ((shape.axis + 1) % 3, (shape.axis + 2) % 3);
    let high = |a: usize| quad.iter().map(|v| v.pos[a]).max().unwrap_or(0);
    let counts = [
        usize::from((high(s) - shape.low[s]) / CELL),
        usize::from((high(t) - shape.low[t]) / CELL),
    ];
    let mut unit = *quad;
    for v in &mut unit {
        v.mode = 0;
    }
    for j in 0..counts[1] {
        for i in 0..counts[0] {
            let origin = [
                shape.low[s] + i as u16 * CELL,
                shape.low[t] + j as u16 * CELL,
            ];
            let mut cell = unit;
            for (n, v) in cell.iter_mut().enumerate() {
                let b = shape.bits[n];
                v.pos[s] = origin[0] + u16::from(b[0]) * CELL;
                v.pos[t] = origin[1] + u16::from(b[1]) * CELL;
                for k in 0..2 {
                    v.uv[k] = if shape.uv_bits[n][k] { FULL_UV } else { 0 };
                }
            }
            emit(cell);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: u32 = 0xFFFF_FFFF;

    fn wall(x: i32, y: i32, color: u32, light: u16) -> [TerrainVertex; 4] {
        let corner = |dx: i32, dy: i32, u: f32, v: f32| {
            TerrainVertex::new(
                [(x + dx) as f32, (y + dy) as f32, 3.0],
                [u, v],
                5,
                color,
                light,
            )
        };
        [
            corner(0, 0, 0.0, 1.0),
            corner(0, 1, 0.0, 0.0),
            corner(1, 1, 1.0, 0.0),
            corner(1, 0, 1.0, 1.0),
        ]
    }

    fn mesh(cells: impl Iterator<Item = [TerrainVertex; 4]>) -> Vec<TerrainVertex> {
        cells.flatten().collect()
    }

    fn area(solid: &[TerrainVertex]) -> f32 {
        solid
            .as_chunks::<4>()
            .0
            .iter()
            .map(|q| {
                let [a, b, c, _] = q.map(|v| v.pos());
                let e = |p: [f32; 3], r: [f32; 3]| [p[0] - r[0], p[1] - r[1], p[2] - r[2]];
                let (u, w) = (e(b, a), e(c, a));
                let n = [
                    u[1] * w[2] - u[2] * w[1],
                    u[2] * w[0] - u[0] * w[2],
                    u[0] * w[1] - u[1] * w[0],
                ];
                (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
            })
            .sum()
    }

    fn unit_set(solid: &[TerrainVertex]) -> Vec<[(u16, u16, u16, u16, u16); 4]> {
        let mut all = Vec::new();
        for quad in solid.as_chunks::<4>().0 {
            split(quad, |q| {
                all.push(q.map(|v| (v.pos[0], v.pos[1], v.pos[2], v.uv[0], v.uv[1])));
            });
        }
        all.sort_unstable();
        all
    }

    fn grid(w: i32, h: i32) -> Vec<TerrainVertex> {
        mesh((0..h).flat_map(|y| (0..w).map(move |x| wall(x, y, WHITE, 0x00F0))))
    }

    #[test]
    fn a_flat_wall_of_one_block_type_is_one_quad() {
        let mut solid = grid(7, 5);
        let before = unit_set(&solid);
        let area_before = area(&solid);
        merge(&mut solid);
        assert_eq!(solid.len(), 4);
        assert_eq!(solid[0].mode, TILED_MODE);
        assert_eq!(area(&solid), area_before);
        assert_eq!(unit_set(&solid), before);
        let uv: Vec<_> = solid.iter().map(|v| v.uv()).collect();
        assert!(uv.contains(&[0.0, 5.0]) && uv.contains(&[7.0, 0.0]));
    }

    #[test]
    fn runs_stop_at_the_section_width() {
        let mut solid = grid(16, 16);
        merge(&mut solid);
        assert_eq!(solid.len(), 4);
        let mut solid = mesh((0..40).map(|x| wall(x, 0, WHITE, 0x00F0)));
        let before = unit_set(&solid);
        merge(&mut solid);
        assert_eq!(solid.len(), 12);
        assert_eq!(unit_set(&solid), before);
    }

    #[test]
    fn different_light_colour_or_layer_prevents_merging() {
        let mut solid = mesh(
            [
                wall(0, 0, WHITE, 0x00F0),
                wall(1, 0, WHITE, 0x00E0),
                wall(2, 0, 0xFF80_8080, 0x00F0),
            ]
            .into_iter(),
        );
        merge(&mut solid);
        assert_eq!(solid.len(), 12);
        let mut other = wall(1, 0, WHITE, 0x00F0);
        for v in &mut other {
            v.layer = 6;
        }
        let mut solid = mesh([wall(0, 0, WHITE, 0x00F0), other].into_iter());
        merge(&mut solid);
        assert_eq!(solid.len(), 8);
    }

    #[test]
    fn a_corner_that_differs_prevents_merging() {
        let mut shaded = wall(1, 0, WHITE, 0x00F0);
        shaded[2].color = 0xFF40_4040;
        let mut lit = wall(2, 0, WHITE, 0x00F0);
        lit[1].light = 0x0010;
        let mut solid = mesh([wall(0, 0, WHITE, 0x00F0), shaded, lit].into_iter());
        let before = solid.clone();
        merge(&mut solid);
        assert_eq!(solid.len(), 12);
        assert_eq!(unit_set(&solid), unit_set(&before));
    }

    #[test]
    fn gaps_and_odd_shapes_cover_the_same_area() {
        let cells = [
            (0, 0),
            (1, 0),
            (2, 0),
            (0, 1),
            (1, 1),
            (3, 1),
            (0, 2),
            (5, 5),
        ];
        let mut solid = mesh(cells.iter().map(|&(x, y)| wall(x, y, WHITE, 0x00F0)));
        let before = unit_set(&solid);
        let area_before = area(&solid);
        merge(&mut solid);
        assert!(solid.len() < cells.len() * 4);
        assert_eq!(area(&solid), area_before);
        assert_eq!(unit_set(&solid), before);
    }

    #[test]
    fn a_block_offset_wall_does_not_merge() {
        let shifted = |x: f32| {
            let mut q = wall(0, 0, WHITE, 0x00F0);
            for v in &mut q {
                let p = v.pos();
                *v = TerrainVertex::new([p[0] + x, p[1], p[2]], v.uv(), 5, WHITE, 0x00F0);
            }
            q
        };
        let mut solid = mesh([shifted(0.0), shifted(1.0 + 1.0 / 16.0)].into_iter());
        merge(&mut solid);
        assert_eq!(solid.len(), 8);
    }

    #[test]
    fn rotated_and_flipped_textures_merge_only_with_their_own_kind() {
        let mut turned = wall(1, 0, WHITE, 0x00F0);
        for v in &mut turned {
            v.uv = [v.uv[1], v.uv[0]];
        }
        let mut solid = mesh(
            [
                wall(0, 0, WHITE, 0x00F0),
                turned,
                wall(2, 0, WHITE, 0x00F0),
                wall(3, 0, WHITE, 0x00F0),
            ]
            .into_iter(),
        );
        let before = unit_set(&solid);
        merge(&mut solid);
        assert_eq!(solid.len(), 12);
        assert_eq!(unit_set(&solid), before);
    }

    #[test]
    fn tilted_and_partial_uv_quads_pass_through() {
        let mut half = wall(0, 0, WHITE, 0x00F0);
        half[2].uv[0] = 16384;
        let mut solid = mesh([half, wall(1, 0, WHITE, 0x00F0)].into_iter());
        let before = solid.clone();
        merge(&mut solid);
        assert_eq!(solid.len(), before.len());
    }
}
