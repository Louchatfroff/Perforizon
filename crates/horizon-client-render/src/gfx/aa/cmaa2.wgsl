const EDGE_THRESHOLD: f32 = 0.07;
const LOCAL_CONTRAST_ADAPTATION: f32 = 0.10;
const SIMPLE_SHAPE_BLURINESS: f32 = 0.10;
const SYMMETRY_CORRECTION_OFFSET: f32 = 0.22;
const DAMPENING_EFFECT: f32 = 0.15;
const MAX_LINE_LENGTH: f32 = 86.0;

const KERNEL: u32 = 16u;
const OUTPUT_KERNEL: i32 = 14;
const PROCESS_THREADS: u32 = 128u;
const BLEND_SLOTS: u32 = 768u;
const NO_ITEM: u32 = 0xFFFFFFFFu;

const CONTROL_COUNT: u32 = 3u;
const CONTROL_CANDIDATES: u32 = 4u;
const CONTROL_LOCATIONS: u32 = 8u;
const CONTROL_ITEMS: u32 = 12u;

alias Neighbourhood = array<array<vec2<f32>, 4>, 4>;

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var edges_tex: texture_storage_2d<r32uint, read_write>;
@group(0) @binding(3) var<storage, read_write> candidates: array<u32>;
@group(0) @binding(4) var<storage, read_write> locations: array<u32>;
@group(0) @binding(5) var<storage, read_write> items: array<vec2<u32>>;
@group(0) @binding(6) var<storage, read_write> heads: array<atomic<u32>>;
@group(0) @binding(7) var<storage, read_write> control: array<atomic<u32>, 16>;
@group(0) @binding(8) var blit_src: texture_2d<f32>;
@group(1) @binding(0) var<storage, read_write> indirect_args: array<u32, 3>;

var<workgroup> shared_edges_h: array<vec4<f32>, 256>;
var<workgroup> shared_edges_v: array<vec4<f32>, 256>;
var<workgroup> blend_count: atomic<u32>;
var<workgroup> blend_items: array<vec2<u32>, 768>;

fn image_size() -> vec2<i32> {
    return vec2<i32>(textureDimensions(src));
}

fn quad_stride() -> u32 {
    return (textureDimensions(src).x + 1u) / 2u;
}

fn load_color(p: vec2<i32>) -> vec4<f32> {
    return textureLoad(src, clamp(p, vec2<i32>(0), image_size() - vec2<i32>(1)), 0);
}

fn luma_for_edges(rgb: vec3<f32>) -> f32 {
    return dot(rgb, vec3<f32>(0.299, 0.587, 0.114));
}

fn pack_edges(edges: vec4<f32>) -> u32 {
    return u32(dot(edges, vec4<f32>(1.0, 2.0, 4.0, 8.0)));
}

fn unpack_edges(value: u32) -> vec4<f32> {
    return vec4<f32>(
        f32((value & 1u) != 0u),
        f32((value & 2u) != 0u),
        f32((value & 4u) != 0u),
        f32((value & 8u) != 0u),
    );
}

fn load_edge(p: vec2<i32>) -> u32 {
    let size = image_size();
    if p.x < 0 || p.y < 0 || p.x >= size.x || p.y >= size.y {
        return 0u;
    }
    let texel = textureLoad(edges_tex, vec2<i32>(p.x >> 1u, p.y)).x;
    return (texel >> (u32(p.x & 1) * 4u)) & 0xFu;
}

fn f16_bits(x: f32) -> u32 {
    return pack2x16float(vec2<f32>(x, 0.0)) & 0xFFFFu;
}

fn f16_value(bits: u32) -> f32 {
    return unpack2x16float(bits & 0xFFFFu).x;
}

fn pack_color(rgb: vec3<f32>) -> u32 {
    let top = bitcast<f32>(0x3FFFFFFFu);
    let c = clamp(rgb, vec3<f32>(0.0), vec3<f32>(top));
    let r = ((f16_bits(c.r) + 4u) >> 3u) & 0x000007FFu;
    let g = ((f16_bits(c.g) + 4u) << 8u) & 0x003FF800u;
    let b = ((f16_bits(c.b) + 8u) << 18u) & 0xFFC00000u;
    return r | g | b;
}

