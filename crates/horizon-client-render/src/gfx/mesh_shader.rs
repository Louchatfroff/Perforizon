#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    pub quads: u32,
    pub mesh_threads: u32,
    pub task_threads: u32,
}

impl Shape {
    pub fn vertices(self) -> u32 {
        self.quads * 4
    }

    pub fn primitives(self) -> u32 {
        self.quads * 2
    }

    pub fn workgroup_bytes(self) -> u32 {
        self.vertices() * VERTEX_BYTES + self.primitives() * PRIMITIVE_BYTES + 8
    }
}

pub const SHAPE: Shape = Shape {
    quads: 32,
    mesh_threads: 64,
    task_threads: 32,
};

pub const VERTEX_BYTES: u32 = 96;
pub const PRIMITIVE_BYTES: u32 = 16;
pub const PAYLOAD_BYTES: u32 = 32;
pub const IMMEDIATE_BYTES: u32 = 4;
pub const BIND_GROUPS: u32 = 3;
pub const MIN_MESHLETS: u32 = 8192;
pub const MIN_DRAWS: u32 = 4096;

pub fn shape() -> Shape {
    let from_env = std::env::var("HORIZON_MESH_SHAPE").ok().and_then(|v| {
        let n: Vec<u32> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        (n.len() == 3).then(|| Shape {
            quads: n[0],
            mesh_threads: n[1],
            task_threads: n[2],
        })
    });
    from_env.unwrap_or(SHAPE)
}

pub fn requested() -> Option<bool> {
    match std::env::var("HORIZON_MESH_SHADERS").as_deref() {
        Ok("0") => Some(false),
        Ok("1") => Some(true),
        _ => None,
    }
}

pub fn shortfalls(limits: &wgpu::Limits, shape: Shape, page_bytes: u64) -> Vec<String> {
    let needs = [
        (
            "task invocations per workgroup",
            limits.max_task_invocations_per_workgroup,
            shape.task_threads,
        ),
        (
            "task invocations per dimension",
            limits.max_task_invocations_per_dimension,
            shape.task_threads,
        ),
        (
            "mesh invocations per workgroup",
            limits.max_mesh_invocations_per_workgroup,
            shape.mesh_threads,
        ),
        (
            "mesh invocations per dimension",
            limits.max_mesh_invocations_per_dimension,
            shape.mesh_threads,
        ),
        (
            "mesh output vertices",
            limits.max_mesh_output_vertices,
            shape.vertices(),
        ),
        (
            "mesh output primitives",
            limits.max_mesh_output_primitives,
            shape.primitives(),
        ),
        (
            "task payload bytes",
            limits.max_task_payload_size,
            PAYLOAD_BYTES,
        ),
        (
            "task workgroups per dimension",
            limits.max_task_workgroups_per_dimension,
            MIN_DRAWS,
        ),
        (
            "task workgroups in total",
            limits.max_task_workgroup_total_count,
            MIN_DRAWS,
        ),
        (
            "mesh workgroups per dimension",
            limits.max_mesh_workgroups_per_dimension,
            MIN_MESHLETS,
        ),
        (
            "mesh workgroups in total",
            limits.max_mesh_workgroup_total_count,
            MIN_MESHLETS,
        ),
        (
            "workgroup memory bytes",
            limits.max_compute_workgroup_storage_size,
            shape.workgroup_bytes(),
        ),
        (
            "immediate bytes",
            limits.max_immediate_size,
            IMMEDIATE_BYTES,
        ),
        ("bind groups", limits.max_bind_groups, BIND_GROUPS),
        (
            "storage buffers per stage",
            limits.max_storage_buffers_per_shader_stage,
            2,
        ),
    ];
    let mut missing: Vec<String> = needs
        .iter()
        .filter(|(_, have, want)| have < want)
        .map(|(name, have, want)| format!("{name} {have} (needs {want})"))
        .collect();
    if limits.max_storage_buffer_binding_size < page_bytes {
        missing.push(format!(
            "storage binding bytes {} (needs {page_bytes})",
            limits.max_storage_buffer_binding_size
        ));
    }
    missing
}

pub fn unsupported(adapter: &wgpu::Adapter, shape: Shape, page_bytes: u64) -> Option<String> {
    let info = adapter.get_info();
    if info.backend != wgpu::Backend::Vulkan {
        return Some(format!(
            "wgpu has no mesh shaders on the {:?} backend",
            info.backend
        ));
    }
    if info.device_type == wgpu::DeviceType::Cpu {
        return Some("a software adapter".into());
    }
    let features = adapter.features();
    for (feature, what) in [
        (
            wgpu::Features::EXPERIMENTAL_MESH_SHADER,
            "the driver offers no mesh shaders",
        ),
        (
            wgpu::Features::IMMEDIATES,
            "the driver offers no push constants",
        ),
        (
            wgpu::Features::INDIRECT_FIRST_INSTANCE,
            "the driver offers no indirect first instance",
        ),
    ] {
        if !features.contains(feature) {
            return Some(what.into());
        }
    }
    let short = shortfalls(&adapter.limits(), shape, page_bytes);
    (!short.is_empty()).then(|| format!("the GPU's limits are too low: {}", short.join(", ")))
}

pub fn features(adapter: &wgpu::Adapter, page_bytes: u64) -> wgpu::Features {
    if requested() == Some(false) || unsupported(adapter, shape(), page_bytes).is_some() {
        return wgpu::Features::empty();
    }
    wgpu::Features::EXPERIMENTAL_MESH_SHADER | wgpu::Features::IMMEDIATES
}

pub fn status(adapter: &wgpu::Adapter, page_bytes: u64) -> Result<(), String> {
    if requested() == Some(false) {
        return Err("HORIZON_MESH_SHADERS=0".into());
    }
    match unsupported(adapter, shape(), page_bytes) {
        Some(reason) => Err(reason),
        None => Ok(()),
    }
}
