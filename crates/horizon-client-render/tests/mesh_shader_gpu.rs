#[path = "mesh_shader_gpu/demo.rs"]
mod demo;
#[path = "../src/gfx/mesh_shader.rs"]
mod mesh_shader;

use wgpu::util::DeviceExt;

const SIZE: u32 = 64;
const RECTS: [[f32; 4]; 4] = [
    [8.0, 8.0, 24.0, 24.0],
    [40.0, 8.0, 56.0, 24.0],
    [8.0, 40.0, 24.0, 56.0],
    [40.0, 40.0, 56.0, 56.0],
];
const COLORS: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 1.0],
    [0.0, 1.0, 0.0, 1.0],
    [0.0, 0.0, 1.0, 1.0],
    [1.0, 1.0, 0.0, 1.0],
];
const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const YELLOW: [u8; 4] = [255, 255, 0, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 255];

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    draw_index: bool,
}

const PAGE_BYTES: u64 = 256 << 20;

fn instance_adapter() -> Option<wgpu::Adapter> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle().with_env());
    pollster::block_on(instance.request_adapter(&Default::default())).ok()
}

fn gpu() -> Option<Gpu> {
    let adapter = instance_adapter()?;
    let status = mesh_shader::status(&adapter, PAGE_BYTES);
    eprintln!(
        "mesh shader GPU: {:?}, status: {status:?}",
        adapter.get_info()
    );
    if status.is_err() {
        return None;
    }
    let required_features = wgpu::Features::EXPERIMENTAL_MESH_SHADER
        | (adapter.features() & wgpu::Features::SHADER_DRAW_INDEX);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("mesh shader test"),
        required_features,
        required_limits: adapter.limits(),
        experimental_features: unsafe { wgpu::ExperimentalFeatures::enabled() },
        ..Default::default()
    }))
    .expect("device with EXPERIMENTAL_MESH_SHADER");
    Some(Gpu {
        device,
        queue,
        draw_index: required_features.contains(wgpu::Features::SHADER_DRAW_INDEX),
    })
}

enum Draw {
    Direct,
    Indirect,
    MultiIndirect,
}

fn render(gpu: &Gpu, visible: [f32; 4], draw: Draw) -> Vec<u8> {
    let device = &gpu.device;
    let pipeline = demo::create_pipeline(
        device,
        wgpu::TextureFormat::Rgba8Unorm,
        matches!(draw, Draw::MultiIndirect),
    );
    let view = demo::View {
        visible,
        target_size: [SIZE as f32, SIZE as f32, 0.0, 0.0],
    };
    let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("view"),
        contents: bytemuck::bytes_of(&view),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let storage = |label, data: &[[f32; 4]]| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(data),
            usage: wgpu::BufferUsages::STORAGE,
        })
    };
    let rects = storage("rects", &RECTS);
    let colors = storage("colors", &COLORS);
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("scene"),
        layout: &pipeline.layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: rects.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: colors.as_entire_binding(),
            },
        ],
    });
    let indirect_words: &[u32] = match draw {
        Draw::Direct => &[],
        Draw::Indirect => &[4, 1, 1],
        Draw::MultiIndirect => &[2, 1, 1, 2, 1, 1],
    };
    let indirect = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("indirect"),
        contents: bytemuck::cast_slice(if indirect_words.is_empty() {
            &[0u32; 3]
        } else {
            indirect_words
        }),
        usage: wgpu::BufferUsages::INDIRECT,
    });
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(SIZE * SIZE * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mesh shader"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(0, &group, &[]);
        match draw {
            Draw::Direct => pass.draw_mesh_tasks(4, 1, 1),
            Draw::Indirect => pass.draw_mesh_tasks_indirect(&indirect, 0),
            Draw::MultiIndirect => pass.multi_draw_mesh_tasks_indirect(&indirect, 0, 2),
        }
    }
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SIZE * 4),
                rows_per_image: Some(SIZE),
            },
        },
        wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    if let Some(error) = pollster::block_on(validation.pop()) {
        panic!("validation error: {error}");
    }
    readback.map_async(wgpu::MapMode::Read, .., |r| r.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let pixels = readback.get_mapped_range(..).unwrap().to_vec();
    readback.unmap();
    pixels
}

fn pixel(image: &[u8], x: u32, y: u32) -> [u8; 4] {
    let at = ((y * SIZE + x) * 4) as usize;
    image[at..at + 4].try_into().unwrap()
}