fn unpack_color(packed: u32) -> vec3<f32> {
    let r = f16_value((packed << 3u) & 0x3FF8u);
    let g = f16_value((packed >> 8u) & 0x3FF8u);
    let b = f16_value((packed >> 18u) & 0x3FF0u);
    return vec3<f32>(r, g, b);
}

fn store_color_sample(pos: vec2<i32>, color: vec3<f32>, complex_shape: bool) {
    let size = image_size();
    if pos.x < 0 || pos.y < 0 || pos.x >= size.x || pos.y >= size.y {
        return;
    }
    let counter = atomicAdd(&control[CONTROL_ITEMS], 1u);
    let upos = vec2<u32>(pos);
    let quad = upos / vec2<u32>(2u);
    let offset_xy = (upos.y % 2u) * 2u + (upos.x % 2u);
    let header = (offset_xy << 30u) | (u32(complex_shape) << 26u);
    let original = atomicExchange(&heads[quad.y * quad_stride() + quad.x], counter | header);
    items[counter] = vec2<u32>(original, pack_color(color));
    if original == NO_ITEM {
        let location = atomicAdd(&control[CONTROL_LOCATIONS], 1u);
        locations[location] = (quad.x << 16u) | quad.y;
    }
}

fn step_edge(value: f32) -> f32 {
    return select(0.0, 1.0, value > EDGE_THRESHOLD);
}

fn load_quad(addr: u32) -> array<vec2<f32>, 4> {
    let h = shared_edges_h[addr];
    let v = shared_edges_v[addr];
    return array<vec2<f32>, 4>(
        vec2<f32>(v.x, h.x),
        vec2<f32>(v.y, h.y),
        vec2<f32>(v.z, h.z),
        vec2<f32>(v.w, h.w),
    );
}

fn contrast_v(x: i32, y: i32, n: Neighbourhood) -> f32 {
    return max(
        max(n[x + 1][y + 0].y, n[x + 1][y + 1].y),
        max(n[x + 2][y + 0].y, n[x + 2][y + 1].y),
    ) * LOCAL_CONTRAST_ADAPTATION;
}

fn contrast_h(x: i32, y: i32, n: Neighbourhood) -> f32 {
    return max(
        max(n[x + 0][y + 1].x, n[x + 1][y + 1].x),
        max(n[x + 0][y + 2].x, n[x + 1][y + 2].x),
    ) * LOCAL_CONTRAST_ADAPTATION;
}

