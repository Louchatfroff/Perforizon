struct Payload {
    origin: vec4<f32>,
    base_vertex: u32,
    quads: u32,
}

struct Run {
    first: u32,
}

struct MeshletPrimitive {
    @builtin(triangle_indices) indices: vec3<u32>,
}

struct Meshlet {
    @builtin(vertices) vertices: array<TerrainOut, MESHLET_VERTICES>,
    @builtin(primitives) primitives: array<MeshletPrimitive, MESHLET_PRIMITIVES>,
    @builtin(vertex_count) vertex_count: u32,
    @builtin(primitive_count) primitive_count: u32,
}

var<immediate> run: Run;
@group(1) @binding(2) var<storage, read> draws: array<u32>;
@group(1) @binding(3) var<storage, read> origins: array<vec4<f32>>;
@group(2) @binding(0) var<storage, read> page: array<u32>;
var<task_payload> payload: Payload;
var<workgroup> meshlet: Meshlet;

const SECTION_CENTER: vec3<f32> = vec3<f32>(8.0);
const SECTION_REACH: vec3<f32> = vec3<f32>(9.5);

fn beyond(plane: vec4<f32>, center: vec3<f32>) -> bool {
    return dot(plane.xyz, center) + plane.w + dot(abs(plane.xyz), SECTION_REACH) < 0.0;
}

fn in_view(origin: vec3<f32>) -> bool {
    let m = globals.view_proj;
    let x = vec4(m[0][0], m[1][0], m[2][0], m[3][0]);
    let y = vec4(m[0][1], m[1][1], m[2][1], m[3][1]);
    let w = vec4(m[0][3], m[1][3], m[2][3], m[3][3]);
    let center = origin + SECTION_CENTER;
    return !(beyond(w + x, center) || beyond(w - x, center)
        || beyond(w + y, center) || beyond(w - y, center));
}

@task
@payload(payload)
@workgroup_size(TASK_THREADS)
fn ts_terrain(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) -> @builtin(mesh_task_size) vec3<u32> {
    let at = (run.first + group.x) * 5u;
    let quads = draws[at] / 6u;
    if draws[at + 1u] == 0u || quads == 0u {
        return vec3(0u, 0u, 0u);
    }
    let origin = origins[draws[at + 4u]];
    if !in_view(origin.xyz) {
        return vec3(0u, 0u, 0u);
    }
    if lane == 0u {
        payload.origin = origin;
        payload.base_vertex = draws[at + 3u];
        payload.quads = quads;
    }
    return vec3((quads + MESHLET_QUADS - 1u) / MESHLET_QUADS, 1u, 1u);
}

fn terrain_vertex_at(vertex: u32, origin: vec4<f32>) -> TerrainOut {
    let at = vertex * 5u;
    let xy = page[at];
    let zl = page[at + 1u];
    let packed_uv = page[at + 2u];
    let light_mode = page[at + 3u];
    let raw = vec4(xy & 0xffffu, xy >> 16u, zl & 0xffffu, zl >> 16u);
    let position = terrain_position(raw, light_mode >> 16u);
    let uv = terrain_uv(vec2(packed_uv & 0xffffu, packed_uv >> 16u), origin.w, light_mode >> 16u);
    let light = vec2(light_mode & 0xffu, (light_mode >> 8u) & 0xffu);
    return terrain_vertex(position, uv, raw.w, unpack4x8unorm(page[at + 4u]), light, origin.xyz, origin.w);
}

@mesh(meshlet)
@payload(payload)
@workgroup_size(MESH_THREADS)
fn ms_terrain(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) {
    let first = group.x * MESHLET_QUADS;
    let quads = min(payload.quads - first, MESHLET_QUADS);
    let vertices = quads * 4u;
    let primitives = quads * 2u;
    if lane == 0u {
        meshlet.vertex_count = vertices;
        meshlet.primitive_count = primitives;
    }
    let base = payload.base_vertex + first * 4u;
    for (var v = lane; v < vertices; v += MESH_THREADS) {
        meshlet.vertices[v] = terrain_vertex_at(base + v, payload.origin);
    }
    for (var p = lane; p < primitives; p += MESH_THREADS) {
        let corner = (p >> 1u) * 4u;
        let second = p & 1u;
        meshlet.primitives[p].indices = vec3(corner, corner + 1u + second, corner + 2u + second);
    }
}
