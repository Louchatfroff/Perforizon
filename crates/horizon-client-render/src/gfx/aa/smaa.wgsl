const THRESHOLD: f32 = 0.1;
const MAX_SEARCH_STEPS: f32 = 16.0;
const MAX_SEARCH_STEPS_DIAG: f32 = 8.0;
const CORNER_ROUNDING_NORM: f32 = 0.25;
const LOCAL_CONTRAST_ADAPTATION_FACTOR: f32 = 2.0;
const AREATEX_MAX_DISTANCE: f32 = 16.0;
const AREATEX_MAX_DISTANCE_DIAG: f32 = 20.0;
const AREATEX_PIXEL_SIZE: vec2<f32> = vec2<f32>(1.0 / 160.0, 1.0 / 560.0);
const AREATEX_SUBTEX_SIZE: f32 = 1.0 / 7.0;
const SEARCHTEX_SIZE: vec2<f32> = vec2<f32>(66.0, 33.0);
const SEARCHTEX_PACKED_SIZE: vec2<f32> = vec2<f32>(64.0, 16.0);

@group(0) @binding(0) var color_tex: texture_2d<f32>;
@group(0) @binding(1) var edges_tex: texture_2d<f32>;
@group(0) @binding(2) var area_tex: texture_2d<f32>;
@group(0) @binding(3) var search_tex: texture_2d<f32>;
@group(0) @binding(4) var blend_tex: texture_2d<f32>;
@group(0) @binding(5) var linear_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
}

fn metrics(size: vec2<u32>) -> vec4<f32> {
    let s = vec2<f32>(size);
    return vec4<f32>(1.0 / s, s);
}

fn load_point(tex: texture_2d<f32>, uv: vec2<f32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(tex));
    let p = clamp(vec2<i32>(floor(uv * vec2<f32>(size))), vec2<i32>(0), size - vec2<i32>(1));
    return textureLoad(tex, p, 0);
}

