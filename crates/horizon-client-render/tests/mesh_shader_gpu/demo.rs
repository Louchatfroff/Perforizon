use std::num::NonZeroU64;

pub const SOURCE: &str = include_str!("rects.wgsl");
pub const DRAW_SOURCE: &str = include_str!("rects_draw.wgsl");

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct View {
    pub visible: [f32; 4],
    pub target_size: [f32; 4],
}

pub struct Pipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub layout: wgpu::BindGroupLayout,
}

fn storage(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::TASK | wgpu::ShaderStages::MESH,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(16),
        },
        count: None,
    }
}

pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Mesh shader scene"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::TASK | wgpu::ShaderStages::MESH,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(size_of::<View>() as u64),
                },
                count: None,
            },
            storage(1),
            storage(2),
        ],
    })
}

pub fn create_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    draw_index: bool,
) -> Pipeline {
    let layout = bind_group_layout(device);
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Mesh shader"),
        bind_group_layouts: &[Some(&layout)],
        ..Default::default()
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Mesh shader"),
        source: wgpu::ShaderSource::Wgsl(SOURCE.into()),
    });
    let draw_shader = draw_index.then(|| {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Mesh shader draw index"),
            source: wgpu::ShaderSource::Wgsl(DRAW_SOURCE.into()),
        })
    });
    let pipeline = device.create_mesh_pipeline(&wgpu::MeshPipelineDescriptor {
        label: Some("Mesh shader"),
        layout: Some(&pipeline_layout),
        task: Some(wgpu::TaskState {
            module: draw_shader.as_ref().unwrap_or(&shader),
            entry_point: Some(if draw_index { "ts_draw" } else { "ts_main" }),
            compilation_options: Default::default(),
        }),
        mesh: wgpu::MeshState {
            module: &shader,
            entry_point: Some("ms_main"),
            compilation_options: Default::default(),
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(format.into())],
        }),
        multiview: None,
        cache: None,
    });
    Pipeline { pipeline, layout }
}
