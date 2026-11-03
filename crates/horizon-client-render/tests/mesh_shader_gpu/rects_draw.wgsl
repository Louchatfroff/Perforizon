enable wgpu_mesh_shader;
enable draw_index;

struct View {
    visible: vec4<f32>,
    target_size: vec4<f32>,
}

struct Payload {
    meshlet: u32,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<storage, read> rects: array<vec4<f32>>;

var<task_payload> payload: Payload;

fn outside(rect: vec4<f32>) -> bool {
    return rect.z <= view.visible.x || rect.x >= view.visible.z
        || rect.w <= view.visible.y || rect.y >= view.visible.w;
}

@task
@payload(payload)
@workgroup_size(32)
fn ts_draw(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
    @builtin(draw_index) draw: u32,
) -> @builtin(mesh_task_size) vec3<u32> {
    let id = draw * 2u + group.x;
    if outside(rects[id]) {
        return vec3(0u, 0u, 0u);
    }
    if lane == 0u {
        payload.meshlet = id;
    }
    return vec3(1u, 1u, 1u);
}