fn assert_filled(image: &[u8], rect: [u32; 4], expected: [u8; 4]) {
    let [x0, y0, x1, y1] = rect;
    for (x, y) in [
        (x0, y0),
        (x1 - 1, y0),
        (x0, y1 - 1),
        (x1 - 1, y1 - 1),
        ((x0 + x1) / 2, (y0 + y1) / 2),
    ] {
        assert_eq!(pixel(image, x, y), expected, "inside at ({x}, {y})");
    }
    for (x, y) in [(x0 - 1, y0), (x1, y0), (x0, y0 - 1), (x0, y1), (x1 - 1, y1)] {
        assert_eq!(pixel(image, x, y), CLEAR, "outside at ({x}, {y})");
    }
}

fn assert_left_half_only(image: &[u8]) {
    assert_filled(image, [8, 8, 24, 24], RED);
    assert_filled(image, [8, 40, 24, 56], BLUE);
    for (x, y) in [
        (48, 16),
        (40, 8),
        (55, 23),
        (48, 48),
        (40, 40),
        (4, 4),
        (63, 63),
    ] {
        assert_eq!(
            pixel(image, x, y),
            CLEAR,
            "culled meshlet drew at ({x}, {y})"
        );
    }
}

#[test]
fn task_shader_culls_and_mesh_shader_draws_quads() {
    let Some(gpu) = gpu() else {
        return;
    };
    let image = render(&gpu, [0.0, 0.0, 32.0, 64.0], Draw::Direct);
    assert_left_half_only(&image);
}

#[test]
fn nothing_is_drawn_when_every_meshlet_is_culled() {
    let Some(gpu) = gpu() else {
        return;
    };
    let image = render(&gpu, [100.0, 100.0, 200.0, 200.0], Draw::Direct);
    assert!(image.as_chunks::<4>().0.iter().all(|p| *p == CLEAR));
}

#[test]
fn every_meshlet_draws_when_all_are_visible() {
    let Some(gpu) = gpu() else {
        return;
    };
    let image = render(&gpu, [0.0, 0.0, 64.0, 64.0], Draw::Direct);
    assert_filled(&image, [8, 8, 24, 24], RED);
    assert_filled(&image, [40, 8, 56, 24], GREEN);
    assert_filled(&image, [8, 40, 24, 56], BLUE);
    assert_filled(&image, [40, 40, 56, 56], YELLOW);
}

#[test]
fn indirect_draw_matches_the_direct_draw() {
    let Some(gpu) = gpu() else {
        return;
    };
    let direct = render(&gpu, [0.0, 0.0, 32.0, 64.0], Draw::Direct);
    let indirect = render(&gpu, [0.0, 0.0, 32.0, 64.0], Draw::Indirect);
    assert_left_half_only(&indirect);
    assert_eq!(direct, indirect);
}

#[test]
fn multi_draw_indirect_sees_each_draw_index() {
    let Some(gpu) = gpu() else {
        return;
    };
    if !gpu.draw_index {
        eprintln!("SHADER_DRAW_INDEX missing: multi-draw test not run");
        return;
    }
    let image = render(&gpu, [0.0, 0.0, 64.0, 64.0], Draw::MultiIndirect);
    assert_filled(&image, [8, 8, 24, 24], RED);
    assert_filled(&image, [40, 8, 56, 24], GREEN);
    assert_filled(&image, [8, 40, 24, 56], BLUE);
    assert_filled(&image, [40, 40, 56, 56], YELLOW);
}

fn validate(source: &str, capabilities: naga::valid::Capabilities) -> Result<(), String> {
    let module = naga::front::wgsl::parse_str(source).map_err(|e| e.emit_to_string(source))?;
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), capabilities)
        .validate(&module)
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}

#[test]
fn shaders_validate_with_the_mesh_capability() {
    let mesh = naga::valid::Capabilities::MESH_SHADER;
    validate(demo::SOURCE, mesh).unwrap();
    validate(
        demo::DRAW_SOURCE,
        mesh | naga::valid::Capabilities::DRAW_INDEX,
    )
    .unwrap();
}

#[test]
fn shaders_are_rejected_without_the_mesh_capability() {
    let none = naga::valid::Capabilities::empty();
    assert!(validate(demo::SOURCE, none).is_err());
    assert!(
        validate(demo::DRAW_SOURCE, naga::valid::Capabilities::MESH_SHADER).is_err(),
        "draw_index needs its own capability"
    );
}