@compute @workgroup_size(16, 16, 1)
fn edges_2x2(@builtin(workgroup_id) group_id: vec3<u32>, @builtin(local_invocation_id) local_id: vec3<u32>) {
    let size = image_size();
    let quad_count = (size + vec2<i32>(1)) / 2;
    let quad = vec2<i32>(group_id.xy) * OUTPUT_KERNEL + vec2<i32>(local_id.xy) - vec2<i32>(1);
    let pixel_pos = quad * 2;
    let center = local_id.x + local_id.y * KERNEL;
    let in_output_kernel = local_id.x != KERNEL - 1u && local_id.x != 0u && local_id.y != KERNEL - 1u && local_id.y != 0u && all(quad < quad_count);

    var l: array<f32, 8>;
    for (var i = 0; i < 8; i++) {
        l[i] = luma_for_edges(load_color(pixel_pos + vec2<i32>(i % 3, i / 3)).rgb);
    }
    let qe0 = vec2<f32>(abs(l[0] - l[1]), abs(l[0] - l[3]));
    let qe1 = vec2<f32>(abs(l[1] - l[2]), abs(l[1] - l[4]));
    let qe2 = vec2<f32>(abs(l[3] - l[4]), abs(l[3] - l[6]));
    let qe3 = vec2<f32>(abs(l[4] - l[5]), abs(l[4] - l[7]));

    shared_edges_v[center] = vec4<f32>(qe0.x, qe1.x, qe2.x, qe3.x);
    shared_edges_h[center] = vec4<f32>(qe0.y, qe1.y, qe2.y, qe3.y);

    workgroupBarrier();

    if !in_output_kernel {
        return;
    }

    var out_edges = array<u32, 4>(0u, 0u, 0u, 0u);
    var top_row = shared_edges_h[center - KERNEL].zw;
    var left_column = shared_edges_v[center - 1u].yw;
    let sum = vec4<f32>(qe0.x, qe0.y, qe1.x, qe1.y) + vec4<f32>(qe2.x, qe2.y, qe3.x, qe3.y) + vec4<f32>(top_row, left_column);

    if any(sum != vec4<f32>(0.0)) {
        atomicStore(&heads[u32(quad.y) * u32(quad_count.x) + u32(quad.x)], NO_ITEM);

        var n: Neighbourhood;
        let q_tl = load_quad(center - KERNEL - 1u);
        let q_t = load_quad(center - KERNEL);
        let q_tr = load_quad(center - KERNEL + 1u);
        let q_l = load_quad(center - 1u);
        let q_r = load_quad(center + 1u);
        let q_bl = load_quad(center - 1u + KERNEL);
        let q_b = load_quad(center + KERNEL);
        n[0][0] = q_tl[3];
        n[1][0] = q_t[2];
        n[2][0] = q_t[3];
        n[3][0] = q_tr[2];
        n[0][1] = q_l[1];
        n[0][2] = q_l[3];
        n[3][1] = q_r[0];
        n[3][2] = q_r[2];
        n[0][3] = q_bl[1];
        n[1][3] = q_b[0];
        n[2][3] = q_b[1];
        n[1][0].y = top_row[0];
        n[2][0].y = top_row[1];
        n[0][1].x = left_column[0];
        n[0][2].x = left_column[1];
        n[1][1] = qe0;
        n[2][1] = qe1;
        n[1][2] = qe2;
        n[2][2] = qe3;

        top_row[0] = step_edge(top_row[0] - contrast_h(0, -1, n));
        top_row[1] = step_edge(top_row[1] - contrast_h(1, -1, n));
        left_column[0] = step_edge(left_column[0] - contrast_v(-1, 0, n));
        left_column[1] = step_edge(left_column[1] - contrast_v(-1, 1, n));

        var ce: array<vec4<f32>, 4>;
        ce[0].x = step_edge(qe0.x - contrast_v(0, 0, n));
        ce[0].y = step_edge(qe0.y - contrast_h(0, 0, n));
        ce[1].x = step_edge(qe1.x - contrast_v(1, 0, n));
        ce[1].y = step_edge(qe1.y - contrast_h(1, 0, n));
        ce[2].x = step_edge(qe2.x - contrast_v(0, 1, n));
        ce[2].y = step_edge(qe2.y - contrast_h(0, 1, n));
        ce[3].x = step_edge(qe3.x - contrast_v(1, 1, n));
        ce[3].y = step_edge(qe3.y - contrast_h(1, 1, n));

        ce[0].z = left_column[0];
        ce[1].z = ce[0].x;
        ce[2].z = left_column[1];
        ce[3].z = ce[2].x;

        ce[0].w = top_row[0];
        ce[1].w = top_row[1];
        ce[2].w = ce[0].y;
        ce[3].w = ce[1].y;

        for (var i = 0u; i < 4u; i++) {
            let local_pos = pixel_pos + vec2<i32>(i32(i & 1u), i32(i >> 1u));
            let e = ce[i];
            let is_candidate = (e.x * e.y + e.y * e.z + e.z * e.w + e.w * e.x) != 0.0;
            if is_candidate && all(local_pos < size) {
                let index = atomicAdd(&control[CONTROL_CANDIDATES], 1u);
                candidates[index] = (u32(local_pos.x) << 18u) | u32(local_pos.y);
            }
            out_edges[i] = pack_edges(e);
        }
    }

    textureStore(edges_tex, vec2<i32>(pixel_pos.x / 2, pixel_pos.y), vec4<u32>((out_edges[1] << 4u) | out_edges[0], 0u, 0u, 0u));
    textureStore(edges_tex, vec2<i32>(pixel_pos.x / 2, pixel_pos.y + 1), vec4<u32>((out_edges[3] << 4u) | out_edges[2], 0u, 0u, 0u));

    for (var i = 0u; i < 4u; i++) {
        let p = pixel_pos + vec2<i32>(i32(i & 1u), i32(i >> 1u));
        if all(p < size) {
            textureStore(dst, p, textureLoad(src, p, 0));
        }
    }
}

