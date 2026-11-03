struct Params {
    scale: vec2<f32>,
    limit: vec2<i32>,
    sharpness: f32,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var<uniform> params: Params;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

fn texel(p: vec2<i32>) -> vec4<f32> {
    return textureLoad(source, clamp(p, vec2<i32>(0, 0), params.limit), 0);
}

fn rcp_lo(a: f32) -> f32 {
    return bitcast<f32>(0x7ef07ebbu - bitcast<u32>(a));
}

fn rcp_med(a: f32) -> f32 {
    let b = bitcast<f32>(0x7ef19fffu - bitcast<u32>(a));
    return b * (-b * a + 2.0);
}

fn rsq_lo(a: f32) -> f32 {
    return bitcast<f32>(0x5f347d74u - (bitcast<u32>(a) >> 1u));
}

fn luma2(c: vec3<f32>) -> f32 {
    return c.b * 0.5 + (c.r * 0.5 + c.g);
}

fn easu_set(w: f32, l_a: f32, l_b: f32, l_c: f32, l_d: f32, l_e: f32) -> vec3<f32> {
    let dc = l_d - l_c;
    let cb = l_c - l_b;
    var len_x = rcp_lo(max(abs(dc), abs(cb)));
    let dir_x = l_d - l_b;
    len_x = clamp(abs(dir_x) * len_x, 0.0, 1.0);
    len_x = len_x * len_x;
    let ec = l_e - l_c;
    let ca = l_c - l_a;
    var len_y = rcp_lo(max(abs(ec), abs(ca)));
    let dir_y = l_e - l_a;
    len_y = clamp(abs(dir_y) * len_y, 0.0, 1.0);
    len_y = len_y * len_y;
    return vec3<f32>(dir_x * w, dir_y * w, (len_x + len_y) * w);
}

fn easu_tap(off: vec2<f32>, dir: vec2<f32>, len: vec2<f32>, lob: f32, clp: f32, c: vec3<f32>) -> vec4<f32> {
    var v = vec2<f32>(off.x * dir.x + off.y * dir.y, off.x * (-dir.y) + off.y * dir.x);
    v = v * len;
    let d2 = min(v.x * v.x + v.y * v.y, clp);
    var w_b = 0.4 * d2 - 1.0;
    var w_a = lob * d2 - 1.0;
    w_b = w_b * w_b;
    w_a = w_a * w_a;
    w_b = 1.5625 * w_b - 0.5625;
    let w = w_b * w_a;
    return vec4<f32>(c * w, w);
}

@fragment
fn fs_easu(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let ip = floor(frag.xy);
    var pp = ip * params.scale + (0.5 * params.scale - 0.5);
    let fp = floor(pp);
    pp = pp - fp;
    let base = vec2<i32>(fp);

    let b = texel(base + vec2<i32>(0, -1)).rgb;
    let c = texel(base + vec2<i32>(1, -1)).rgb;
    let e = texel(base + vec2<i32>(-1, 0)).rgb;
    let f = texel(base).rgb;
    let g = texel(base + vec2<i32>(1, 0)).rgb;
    let h = texel(base + vec2<i32>(2, 0)).rgb;
    let i = texel(base + vec2<i32>(-1, 1)).rgb;
    let j = texel(base + vec2<i32>(0, 1)).rgb;
    let k = texel(base + vec2<i32>(1, 1)).rgb;
    let l = texel(base + vec2<i32>(2, 1)).rgb;
    let n = texel(base + vec2<i32>(0, 2)).rgb;
    let o = texel(base + vec2<i32>(1, 2)).rgb;

    let b_l = luma2(b);
    let c_l = luma2(c);
    let e_l = luma2(e);
    let f_l = luma2(f);
    let g_l = luma2(g);
    let h_l = luma2(h);
    let i_l = luma2(i);
    let j_l = luma2(j);
    let k_l = luma2(k);
    let l_l = luma2(l);
    let n_l = luma2(n);
    let o_l = luma2(o);

    let acc = easu_set((1.0 - pp.x) * (1.0 - pp.y), b_l, e_l, f_l, g_l, j_l)
        + easu_set(pp.x * (1.0 - pp.y), c_l, f_l, g_l, h_l, k_l)
        + easu_set((1.0 - pp.x) * pp.y, f_l, i_l, j_l, k_l, n_l)
        + easu_set(pp.x * pp.y, g_l, j_l, k_l, l_l, o_l);
    var dir = acc.xy;
    var len = acc.z;

    let dir2 = dir * dir;
    var dir_r = dir2.x + dir2.y;
    let zro = dir_r < (1.0 / 32768.0);
    dir_r = rsq_lo(dir_r);
    dir_r = select(dir_r, 1.0, zro);
    dir.x = select(dir.x, 1.0, zro);
    dir = dir * dir_r;

    len = len * 0.5;
    len = len * len;
    let stretch = (dir.x * dir.x + dir.y * dir.y) * rcp_lo(max(abs(dir.x), abs(dir.y)));
    let len2 = vec2<f32>(1.0 + (stretch - 1.0) * len, 1.0 - 0.5 * len);
    let lob = 0.5 + ((0.25 - 0.04) - 0.5) * len;
    let clp = rcp_lo(lob);

    let min4 = min(min(min(f, g), j), k);
    let max4 = max(max(max(f, g), j), k);

    var tap = easu_tap(vec2<f32>(0.0, -1.0) - pp, dir, len2, lob, clp, b);
    tap += easu_tap(vec2<f32>(1.0, -1.0) - pp, dir, len2, lob, clp, c);
    tap += easu_tap(vec2<f32>(-1.0, 1.0) - pp, dir, len2, lob, clp, i);
    tap += easu_tap(vec2<f32>(0.0, 1.0) - pp, dir, len2, lob, clp, j);
    tap += easu_tap(vec2<f32>(0.0, 0.0) - pp, dir, len2, lob, clp, f);
    tap += easu_tap(vec2<f32>(-1.0, 0.0) - pp, dir, len2, lob, clp, e);
    tap += easu_tap(vec2<f32>(1.0, 1.0) - pp, dir, len2, lob, clp, k);
    tap += easu_tap(vec2<f32>(2.0, 1.0) - pp, dir, len2, lob, clp, l);
    tap += easu_tap(vec2<f32>(2.0, 0.0) - pp, dir, len2, lob, clp, h);
    tap += easu_tap(vec2<f32>(1.0, 0.0) - pp, dir, len2, lob, clp, g);
    tap += easu_tap(vec2<f32>(1.0, 2.0) - pp, dir, len2, lob, clp, o);
    tap += easu_tap(vec2<f32>(0.0, 2.0) - pp, dir, len2, lob, clp, n);

    let rgb = min(max4, max(min4, tap.rgb * (1.0 / tap.w)));
    let nearest = texel(vec2<i32>(floor((ip + 0.5) * params.scale)));
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), nearest.a);
}