fn luma(c: vec4<f32>) -> f32 {
    return dot(c.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@fragment
fn fs_edges(@builtin(position) frag: vec4<f32>) -> @location(0) vec2<f32> {
    let rt = metrics(textureDimensions(color_tex));
    let tc = frag.xy * rt.xy;
    let o0 = rt.xyxy * vec4<f32>(-1.0, 0.0, 0.0, -1.0) + tc.xyxy;
    let o1 = rt.xyxy * vec4<f32>(1.0, 0.0, 0.0, 1.0) + tc.xyxy;
    let o2 = rt.xyxy * vec4<f32>(-2.0, 0.0, 0.0, -2.0) + tc.xyxy;

    let l = luma(load_point(color_tex, tc));
    let l_left = luma(load_point(color_tex, o0.xy));
    let l_top = luma(load_point(color_tex, o0.zw));
    var delta = vec4<f32>(0.0);
    delta.x = abs(l - l_left);
    delta.y = abs(l - l_top);
    var edges = step(vec2<f32>(THRESHOLD), delta.xy);
    if dot(edges, vec2<f32>(1.0)) == 0.0 {
        return vec2<f32>(0.0);
    }

    let l_right = luma(load_point(color_tex, o1.xy));
    let l_bottom = luma(load_point(color_tex, o1.zw));
    delta.z = abs(l - l_right);
    delta.w = abs(l - l_bottom);
    var max_delta = max(delta.xy, delta.zw);

    let l_leftleft = luma(load_point(color_tex, o2.xy));
    let l_toptop = luma(load_point(color_tex, o2.zw));
    delta.z = abs(l_left - l_leftleft);
    delta.w = abs(l_top - l_toptop);
    max_delta = max(max_delta, delta.zw);
    let final_delta = max(max_delta.x, max_delta.y);

    edges *= step(vec2<f32>(final_delta), LOCAL_CONTRAST_ADAPTATION_FACTOR * delta.xy);
    return edges;
}

fn sample_edges(uv: vec2<f32>) -> vec2<f32> {
    return textureSampleLevel(edges_tex, linear_sampler, uv, 0.0).rg;
}

fn decode_diag2(e_in: vec2<f32>) -> vec2<f32> {
    var e = e_in;
    e.x = e.x * abs(5.0 * e.x - 5.0 * 0.75);
    return round(e);
}

fn decode_diag4(e_in: vec4<f32>) -> vec4<f32> {
    var e = e_in;
    let rb = e.xz * abs(5.0 * e.xz - vec2<f32>(5.0 * 0.75));
    e.x = rb.x;
    e.z = rb.y;
    return round(e);
}

fn search_diag1(texcoord: vec2<f32>, dir: vec2<f32>, rt: vec4<f32>) -> vec4<f32> {
    var coord = vec4<f32>(texcoord, -1.0, 1.0);
    let t = vec3<f32>(rt.xy, 1.0);
    var e = vec2<f32>(0.0);
    loop {
        if !(coord.z < MAX_SEARCH_STEPS_DIAG - 1.0 && coord.w > 0.9) {
            break;
        }
        let next = t * vec3<f32>(dir, 1.0) + coord.xyz;
        coord.x = next.x;
        coord.y = next.y;
        coord.z = next.z;
        e = sample_edges(coord.xy);
        coord.w = dot(e, vec2<f32>(0.5));
    }
    return vec4<f32>(coord.z, coord.w, e);
}

fn search_diag2(texcoord: vec2<f32>, dir: vec2<f32>, rt: vec4<f32>) -> vec4<f32> {
    var coord = vec4<f32>(texcoord, -1.0, 1.0);
    coord.x += 0.25 * rt.x;
    let t = vec3<f32>(rt.xy, 1.0);
    var e = vec2<f32>(0.0);
    loop {
        if !(coord.z < MAX_SEARCH_STEPS_DIAG - 1.0 && coord.w > 0.9) {
            break;
        }
        let next = t * vec3<f32>(dir, 1.0) + coord.xyz;
        coord.x = next.x;
        coord.y = next.y;
        coord.z = next.z;
        e = sample_edges(coord.xy);
        e = decode_diag2(e);
        coord.w = dot(e, vec2<f32>(0.5));
    }
    return vec4<f32>(coord.z, coord.w, e);
}

fn area_diag(dist: vec2<f32>, e: vec2<f32>, offset: f32) -> vec2<f32> {
    var texcoord = vec2<f32>(AREATEX_MAX_DISTANCE_DIAG) * e + dist;
    texcoord = AREATEX_PIXEL_SIZE * texcoord + 0.5 * AREATEX_PIXEL_SIZE;
    texcoord.x += 0.5;
    texcoord.y += AREATEX_SUBTEX_SIZE * offset;
    return textureSampleLevel(area_tex, linear_sampler, texcoord, 0.0).rg;
}

fn diag_weights(texcoord: vec2<f32>, e: vec2<f32>, rt: vec4<f32>) -> vec2<f32> {
    var weights = vec2<f32>(0.0);
    var d = vec4<f32>(0.0);

    if e.x > 0.0 {
        let s = search_diag1(texcoord, vec2<f32>(-1.0, 1.0), rt);
        d.x = s.x + select(0.0, 1.0, s.w > 0.9);
        d.z = s.y;
    } else {
        d.x = 0.0;
        d.z = 0.0;
    }
    let s1 = search_diag1(texcoord, vec2<f32>(1.0, -1.0), rt);
    d.y = s1.x;
    d.w = s1.y;

    if d.x + d.y > 2.0 {
        let coords = vec4<f32>(-d.x + 0.25, d.x, d.y, -d.y - 0.25) * rt.xyxy + texcoord.xyxy;
        var c = vec4<f32>(0.0);
        let c_lo = textureSampleLevel(edges_tex, linear_sampler, coords.xy, 0.0, vec2<i32>(-1, 0)).rg;
        let c_hi = textureSampleLevel(edges_tex, linear_sampler, coords.zw, 0.0, vec2<i32>(1, 0)).rg;
        c = vec4<f32>(c_lo, c_hi);
        let dec = decode_diag4(c);
        c = vec4<f32>(dec.y, dec.x, dec.w, dec.z);
        var cc = vec2<f32>(2.0) * c.xz + c.yw;
        cc = select(cc, vec2<f32>(0.0), d.zw >= vec2<f32>(0.9));
        weights += area_diag(d.xy, cc, 0.0);
    }

    let s2 = search_diag2(texcoord, vec2<f32>(-1.0, -1.0), rt);
    d.x = s2.x;
    d.z = s2.y;
    if textureSampleLevel(edges_tex, linear_sampler, texcoord, 0.0, vec2<i32>(1, 0)).r > 0.0 {
        let s3 = search_diag2(texcoord, vec2<f32>(1.0, 1.0), rt);
        d.y = s3.x + select(0.0, 1.0, s3.w > 0.9);
        d.w = s3.y;
    } else {
        d.y = 0.0;
        d.w = 0.0;
    }

    if d.x + d.y > 2.0 {
        let coords = vec4<f32>(-d.x, -d.x, d.y, d.y) * rt.xyxy + texcoord.xyxy;
        var c = vec4<f32>(0.0);
        c.x = textureSampleLevel(edges_tex, linear_sampler, coords.xy, 0.0, vec2<i32>(-1, 0)).g;
        c.y = textureSampleLevel(edges_tex, linear_sampler, coords.xy, 0.0, vec2<i32>(0, -1)).r;
        let c_hi = textureSampleLevel(edges_tex, linear_sampler, coords.zw, 0.0, vec2<i32>(1, 0)).gr;
        c.z = c_hi.x;
        c.w = c_hi.y;
        var cc = vec2<f32>(2.0) * c.xz + c.yw;
        cc = select(cc, vec2<f32>(0.0), d.zw >= vec2<f32>(0.9));
        weights += area_diag(d.xy, cc, 0.0).gr;
    }
    return weights;
}

fn search_length(e: vec2<f32>, offset: f32) -> f32 {
    var scale = SEARCHTEX_SIZE * vec2<f32>(0.5, -1.0);
    var bias = SEARCHTEX_SIZE * vec2<f32>(offset, 1.0);
    scale += vec2<f32>(-1.0, 1.0);
    bias += vec2<f32>(0.5, -0.5);
    scale *= 1.0 / SEARCHTEX_PACKED_SIZE;
    bias *= 1.0 / SEARCHTEX_PACKED_SIZE;
    return textureSampleLevel(search_tex, linear_sampler, scale * e + bias, 0.0).r;
}

fn search_x_left(start: vec2<f32>, end: f32, rt: vec4<f32>) -> f32 {
    var texcoord = start;
    var e = vec2<f32>(0.0, 1.0);
    loop {
        if !(texcoord.x > end && e.y > 0.8281 && e.x == 0.0) {
            break;
        }
        e = sample_edges(texcoord);
        texcoord = -vec2<f32>(2.0, 0.0) * rt.xy + texcoord;
    }
    let offset = -(255.0 / 127.0) * search_length(e, 0.0) + 3.25;
    return rt.x * offset + texcoord.x;
}

fn search_x_right(start: vec2<f32>, end: f32, rt: vec4<f32>) -> f32 {
    var texcoord = start;
    var e = vec2<f32>(0.0, 1.0);
    loop {
        if !(texcoord.x < end && e.y > 0.8281 && e.x == 0.0) {
            break;
        }
        e = sample_edges(texcoord);
        texcoord = vec2<f32>(2.0, 0.0) * rt.xy + texcoord;
    }
    let offset = -(255.0 / 127.0) * search_length(e, 0.5) + 3.25;
    return -rt.x * offset + texcoord.x;
}

fn search_y_up(start: vec2<f32>, end: f32, rt: vec4<f32>) -> f32 {
    var texcoord = start;
    var e = vec2<f32>(1.0, 0.0);
    loop {
        if !(texcoord.y > end && e.x > 0.8281 && e.y == 0.0) {
            break;
        }
        e = sample_edges(texcoord);
        texcoord = -vec2<f32>(0.0, 2.0) * rt.xy + texcoord;
    }
    let offset = -(255.0 / 127.0) * search_length(e.yx, 0.0) + 3.25;
    return rt.y * offset + texcoord.y;
}

fn search_y_down(start: vec2<f32>, end: f32, rt: vec4<f32>) -> f32 {
    var texcoord = start;
    var e = vec2<f32>(1.0, 0.0);
    loop {
        if !(texcoord.y < end && e.x > 0.8281 && e.y == 0.0) {
            break;
        }
        e = sample_edges(texcoord);
        texcoord = vec2<f32>(0.0, 2.0) * rt.xy + texcoord;
    }
    let offset = -(255.0 / 127.0) * search_length(e.yx, 0.5) + 3.25;
    return -rt.y * offset + texcoord.y;
}

fn area(dist: vec2<f32>, e1: f32, e2: f32, offset: f32) -> vec2<f32> {
    var texcoord = vec2<f32>(AREATEX_MAX_DISTANCE) * round(4.0 * vec2<f32>(e1, e2)) + dist;
    texcoord = AREATEX_PIXEL_SIZE * texcoord + 0.5 * AREATEX_PIXEL_SIZE;
    texcoord.y = AREATEX_SUBTEX_SIZE * offset + texcoord.y;
    return textureSampleLevel(area_tex, linear_sampler, texcoord, 0.0).rg;
}

fn corner_horizontal(weights_in: vec2<f32>, texcoord: vec4<f32>, d: vec2<f32>) -> vec2<f32> {
    let left_right = step(d.xy, d.yx);
    var rounding = (1.0 - CORNER_ROUNDING_NORM) * left_right;
    rounding /= left_right.x + left_right.y;
    var factor = vec2<f32>(1.0);
    factor.x -= rounding.x * textureSampleLevel(edges_tex, linear_sampler, texcoord.xy, 0.0, vec2<i32>(0, 1)).r;
    factor.x -= rounding.y * textureSampleLevel(edges_tex, linear_sampler, texcoord.zw, 0.0, vec2<i32>(1, 1)).r;
    factor.y -= rounding.x * textureSampleLevel(edges_tex, linear_sampler, texcoord.xy, 0.0, vec2<i32>(0, -2)).r;
    factor.y -= rounding.y * textureSampleLevel(edges_tex, linear_sampler, texcoord.zw, 0.0, vec2<i32>(1, -2)).r;
    return weights_in * saturate(factor);
}

fn corner_vertical(weights_in: vec2<f32>, texcoord: vec4<f32>, d: vec2<f32>) -> vec2<f32> {
    let left_right = step(d.xy, d.yx);
    var rounding = (1.0 - CORNER_ROUNDING_NORM) * left_right;
    rounding /= left_right.x + left_right.y;
    var factor = vec2<f32>(1.0);
    factor.x -= rounding.x * textureSampleLevel(edges_tex, linear_sampler, texcoord.xy, 0.0, vec2<i32>(1, 0)).g;
    factor.x -= rounding.y * textureSampleLevel(edges_tex, linear_sampler, texcoord.zw, 0.0, vec2<i32>(1, 1)).g;
    factor.y -= rounding.x * textureSampleLevel(edges_tex, linear_sampler, texcoord.xy, 0.0, vec2<i32>(-2, 0)).g;
    factor.y -= rounding.y * textureSampleLevel(edges_tex, linear_sampler, texcoord.zw, 0.0, vec2<i32>(-2, 1)).g;
    return weights_in * saturate(factor);
}

@fragment
fn fs_weights(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let rt = metrics(textureDimensions(edges_tex));
    let tc = frag.xy * rt.xy;
    let pixcoord = frag.xy;
    let o0 = rt.xyxy * vec4<f32>(-0.25, -0.125, 1.25, -0.125) + tc.xyxy;
    let o1 = rt.xyxy * vec4<f32>(-0.125, -0.25, -0.125, 1.25) + tc.xyxy;
    let o2 = rt.xxyy * (vec4<f32>(-2.0, 2.0, -2.0, 2.0) * MAX_SEARCH_STEPS)
        + vec4<f32>(o0.x, o0.z, o1.y, o1.w);

    var weights = vec4<f32>(0.0);
    var e = sample_edges(tc);

    if e.y > 0.0 {
        let diag = diag_weights(tc, e, rt);
        weights.r = diag.x;
        weights.g = diag.y;
        if weights.r == -weights.g {
            var coords = vec3<f32>(0.0);
            coords.x = search_x_left(o0.xy, o2.x, rt);
            coords.y = o1.y;
            var d = vec2<f32>(0.0);
            d.x = coords.x;
            let e1 = sample_edges(coords.xy).r;
            coords.z = search_x_right(o0.zw, o2.y, rt);
            d.y = coords.z;
            d = abs(round(rt.zz * d - vec2<f32>(pixcoord.x)));
            let sqrt_d = sqrt(d);
            let e2 = textureSampleLevel(edges_tex, linear_sampler, coords.zy, 0.0, vec2<i32>(1, 0)).r;
            let w = area(sqrt_d, e1, e2, 0.0);
            coords.y = tc.y;
            let corner = corner_horizontal(w, vec4<f32>(coords.x, coords.y, coords.z, coords.y), d);
            weights.r = corner.x;
            weights.g = corner.y;
        } else {
            e.x = 0.0;
        }
    }

    if e.x > 0.0 {
        var coords = vec3<f32>(0.0);
        coords.y = search_y_up(o1.xy, o2.z, rt);
        coords.x = o0.x;
        var d = vec2<f32>(0.0);
        d.x = coords.y;
        let e1 = sample_edges(coords.xy).g;
        coords.z = search_y_down(o1.zw, o2.w, rt);
        d.y = coords.z;
        d = abs(round(rt.ww * d - vec2<f32>(pixcoord.y)));
        let sqrt_d = sqrt(d);
        let e2 = textureSampleLevel(edges_tex, linear_sampler, coords.xz, 0.0, vec2<i32>(0, 1)).g;
        let w = area(sqrt_d, e1, e2, 0.0);
        coords.x = tc.x;
        let corner = corner_vertical(w, vec4<f32>(coords.x, coords.y, coords.x, coords.z), d);
        weights.b = corner.x;
        weights.a = corner.y;
    }
    return weights;
}

@fragment
fn fs_blend(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let rt = metrics(textureDimensions(color_tex));
    let tc = frag.xy * rt.xy;
    let offset = rt.xyxy * vec4<f32>(1.0, 0.0, 0.0, 1.0) + tc.xyxy;
    let centre = load_point(color_tex, tc);

    var a = vec4<f32>(0.0);
    a.x = textureSampleLevel(blend_tex, linear_sampler, offset.xy, 0.0).a;
    a.y = textureSampleLevel(blend_tex, linear_sampler, offset.zw, 0.0).g;
    let own = textureSampleLevel(blend_tex, linear_sampler, tc, 0.0);
    a.w = own.x;
    a.z = own.z;

    if dot(a, vec4<f32>(1.0)) < 1e-5 {
        return centre;
    }
    let h = max(a.x, a.z) > max(a.y, a.w);
    var blending_offset = vec4<f32>(0.0, a.y, 0.0, a.w);
    var blending_weight = a.yw;
    if h {
        blending_offset = vec4<f32>(a.x, 0.0, a.z, 0.0);
        blending_weight = a.xz;
    }
    blending_weight /= dot(blending_weight, vec2<f32>(1.0));
    let blending_coord = blending_offset * vec4<f32>(rt.xy, -rt.xy) + tc.xyxy;
    var color = blending_weight.x * textureSampleLevel(color_tex, linear_sampler, blending_coord.xy, 0.0);
    color += blending_weight.y * textureSampleLevel(color_tex, linear_sampler, blending_coord.zw, 0.0);
    return vec4<f32>(color.rgb, centre.a);
}