@compute @workgroup_size(1, 1, 1)
fn compute_dispatch_args(@builtin(workgroup_id) group_id: vec3<u32>) {
    if group_id.x == 1u {
        let count = min(atomicLoad(&control[CONTROL_CANDIDATES]), arrayLength(&candidates));
        indirect_args[0] = (count + PROCESS_THREADS - 1u) / PROCESS_THREADS;
        indirect_args[1] = 1u;
        indirect_args[2] = 1u;
        atomicStore(&control[CONTROL_COUNT], count);
    } else if group_id.y == 1u {
        let count = min(atomicLoad(&control[CONTROL_LOCATIONS]), arrayLength(&locations));
        indirect_args[0] = 1u;
        indirect_args[1] = min((count + 31u) / 32u, 65535u);
        indirect_args[2] = 1u;
        atomicStore(&control[CONTROL_COUNT], count);
        atomicStore(&control[CONTROL_CANDIDATES], 0u);
        atomicStore(&control[CONTROL_LOCATIONS], 0u);
        atomicStore(&control[CONTROL_ITEMS], 0u);
    }
}

fn simple_shape_blend_values(edges: vec4<f32>, edges_left: vec4<f32>, edges_right: vec4<f32>, edges_top: vec4<f32>, edges_bottom: vec4<f32>) -> vec4<f32> {
    var from_right = edges.r;
    var from_below = edges.g;
    var from_left = edges.b;
    var from_above = edges.a;

    var blur = SIMPLE_SHAPE_BLURINESS;

    let number_of_edges = dot(edges, vec4<f32>(1.0));
    let number_of_edges_all_around = dot(edges_left.bga + edges_right.rga + edges_top.rba + edges_bottom.rgb, vec3<f32>(1.0));

    if number_of_edges == 2.0 {
        blur *= 0.75;
        let k = 0.9;
        from_right += k * (edges.g * edges_top.r * (1.0 - edges_left.g) + edges.a * edges_bottom.r * (1.0 - edges_left.a));
        from_below += k * (edges.b * edges_right.g * (1.0 - edges_top.b) + edges.r * edges_left.g * (1.0 - edges_top.r));
        from_left += k * (edges.a * edges_bottom.b * (1.0 - edges_right.a) + edges.g * edges_top.b * (1.0 - edges_right.g));
        from_above += k * (edges.r * edges_left.a * (1.0 - edges_bottom.r) + edges.b * edges_right.a * (1.0 - edges_bottom.b));
    }

    blur *= saturate(1.30 - number_of_edges_all_around / 10.0);

    return vec4<f32>(from_left, from_above, from_right, from_below) * blur;
}

fn detect_zs_horizontal(edges: vec4<f32>, edges_m1p0: vec4<f32>, edges_p1p0: vec4<f32>, edges_p2p0: vec4<f32>) -> vec2<f32> {
    var inverted = edges.r * edges.g * edges_p1p0.a;
    inverted *= 2.0 + (edges_m1p0.g + edges_p2p0.a) - (edges.a + edges_p1p0.g) - 0.7 * (edges_p2p0.g + edges_m1p0.a + edges.b + edges_p1p0.r);

    var normal = edges.r * edges.a * edges_p1p0.g;
    normal *= 2.0 + (edges_m1p0.a + edges_p2p0.g) - (edges.g + edges_p1p0.a) - 0.7 * (edges_p2p0.a + edges_m1p0.g + edges.b + edges_p1p0.r);

    return vec2<f32>(inverted, normal);
}