@fragment
fn fs_rcas(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = vec2<i32>(frag.xy);
    let b = texel(sp + vec2<i32>(0, -1)).rgb;
    let d = texel(sp + vec2<i32>(-1, 0)).rgb;
    let centre = texel(sp);
    let e = centre.rgb;
    let f = texel(sp + vec2<i32>(1, 0)).rgb;
    let h = texel(sp + vec2<i32>(0, 1)).rgb;

    let mn4 = min(min(min(b, d), f), h);
    let mx4 = max(max(max(b, d), f), h);

    let tiny = 1e-10;
    let hit_min = min(mn4, e) * (1.0 / max(4.0 * mx4, vec3<f32>(tiny)));
    let hit_max = (1.0 - max(mx4, e)) * (1.0 / min(4.0 * mn4 - 4.0, vec3<f32>(-tiny)));
    let lobe_rgb = max(-hit_min, hit_max);
    let lobe = max(-0.1875, min(max(lobe_rgb.r, max(lobe_rgb.g, lobe_rgb.b)), 0.0)) * exp2(-params.sharpness);

    let rcp_l = rcp_med(4.0 * lobe + 1.0);
    let rgb = (lobe * b + lobe * d + lobe * h + lobe * f + e) * rcp_l;
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), centre.a);
}
