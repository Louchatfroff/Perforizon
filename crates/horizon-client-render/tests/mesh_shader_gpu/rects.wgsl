enable wgpu_mesh_shader;

struct View {
    visible: vec4<f32>,
    target_size: vec4<f32>,
}

struct Payload {
    meshlet: u32,
}

struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

struct Primitive {
    @builtin(triangle_indices) indices: vec3<u32>,
}

struct MeshOutput {
    @builtin(vertices) vertices: array<Vertex, 4>,
    @builtin(primitives) primitives: array<Primitive, 2>,
    @builtin(vertex_count) vertex_count: u32,
    @builtin(primitive_count) primitive_count: u32,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<storage, read> rects: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> colors: array<vec4<f32>>;

var<task_payload> payload: Payload;
var<workgroup> mesh_output: MeshOutput;

fn outside(rect: vec4<f32>) -> bool {
    return rect.z <= view.visible.x || rect.x >= view.visible.z
        || rect.w <= view.visible.y || rect.y >= view.visible.w;
}

@task
@payload(payload)
@workgroup_size(32)
fn ts_main(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) -> @builtin(mesh_task_size) vec3<u32> {
    let id = group.x;
    if outside(rects[id]) {
        return vec3(0u, 0u, 0u);
    }
    if lane == 0u {
        payload.meshlet = id;
    }
    return vec3(1u, 1u, 1u);
}

@mesh(mesh_output)
@payload(payload)
@workgroup_size(32)
fn ms_main(@builtin(local_invocation_index) lane: u32) {
    if lane == 0u {
        mesh_output.vertex_count = 4u;
        mesh_output.primitive_count = 2u;
        mesh_output.primitives[0].indices = vec3(0u, 1u, 2u);
        mesh_output.primitives[1].indices = vec3(0u, 2u, 3u);
    }
    if lane < 4u {
        let rect = rects[payload.meshlet];
        let x = select(rect.x, rect.z, lane == 1u || lane == 2u);
        let y = select(rect.y, rect.w, lane >= 2u);
        let ndc = vec2(
            x / view.target_size.x * 2.0 - 1.0,
            1.0 - y / view.target_size.y * 2.0,
        );
        mesh_output.vertices[lane].position = vec4(ndc, 0.0, 1.0);
        mesh_output.vertices[lane].color = colors[payload.meshlet];
    }
}

@fragment
fn fs_main(in: Vertex) -> @location(0) vec4<f32> {
    return in.color;
}