fn find_z_line_lengths(screen_pos: vec2<i32>, horizontal: bool, inverted_z: bool, step_right: vec2<i32>) -> vec2<f32> {
    var mask_trace_left = select(0x04u, 0x08u, horizontal);
    var mask_trace_right = select(0x01u, 0x02u, horizontal);
    if inverted_z {
        let temp = mask_trace_left;
        mask_trace_left = mask_trace_right;
        mask_trace_right = temp;
    }

    var continue_left = true;
    var continue_right = true;
    var length_left = 1.0;
    var length_right = 1.0;
    loop {
        let edge_left = load_edge(screen_pos - step_right * i32(length_left));
        let edge_right = load_edge(screen_pos + step_right * (i32(length_right) + 1));

        continue_left = continue_left && ((edge_left & mask_trace_left) == mask_trace_left);
        continue_right = continue_right && ((edge_right & mask_trace_right) == mask_trace_right);

        length_left += select(0.0, 1.0, continue_left);
        length_right += select(0.0, 1.0, continue_right);

        var max_lr = max(length_right, length_left);

        if !continue_left && !continue_right {
            max_lr = MAX_LINE_LENGTH;
        }

        if max_lr >= min(MAX_LINE_LENGTH, 1.25 * min(length_right, length_left) - 0.25) {
            break;
        }
    }
    return vec2<f32>(length_left, length_right);
}

fn collect_blend_zs(screen_pos: vec2<i32>, horizontal: bool, inverted_z: bool, quality: f32, length_left: f32, length_right: f32) -> bool {
    let left_odd = SYMMETRY_CORRECTION_OFFSET * (length_left % 2.0);
    let right_odd = SYMMETRY_CORRECTION_OFFSET * (length_right % 2.0);

    let dampen = saturate((length_left + length_right - quality) * DAMPENING_EFFECT);

    let loop_from = -floor((length_left + 1.0) / 2.0) + 1.0;
    let loop_to = floor((length_right + 1.0) / 2.0);

    let count = u32(loop_to - loop_from + 1.0);
    var item_index = atomicAdd(&blend_count, count);
    if item_index + count > BLEND_SLOTS {
        for (var s = item_index; s < min(item_index + count, BLEND_SLOTS); s++) {
            blend_items[s] = vec2<u32>(NO_ITEM, NO_ITEM);
        }
        return false;
    }

    let total_length = (loop_to - loop_from) + 1.0 - left_odd - right_odd;
    let lerp_step = 1.0 / total_length;
    let lerp_from_k = (0.5 - left_odd - loop_from) * lerp_step;

    let item_header = (u32(screen_pos.x) << 18u) | u32(screen_pos.y);
    let item_static = (u32(horizontal) << 31u) | (u32(inverted_z) << 30u);

    for (var i = loop_from; i <= loop_to; i += 1.0) {
        let second_part = select(0.0, 1.0, i > 0.0);
        let src_offset = 1.0 - second_part * 2.0;

        var lerp_k = (lerp_step * i + lerp_from_k) * src_offset + second_part;
        lerp_k *= dampen;

        let value = item_static | (u32(i + 256.0) << 20u) | (u32(src_offset + 256.0) << 10u) | u32(saturate(lerp_k) * 1023.0 + 0.5);
        blend_items[item_index] = vec2<u32>(item_header, value);
        item_index += 1u;
    }
    return true;
}