#[test]
fn shaders_compile_to_spirv() {
    let capabilities = naga::valid::Capabilities::MESH_SHADER;
    let module = naga::front::wgsl::parse_str(demo::SOURCE).unwrap();
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), capabilities)
        .validate(&module)
        .unwrap();
    let options = naga::back::spv::Options {
        lang_version: (1, 4),
        capabilities: Some(
            [
                naga::back::spv::Capability::Shader,
                naga::back::spv::Capability::MeshShadingEXT,
            ]
            .into_iter()
            .collect(),
        ),
        ..Default::default()
    };
    let words = naga::back::spv::write_vec(&module, &info, &options, None).unwrap();
    assert!(words.len() > 100);
}

#[test]
fn a_device_needs_the_experimental_opt_in() {
    let Some(adapter) = instance_adapter() else {
        return;
    };
    if mesh_shader::unsupported(&adapter, mesh_shader::SHAPE, PAGE_BYTES).is_some() {
        return;
    }
    let refused = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::EXPERIMENTAL_MESH_SHADER,
        required_limits: adapter.limits(),
        ..Default::default()
    }));
    let message = refused.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(
        message.contains("xperimental"),
        "unexpected result: {message:?}"
    );
}

#[test]
fn the_features_asked_for_are_the_ones_the_adapter_offers() {
    let Some(adapter) = instance_adapter() else {
        return;
    };
    let features = mesh_shader::features(&adapter, PAGE_BYTES);
    if mesh_shader::unsupported(&adapter, mesh_shader::SHAPE, PAGE_BYTES).is_none() {
        assert!(features.contains(wgpu::Features::EXPERIMENTAL_MESH_SHADER));
        assert!(features.contains(wgpu::Features::IMMEDIATES));
        assert!(adapter.features().contains(features));
    } else {
        assert!(features.is_empty());
    }
}

fn generous() -> wgpu::Limits {
    wgpu::Limits {
        max_task_invocations_per_workgroup: 128,
        max_task_invocations_per_dimension: 128,
        max_mesh_invocations_per_workgroup: 128,
        max_mesh_invocations_per_dimension: 128,
        max_mesh_output_vertices: 256,
        max_mesh_output_primitives: 256,
        max_task_payload_size: 16384,
        max_task_workgroups_per_dimension: 65535,
        max_task_workgroup_total_count: 1 << 22,
        max_mesh_workgroups_per_dimension: 65535,
        max_mesh_workgroup_total_count: 1 << 22,
        max_compute_workgroup_storage_size: 32768,
        max_immediate_size: 128,
        max_bind_groups: 4,
        max_storage_buffers_per_shader_stage: 8,
        max_storage_buffer_binding_size: u64::from(u32::MAX),
        ..wgpu::Limits::defaults()
    }
}

#[test]
fn limits_that_hold_the_shape_report_no_shortfall() {
    assert!(mesh_shader::shortfalls(&generous(), mesh_shader::SHAPE, PAGE_BYTES).is_empty());
}

#[test]
fn each_missing_limit_is_named() {
    let shape = mesh_shader::SHAPE;
    let cases: [(&str, fn(&mut wgpu::Limits)); 6] = [
        ("mesh output vertices", |l| l.max_mesh_output_vertices = 64),
        ("mesh output primitives", |l| {
            l.max_mesh_output_primitives = 32
        }),
        ("mesh workgroups per dimension", |l| {
            l.max_mesh_workgroups_per_dimension = 1024
        }),
        ("workgroup memory bytes", |l| {
            l.max_compute_workgroup_storage_size = 8192
        }),
        ("immediate bytes", |l| l.max_immediate_size = 0),
        ("storage binding bytes", |l| {
            l.max_storage_buffer_binding_size = 1 << 20
        }),
    ];
    for (name, lower) in cases {
        let mut limits = generous();
        lower(&mut limits);
        let short = mesh_shader::shortfalls(&limits, shape, PAGE_BYTES);
        assert_eq!(short.len(), 1, "{name}: {short:?}");
        assert!(short[0].starts_with(name), "{name}: {short:?}");
    }
}

#[test]
fn a_device_without_mesh_limits_is_unsupported() {
    let short = mesh_shader::shortfalls(&wgpu::Limits::defaults(), mesh_shader::SHAPE, PAGE_BYTES);
    assert!(short.len() >= 8, "{short:?}");
}

#[test]
fn the_meshlet_fits_the_output_the_hardware_allows() {
    let shape = mesh_shader::SHAPE;
    assert!(shape.vertices() <= 256 && shape.primitives() <= 256);
    assert!(shape.mesh_threads <= 128 && shape.task_threads <= 128);
    assert!(shape.workgroup_bytes() <= 28 * 1024);
}