fn blend_zs(screen_pos: vec2<i32>, horizontal: bool, inverted_z: bool, quality: f32, length_left: f32, length_right: f32, step_right: vec2<i32>) {
    var blend_dir = select(vec2<i32>(-1, 0), vec2<i32>(0, -1), horizontal);
    if inverted_z {
        blend_dir = -blend_dir;
    }

    let left_odd = SYMMETRY_CORRECTION_OFFSET * (length_left % 2.0);
    let right_odd = SYMMETRY_CORRECTION_OFFSET * (length_right % 2.0);

    let dampen = saturate((length_left + length_right - quality) * DAMPENING_EFFECT);

    let loop_from = -floor((length_left + 1.0) / 2.0) + 1.0;
    let loop_to = floor((length_right + 1.0) / 2.0);

    let total_length = (loop_to - loop_from) + 1.0 - left_odd - right_odd;
    let lerp_step = 1.0 / total_length;
    let lerp_from_k = (0.5 - left_odd - loop_from) * lerp_step;

    for (var i = loop_from; i <= loop_to; i += 1.0) {
        let second_part = select(0.0, 1.0, i > 0.0);
        let src_offset = 1.0 - second_part * 2.0;

        var lerp_k = (lerp_step * i + lerp_from_k) * src_offset + second_part;
        lerp_k *= dampen;

        let pixel_pos = screen_pos + step_right * i32(i);

        let color_center = load_color(pixel_pos).rgb;
        let color_from = load_color(pixel_pos + blend_dir * i32(src_offset)).rgb;

        store_color_sample(pixel_pos, mix(color_center, color_from, lerp_k), true);
    }
}

@compute @workgroup_size(128, 1, 1)
fn process_candidates(@builtin(global_invocation_id) dispatch_id: vec3<u32>, @builtin(local_invocation_id) local_id: vec3<u32>) {
    if local_id.x == 0u {
        atomicStore(&blend_count, 0u);
    }
    workgroupBarrier();

    let num_candidates = atomicLoad(&control[CONTROL_COUNT]);
    if dispatch_id.x < num_candidates {
        let pixel_id = candidates[dispatch_id.x];
        let pixel_pos = vec2<i32>(i32(pixel_id >> 18u), i32(pixel_id & 0x3FFFu));

        let edges = unpack_edges(load_edge(pixel_pos));
        let edges_left = unpack_edges(load_edge(pixel_pos + vec2<i32>(-1, 0)));
        let edges_right = unpack_edges(load_edge(pixel_pos + vec2<i32>(1, 0)));
        let edges_bottom = unpack_edges(load_edge(pixel_pos + vec2<i32>(0, 1)));
        let edges_top = unpack_edges(load_edge(pixel_pos + vec2<i32>(0, -1)));

        {
            let blend_val = simple_shape_blend_values(edges, edges_left, edges_right, edges_top, edges_bottom);
            let four_weight_sum = dot(blend_val, vec4<f32>(1.0));
            let center_weight = 1.0 - four_weight_sum;

            var out_color = load_color(pixel_pos).rgb * center_weight;
            if blend_val.x > 0.0 {
                out_color += blend_val.x * load_color(pixel_pos + vec2<i32>(-1, 0)).rgb;
            }
            if blend_val.y > 0.0 {
                out_color += blend_val.y * load_color(pixel_pos + vec2<i32>(0, -1)).rgb;
            }
            if blend_val.z > 0.0 {
                out_color += blend_val.z * load_color(pixel_pos + vec2<i32>(1, 0)).rgb;
            }
            if blend_val.w > 0.0 {
                out_color += blend_val.w * load_color(pixel_pos + vec2<i32>(0, 1)).rgb;
            }
            store_color_sample(pixel_pos, out_color, false);
        }

        {
            var horizontal = true;
            var inverted_z = false;

            let edges_p2p0 = unpack_edges(load_edge(pixel_pos + vec2<i32>(2, 0)));
            let scores_h = detect_zs_horizontal(edges, edges_left, edges_right, edges_p2p0);
            var max_score = max(scores_h.x, scores_h.y);
            if max_score > 0.0 {
                inverted_z = scores_h.x > scores_h.y;
            }

            let edges_p0m2 = unpack_edges(load_edge(pixel_pos + vec2<i32>(0, -2)));
            let scores_v = detect_zs_horizontal(edges.argb, edges_bottom.argb, edges_top.argb, edges_p0m2.argb);
            let vert_score = max(scores_v.x, scores_v.y);
            if vert_score > max_score {
                max_score = vert_score;
                horizontal = false;
                inverted_z = scores_v.x > scores_v.y;
            }

            if max_score > 0.0 {
                let quality = floor(clamp(4.0 - max_score, 0.0, 3.0));
                let step_right = select(vec2<i32>(0, -1), vec2<i32>(1, 0), horizontal);
                let lengths = find_z_line_lengths(pixel_pos, horizontal, inverted_z, step_right);
                let length_left = lengths.x - quality;
                let length_right = lengths.y - quality;

                if (length_left + length_right) >= 5.0 {
                    if !collect_blend_zs(pixel_pos, horizontal, inverted_z, quality, length_left, length_right) {
                        blend_zs(pixel_pos, horizontal, inverted_z, quality, length_left, length_right, step_right);
                    }
                }
            }
        }
    }

    workgroupBarrier();

    let total_item_count = min(BLEND_SLOTS, atomicLoad(&blend_count));
    let loops = (total_item_count + (PROCESS_THREADS - 1u) - local_id.x) / PROCESS_THREADS;

    for (var pass_index = 0u; pass_index < loops; pass_index++) {
        let index = pass_index * PROCESS_THREADS + local_id.x;
        let item = blend_items[index];
        if item.x == NO_ITEM {
            continue;
        }

        let starting_pos = vec2<i32>(i32(item.x >> 18u), i32(item.x & 0x3FFFu));
        let horizontal = ((item.y >> 31u) & 1u) != 0u;
        let inverted_z = ((item.y >> 30u) & 1u) != 0u;
        let step_index = i32((item.y >> 20u) & 0x3FFu) - 256;
        let src_offset = i32((item.y >> 10u) & 0x3FFu) - 256;
        let lerp_k = f32(item.y & 0x3FFu) / 1023.0;

        let step_right = select(vec2<i32>(0, -1), vec2<i32>(1, 0), horizontal);
        var blend_dir = select(vec2<i32>(-1, 0), vec2<i32>(0, -1), horizontal);
        if inverted_z {
            blend_dir = -blend_dir;
        }

        let item_pixel_pos = starting_pos + step_right * step_index;

        let color_center = load_color(item_pixel_pos).rgb;
        let color_from = load_color(item_pixel_pos + blend_dir * src_offset).rgb;

        store_color_sample(item_pixel_pos, mix(color_center, color_from, lerp_k), true);
    }
}

@compute @workgroup_size(4, 32, 1)
fn deferred_color_apply(@builtin(global_invocation_id) dispatch_id: vec3<u32>, @builtin(local_invocation_id) local_id: vec3<u32>) {
    let num_candidates = atomicLoad(&control[CONTROL_COUNT]);
    let current_candidate = dispatch_id.y;
    let current_quad_offset = local_id.x;

    if current_candidate >= num_candidates {
        return;
    }

    let pixel_id = locations[current_candidate];
    let quad_pos = vec2<u32>(pixel_id >> 16u, pixel_id & 0xFFFFu);
    let pixel_pos = vec2<i32>(quad_pos * 2u) + vec2<i32>(i32(current_quad_offset & 1u), i32(current_quad_offset >> 1u));
    if any(pixel_pos >= image_size()) {
        return;
    }

    var head = atomicLoad(&heads[quad_pos.y * quad_stride() + quad_pos.x]);
    var out_colors = vec4<f32>(0.0);

    for (var i = 0u; head != NO_ITEM && i < 32u; i++) {
        let offset_xy = (head >> 30u) & 0x03u;
        let complex_shape = (head >> 26u) & 0x01u;
        let index = head & ((1u << 26u) - 1u);
        if index >= arrayLength(&items) {
            break;
        }

        let val = items[index];
        head = val.x;

        if offset_xy == current_quad_offset {
            let color = unpack_color(val.y);
            let weight = 0.8 + 1.0 * f32(complex_shape);
            out_colors += vec4<f32>(color * weight, weight);
        }
    }

    if out_colors.a == 0.0 {
        return;
    }

    let alpha = textureLoad(src, pixel_pos, 0).a;
    textureStore(dst, pixel_pos, vec4<f32>(out_colors.rgb / out_colors.a, alpha));
}

@vertex
fn vs_blit(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(p * 2.0 - vec2<f32>(1.0), 0.0, 1.0);
}

@fragment
fn fs_blit(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(blit_src, vec2<i32>(position.xy), 0);
}
