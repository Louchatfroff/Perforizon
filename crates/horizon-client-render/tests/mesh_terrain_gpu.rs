#![allow(dead_code)]
#[path = "../src/gfx/mesh_shader.rs"]
mod mesh_shader;
#[path = "../src/gfx/mesh_terrain.rs"]
mod mesh_terrain;

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

const SIZE: u32 = 64;
const PAGE_BYTES: u64 = 256 << 20;
const TERRAIN: &str = include_str!("../src/terrain.wgsl");
const LAYERS: u32 = 4;
const TEXELS: u32 = 16;
const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const OPAQUE: u16 = 0x8000;
const BACKGROUND: [u8; 4] = [25, 51, 76, 255];
const STEP: f32 = 1.0 / 32768.0;

const ATTRIBUTES: [wgpu::VertexAttribute; 5] =
    wgpu::vertex_attr_array![0=>Float32x3,1=>Uint16x2,2=>Uint16,4=>Uint8x2,3=>Unorm8x4];

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 3],
    uv: [u16; 2],
    layer: u16,
    light: u16,
    color: u32,
}

#[derive(Clone, Copy)]
struct Look {
    layer: u16,
    color: [u8; 4],
    light: [u8; 2],
}

impl Look {
    fn new(layer: u16, rgb: [u8; 3]) -> Self {
        Self {
            layer,
            color: [rgb[0], rgb[1], rgb[2], 255],
            light: [200, 40],
        }
    }
}

fn quad(corners: [[f32; 3]; 4], look: Look) -> [Vertex; 4] {
    let uvs = [[0u16, 0], [32768, 0], [32768, 32768], [0, 32768]];
    std::array::from_fn(|i| Vertex {
        pos: corners[i],
        uv: uvs[i],
        layer: look.layer,
        light: u16::from(look.light[0]) | u16::from(look.light[1]) << 8,
        color: u32::from_le_bytes(look.color),
    })
}

fn facing(x: f32, y: f32, z: f32, size: f32) -> [[f32; 3]; 4] {
    [
        [x, y, z],
        [x + size, y, z],
        [x + size, y + size, z],
        [x, y + size, z],
    ]
}

fn turned_away(corners: [[f32; 3]; 4]) -> [[f32; 3]; 4] {
    [corners[0], corners[3], corners[2], corners[1]]
}

fn instance(origin: [f32; 3], fade: u32) -> [f32; 4] {
    let w = if fade == 0 {
        STEP
    } else {
        -(fade as f32 + STEP)
    };
    [origin[0], origin[1], origin[2], w]
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn gpu() -> Option<Gpu> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle().with_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
    if let Err(reason) = mesh_shader::status(&adapter, PAGE_BYTES) {
        eprintln!("mesh terrain GPU test skipped: {reason}");
        return None;
    }
    let required_features = mesh_shader::features(&adapter, PAGE_BYTES)
        | (adapter.features()
            & (wgpu::Features::INDIRECT_FIRST_INSTANCE
                | wgpu::Features::PIPELINE_STATISTICS_QUERY));
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("mesh terrain test"),
        required_features,
        required_limits: adapter.limits(),
        experimental_features: unsafe { wgpu::ExperimentalFeatures::enabled() },
        ..Default::default()
    }))
    .expect("a device with mesh shaders");
    Some(Gpu { device, queue })
}

fn put(bytes: &mut [u8], offset: usize, values: &[f32]) {
    bytes[offset..offset + values.len() * 4].copy_from_slice(bytemuck::cast_slice(values));
}

fn put_words(bytes: &mut [u8], offset: usize, values: &[u32]) {
    bytes[offset..offset + values.len() * 4].copy_from_slice(bytemuck::cast_slice(values));
}

const GLOBALS_BYTES: usize = 2160;

fn globals(view_proj: Mat4) -> Vec<u8> {
    let mut bytes = vec![0u8; GLOBALS_BYTES];
    put(&mut bytes, 0, &view_proj.to_cols_array());
    put(&mut bytes, 64, &[0.55, 0.65, 0.8, 1.0]);
    put(&mut bytes, 80, &[2.0, 60.0, 4.0, 80.0]);
    put(&mut bytes, 272 + 8, &[1.0]);
    put(&mut bytes, 320, &[0.0, 0.0, -5.0, 12.0]);
    put_words(&mut bytes, 1104, &[2, 3, 0, 0]);
    put_words(&mut bytes, 1616, &[1, 0, 0, 0]);
    put(&mut bytes, 2144, &[0.0, TEXELS as f32, 0.0, 0.0]);
    bytes
}

#[test]
fn the_globals_offsets_match_terrain_wgsl() {
    let module = naga::front::wgsl::parse_str(TERRAIN).unwrap();
    let (_, globals) = module
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("Globals"))
        .unwrap();
    let naga::TypeInner::Struct { members, span } = &globals.inner else {
        panic!("Globals is a struct");
    };
    let offset = |name: &str| {
        members
            .iter()
            .find(|m| m.name.as_deref() == Some(name))
            .unwrap()
            .offset as usize
    };
    assert_eq!(offset("view_proj"), 0);
    assert_eq!(offset("fog_color"), 64);
    assert_eq!(offset("fog"), 80);
    assert_eq!(offset("time"), 272);
    assert_eq!(offset("dynamic_lights"), 320);
    assert_eq!(offset("emissive_layers"), 1104);
    assert_eq!(offset("emissive_count"), 1616);
    assert_eq!(offset("filtering"), 2144);
    assert_eq!(*span as usize, GLOBALS_BYTES);
}

fn sprite_array(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let mut pixels = Vec::new();
    for layer in 0..LAYERS {
        for y in 0..TEXELS {
            for x in 0..TEXELS {
                let checker = (x / 4 + y / 4) % 2 == 0;
                let value = if checker { 220 } else { 60 };
                pixels.extend_from_slice(&match layer {
                    0 => [value, (x * 16) as u8, 90, 255],
                    1 => [30, value, (y * 16) as u8, 255],
                    2 => [value, 40, value / 2, if x < 8 { 255 } else { 0 }],
                    _ => [255 - value, 200, 20, if checker { 255 } else { 90 }],
                });
            }
        }
    }
    device
        .create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("sprites"),
                size: wgpu::Extent3d {
                    width: TEXELS,
                    height: TEXELS,
                    depth_or_array_layers: LAYERS,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: COLOR,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &pixels,
        )
        .create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        })
}

fn plain_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    pixels: &[u8],
) -> wgpu::TextureView {
    device
        .create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("plain"),
                size: wgpu::Extent3d {
                    width,
                    height: pixels.len() as u32 / 4 / width,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: COLOR,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            pixels,
        )
        .create_view(&Default::default())
}

struct Resources {
    classic_layout: wgpu::BindGroupLayout,
    classic_group: wgpu::BindGroup,
    mesh_group: wgpu::BindGroup,
}

fn classic_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let all = wgpu::ShaderStages::VERTEX_FRAGMENT;
    let sprites = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
        view_dimension: wgpu::TextureViewDimension::D2Array,
        multisampled: false,
    };
    let plain = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    };
    let sampler = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
    let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: all,
        ty,
        count: None,
    };
    let mut entries = vec![
        entry(
            0,
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
        ),
        entry(1, sprites),
        entry(2, sampler),
        entry(3, plain),
        entry(4, sampler),
    ];
    entries.extend((5..=11).map(|binding| entry(binding, sprites)));
    entries.push(entry(12, plain));
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("classic terrain bindings"),
        entries: &entries,
    })
}

fn resources(gpu: &Gpu, mesh_layout: &wgpu::BindGroupLayout, view_proj: Mat4) -> Resources {
    let device = &gpu.device;
    let sprites = sprite_array(device, &gpu.queue);
    let unused = device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("unused sprites"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
    let mut lightmap = Vec::new();
    for sky in 0..16u32 {
        for block in 0..16u32 {
            lightmap.extend_from_slice(&[
                (60 + sky * 12) as u8,
                (40 + block * 8 + sky * 4) as u8,
                (90 + block * 10) as u8,
                255,
            ]);
        }
    }
    let lightmap = plain_texture(device, &gpu.queue, 16, &lightmap);
    let thresholds = plain_texture(
        device,
        &gpu.queue,
        4,
        &[
            128, 128, 0, 255, 90, 200, 0, 255, 255, 255, 0, 255, 10, 10, 0, 255,
        ],
    );
    let sprite_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    });
    let lightmap_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let globals = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("globals"),
        contents: &globals(view_proj),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let classic_layout = classic_layout(device);
    let group = |layout: &wgpu::BindGroupLayout| {
        let view = |binding, view| wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::TextureView(view),
        };
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            },
            view(1, &sprites),
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&sprite_sampler),
            },
            view(3, &lightmap),
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(&lightmap_sampler),
            },
        ];
        entries.extend((5..=11).map(|binding| view(binding, &unused)));
        entries.push(view(12, &thresholds));
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain resources"),
            layout,
            entries: &entries,
        })
    };
    Resources {
        classic_group: group(&classic_layout),
        mesh_group: group(mesh_layout),
        classic_layout,
    }
}

fn classic_pipeline(gpu: &Gpu, layout: &wgpu::BindGroupLayout) -> wgpu::RenderPipeline {
    let device = &gpu.device;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("terrain"),
        source: wgpu::ShaderSource::Wgsl(TERRAIN.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("terrain"),
        bind_group_layouts: &[Some(layout)],
        ..Default::default()
    });
    let instance_attributes = wgpu::vertex_attr_array![5=>Float32x4];
    let buffers = [
        Some(wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }),
        Some(wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &instance_attributes,
        }),
    ];
    let constants = [("SPRITE_ARRAYS", 1.0), ("SPRITE_SPLIT", 2048.0)];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("terrain solid"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_terrain"),
            compilation_options: Default::default(),
            buffers: &buffers,
        },
        primitive: wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_solid"),
            compilation_options: wgpu::PipelineCompilationOptions {
                constants: &constants,
                ..Default::default()
            },
            targets: &[Some(wgpu::ColorTargetState {
                format: COLOR,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn quad_indices(device: &wgpu::Device, quads: u32) -> wgpu::Buffer {
    let indices: Vec<u32> = (0..quads)
        .flat_map(|q| {
            let b = q * 4;
            [b, b + 1, b + 2, b, b + 2, b + 3]
        })
        .collect();
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("quad indices"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    })
}

#[derive(Default)]
struct Scene {
    vertices: Vec<Vertex>,
    origins: Vec<[f32; 4]>,
    draws: Vec<[u32; 5]>,
}

impl Scene {
    fn add(&mut self, quads: &[[Vertex; 4]], origin: [f32; 4]) {
        self.draw(quads, origin, 1);
    }

    fn draw(&mut self, quads: &[[Vertex; 4]], origin: [f32; 4], instances: u32) {
        let start = self.vertices.len() as u32;
        let instance = self.origins.len() as u32;
        self.vertices.extend(quads.iter().flatten());
        self.origins.push(origin);
        self.draws
            .push([quads.len() as u32 * 6, instances, 0, start, instance]);
    }
}

struct Frame<'a> {
    gpu: &'a Gpu,
    resources: &'a Resources,
    page: wgpu::Buffer,
    origins: wgpu::Buffer,
    scene: &'a Scene,
}

fn padded_draws(draws: &[[u32; 5]]) -> Vec<u32> {
    draws.iter().flatten().copied().collect()
}

#[derive(Clone, Copy)]
enum Path {
    Classic,
    Meshlets(usize),
}

struct Drawn {
    pixels: Vec<u8>,
    triangles: Option<u64>,
}

fn render(
    frame: &Frame,
    path: Path,
    mesh: &mut mesh_terrain::MeshTerrain,
    runs: &[(u32, u32, u32)],
    deferred: Option<&[[u32; 5]]>,
) -> Drawn {
    let device = &frame.gpu.device;
    let statistics = device
        .features()
        .contains(wgpu::Features::PIPELINE_STATISTICS_QUERY)
        .then(|| {
            let set = device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("clipper"),
                ty: wgpu::QueryType::PipelineStatistics(
                    wgpu::PipelineStatisticsTypes::CLIPPER_INVOCATIONS,
                ),
                count: 1,
            });
            let resolved = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("clipper resolved"),
                size: 8,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let read = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("clipper read"),
                size: 8,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            (set, resolved, read)
        });
    let draws = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("draws"),
        contents: bytemuck::cast_slice(&padded_draws(&frame.scene.draws)),
        usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::STORAGE,
    });
    let deferred = deferred.map(|records| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("deferred draws"),
            contents: bytemuck::cast_slice(&padded_draws(records)),
            usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::STORAGE,
        })
    });
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("color"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: COLOR,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let depth = device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(SIZE * SIZE * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let color_view = color.create_view(&Default::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("terrain"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        if let Some((set, ..)) = &statistics {
            pass.begin_pipeline_statistics_query(set, 0);
        }
        match path {
            Path::Classic => {
                let pipeline = classic_pipeline(frame.gpu, &frame.resources.classic_layout);
                let indices = quad_indices(device, frame.scene.vertices.len() as u32 / 4);
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &frame.resources.classic_group, &[]);
                pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.set_vertex_buffer(0, frame.page.slice(..));
                pass.set_vertex_buffer(1, frame.origins.slice(..));
                pass.multi_draw_indexed_indirect(&draws, 0, frame.scene.draws.len() as u32);
            }
            Path::Meshlets(scene) => {
                mesh.prepare(
                    device,
                    [Some(&draws), deferred.as_ref()],
                    &frame.origins,
                    [(0, &frame.page)],
                );
                mesh.draw(
                    &mut pass,
                    &frame.resources.mesh_group,
                    scene,
                    runs.iter().copied(),
                );
            }
        }
        if statistics.is_some() {
            pass.end_pipeline_statistics_query();
        }
    }
    if let Some((set, resolved, read)) = &statistics {
        encoder.resolve_query_set(set, 0..1, resolved, 0);
        encoder.copy_buffer_to_buffer(resolved, 0, read, 0, 8);
    }
    encoder.copy_texture_to_buffer(
        color.as_image_copy(),
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
    frame.gpu.queue.submit([encoder.finish()]);
    if let Some(error) = pollster::block_on(scope.pop()) {
        panic!("validation error: {error}");
    }
    readback.map_async(wgpu::MapMode::Read, .., |r| r.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let pixels = readback.get_mapped_range(..).unwrap().to_vec();
    readback.unmap();
    let triangles = statistics.map(|(_, _, read)| {
        read.map_async(wgpu::MapMode::Read, .., |r| r.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let count = u64::from_le_bytes(read.get_mapped_range(..).unwrap()[..8].try_into().unwrap());
        read.unmap();
        count
    });
    Drawn { pixels, triangles }
}

struct Setup {
    gpu: Gpu,
    resources: Resources,
    mesh: mesh_terrain::MeshTerrain,
}

fn setup() -> Option<Setup> {
    let gpu = gpu()?;
    let mesh =
        mesh_terrain::MeshTerrain::new(&gpu.device, COLOR, [1.0, 2048.0], mesh_shader::SHAPE)
            .expect("the terrain meshlet pipeline");
    let projection = Mat4::perspective_rh(70f32.to_radians(), 1.0, 256.0, 0.05);
    let view = Mat4::look_to_rh(Vec3::ZERO, -Vec3::Z, Vec3::Y);
    let resources = resources(&gpu, mesh.terrain_layout(), projection * view);
    Some(Setup {
        gpu,
        resources,
        mesh,
    })
}

fn compare(setup: &mut Setup, scene: &Scene) -> (Vec<u8>, Vec<u8>) {
    let (classic, meshlets) = compare_drawn(setup, scene);
    (classic.pixels, meshlets.pixels)
}

fn compare_drawn(setup: &mut Setup, scene: &Scene) -> (Drawn, Drawn) {
    let device = &setup.gpu.device;
    let page = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("page"),
        contents: bytemuck::cast_slice(&scene.vertices),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
    });
    let origins = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("origins"),
        contents: bytemuck::cast_slice(&scene.origins),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
    });
    let frame = Frame {
        gpu: &setup.gpu,
        resources: &setup.resources,
        page,
        origins,
        scene,
    };
    let all = [(0, 0, scene.draws.len() as u32)];
    let classic = render(&frame, Path::Classic, &mut setup.mesh, &all, None);
    let meshlets = render(
        &frame,
        Path::Meshlets(mesh_terrain::FRAME),
        &mut setup.mesh,
        &all,
        None,
    );
    (classic, meshlets)
}

fn pixel(image: &[u8], x: u32, y: u32) -> [u8; 4] {
    let at = ((y * SIZE + x) * 4) as usize;
    image[at..at + 4].try_into().unwrap()
}

fn distinct(image: &[u8]) -> usize {
    let mut seen = std::collections::HashSet::new();
    for p in image.as_chunks::<4>().0 {
        seen.insert(*p);
    }
    seen.len()
}

fn differing(a: &[u8], b: &[u8]) -> usize {
    a.as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .filter(|(a, b)| a != b)
        .count()
}

fn wall(scene: &mut Scene, z: f32, cells: u32, first_layer: u16) {
    let size = 6.0 / cells as f32;
    let mut quads = Vec::new();
    for row in 0..cells {
        for column in 0..cells {
            let n = row * cells + column;
            let look = Look {
                layer: (first_layer + (n % 2) as u16 * 2 + n.is_multiple_of(3) as u16)
                    % LAYERS as u16,
                color: [
                    (80 + (n * 37) % 170) as u8,
                    (80 + (n * 59) % 170) as u8,
                    (80 + (n * 83) % 170) as u8,
                    255,
                ],
                light: [(40 + (n * 13) % 200) as u8, (n * 29 % 160) as u8],
            };
            quads.push(quad(
                facing(
                    -3.0 + column as f32 * size,
                    -3.0 + row as f32 * size,
                    z,
                    size,
                ),
                look,
            ));
        }
    }
    scene.add(&quads, instance([0.0; 3], 0));
}

#[test]
fn meshlets_draw_the_pixels_the_indexed_draw_does() {
    let Some(mut setup) = setup() else {
        return;
    };
    let mut scene = Scene::default();
    wall(&mut scene, -6.0, 12, 0);
    let (classic, meshlets) = compare(&mut setup, &scene);
    assert!(
        distinct(&classic) > 20,
        "the scene should show terrain, not a flat colour"
    );
    assert_eq!(differing(&classic, &meshlets), 0);
    assert_eq!(classic, meshlets);
}

#[test]
fn many_draws_overlap_and_fade_like_the_indexed_draws_do() {
    let Some(mut setup) = setup() else {
        return;
    };
    let mut scene = Scene::default();
    wall(&mut scene, -9.0, 7, 0);
    wall(&mut scene, -7.5, 5, 1);
    let overlay = [
        quad(
            facing(-1.0, -1.0, -5.0, 2.0),
            Look::new(2 | OPAQUE, [250, 90, 40]),
        ),
        quad(facing(-0.5, -0.5, -5.0, 1.0), Look::new(3, [40, 220, 120])),
        quad(
            facing(0.0, -1.5, -5.0, 2.0),
            Look {
                color: [200, 200, 255, 0xfd],
                ..Look::new(1, [200, 200, 255])
            },
        ),
    ];
    scene.add(&overlay, instance([0.0; 3], 0));
    scene.add(&overlay, instance([1.5, 0.5, -1.0], 0));
    scene.add(&overlay, instance([-2.0, -0.5, -2.0], 70));
    scene.add(&overlay, instance([-1.0, 1.0, -1.5], 129 + 20));
    let (classic, meshlets) = compare(&mut setup, &scene);
    assert!(distinct(&classic) > 30);
    assert_eq!(differing(&classic, &meshlets), 0);
}

#[test]
fn a_quad_turned_away_is_culled_and_a_quad_facing_the_eye_is_not() {
    let Some(mut setup) = setup() else {
        return;
    };
    let front = facing(-1.0, -1.0, -4.0, 2.0);
    let look = Look::new(OPAQUE, [255, 255, 255]);
    let mut shown = Scene::default();
    shown.add(&[quad(front, look)], instance([0.0; 3], 0));
    let mut hidden = Scene::default();
    hidden.add(&[quad(turned_away(front), look)], instance([0.0; 3], 0));
    let (classic_shown, meshlets_shown) = compare(&mut setup, &shown);
    let (classic_hidden, meshlets_hidden) = compare(&mut setup, &hidden);
    assert_ne!(pixel(&classic_shown, 32, 32), BACKGROUND);
    assert_eq!(pixel(&classic_hidden, 32, 32), BACKGROUND);
    assert_eq!(classic_shown, meshlets_shown);
    assert_eq!(classic_hidden, meshlets_hidden);
    assert!(
        classic_hidden
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == BACKGROUND)
    );
}

#[test]
fn coplanar_quads_keep_their_drawing_order() {
    let Some(mut setup) = setup() else {
        return;
    };
    let mut quads = Vec::new();
    for n in 0..40u32 {
        let shade = (40 + n * 5) as u8;
        quads.push(quad(
            facing(-1.0, -1.0, -4.0, 2.0),
            Look::new(OPAQUE, [shade, 255 - shade, 128]),
        ));
    }
    let mut scene = Scene::default();
    scene.add(&quads, instance([0.0; 3], 0));
    let (classic, meshlets) = compare(&mut setup, &scene);
    assert_eq!(classic, meshlets);
    let last = Look::new(OPAQUE, [40 + 39 * 5, 255 - (40 + 39 * 5), 128]);
    let tinted = quads.last().unwrap()[0].color;
    assert_eq!(tinted, u32::from_le_bytes(last.color));
}

#[test]
fn a_draw_without_an_instance_draws_nothing_until_it_has_one() {
    let Some(mut setup) = setup() else {
        return;
    };
    let look = Look::new(OPAQUE, [255, 120, 60]);
    let quads = [quad(facing(-1.0, -1.0, -4.0, 2.0), look)];
    let mut skipped = Scene::default();
    skipped.draw(&quads, instance([0.0; 3], 0), 0);
    let (classic, meshlets) = compare(&mut setup, &skipped);
    assert!(meshlets.as_chunks::<4>().0.iter().all(|p| *p == BACKGROUND));
    assert_eq!(classic, meshlets);
    let mut drawn = Scene::default();
    drawn.draw(&quads, instance([0.0; 3], 0), 1);
    let (classic, meshlets) = compare(&mut setup, &drawn);
    assert_ne!(pixel(&meshlets, 32, 32), BACKGROUND);
    assert_eq!(classic, meshlets);
}

#[test]
fn a_section_wholly_outside_the_view_is_left_out_without_changing_the_picture() {
    let Some(mut setup) = setup() else {
        return;
    };
    let mut scene = Scene::default();
    wall(&mut scene, -6.0, 6, 0);
    let aside = [quad(
        facing(0.0, 0.0, 0.0, 4.0),
        Look::new(1 | OPAQUE, [10, 250, 10]),
    )];
    scene.add(&aside, instance([200.0, 0.0, -30.0], 0));
    scene.add(&aside, instance([-40.0, 0.0, 60.0], 0));
    scene.add(&aside, instance([0.0, -90.0, -20.0], 0));
    let (classic, meshlets) = compare_drawn(&mut setup, &scene);
    assert_eq!(classic.pixels, meshlets.pixels);
    let (Some(classic), Some(meshlets)) = (classic.triangles, meshlets.triangles) else {
        eprintln!("pipeline statistics are not offered: the cull is not counted");
        return;
    };
    let wall_triangles = 6 * 6 * 2;
    assert_eq!(classic, wall_triangles + 3 * 2);
    assert_eq!(meshlets, wall_triangles);
}

#[test]
fn a_draw_larger_than_one_meshlet_is_split_without_gaps() {
    let Some(mut setup) = setup() else {
        return;
    };
    let shape = mesh_shader::SHAPE;
    let cells = 16;
    for count in [
        shape.quads - 1,
        shape.quads,
        shape.quads + 1,
        3 * shape.quads + 5,
        cells * cells,
    ] {
        let mut scene = Scene::default();
        let size = 6.0 / cells as f32;
        let quads: Vec<_> = (0..count)
            .map(|n| {
                quad(
                    facing(
                        -3.0 + (n % cells) as f32 * size,
                        -3.0 + (n / cells) as f32 * size,
                        -5.0,
                        size * 0.9,
                    ),
                    Look::new((n % 4) as u16, [(60 + n * 3 % 190) as u8, 200, 90]),
                )
            })
            .collect();
        scene.add(&quads, instance([0.0; 3], 0));
        let (classic, meshlets) = compare(&mut setup, &scene);
        assert!(distinct(&classic) > 3, "{count} quads");
        assert_eq!(classic, meshlets, "{count} quads");
    }
}

#[test]
fn draws_in_separate_runs_use_the_first_draw_they_are_given() {
    let Some(mut setup) = setup() else {
        return;
    };
    let mut scene = Scene::default();
    for n in 0..6u32 {
        let quads = [quad(
            facing(-3.0 + n as f32, -1.0, -5.0, 0.8),
            Look::new((n % 4) as u16 | OPAQUE, [(40 + n * 40) as u8, 120, 200]),
        )];
        scene.add(&quads, instance([0.0; 3], 0));
    }
    let device = &setup.gpu.device;
    let page = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("page"),
        contents: bytemuck::cast_slice(&scene.vertices),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
    });
    let origins = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("origins"),
        contents: bytemuck::cast_slice(&scene.origins),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
    });
    let frame = Frame {
        gpu: &setup.gpu,
        resources: &setup.resources,
        page,
        origins,
        scene: &scene,
    };
    let all = [(0, 0, 6)];
    let split = [(0, 0, 2), (0, 2, 3), (0, 5, 1)];
    let frame_scene = Path::Meshlets(mesh_terrain::FRAME);
    let classic = render(&frame, Path::Classic, &mut setup.mesh, &all, None).pixels;
    let whole = render(&frame, frame_scene, &mut setup.mesh, &all, None).pixels;
    let pieces = render(&frame, frame_scene, &mut setup.mesh, &split, None).pixels;
    assert!(distinct(&classic) > 4);
    assert_eq!(whole, classic);
    assert_eq!(pieces, classic);
}

#[test]
fn deferred_draws_read_their_instance_counts_from_their_own_buffer() {
    let Some(mut setup) = setup() else {
        return;
    };
    let look = Look::new(OPAQUE, [30, 220, 240]);
    let quads = [quad(facing(-1.0, -1.0, -4.0, 2.0), look)];
    let mut held = Scene::default();
    held.draw(&quads, instance([0.0; 3], 0), 0);
    let mut seen = Scene::default();
    seen.draw(&quads, instance([0.0; 3], 0), 1);
    let device = &setup.gpu.device;
    let page = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("page"),
        contents: bytemuck::cast_slice(&held.vertices),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
    });
    let origins = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("origins"),
        contents: bytemuck::cast_slice(&held.origins),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
    });
    let frame = Frame {
        gpu: &setup.gpu,
        resources: &setup.resources,
        page,
        origins,
        scene: &held,
    };
    let one = [(0, 0, 1)];
    let seen_draws = seen.draws.clone();
    let nothing = render(
        &frame,
        Path::Meshlets(mesh_terrain::FRAME),
        &mut setup.mesh,
        &one,
        Some(&seen_draws),
    )
    .pixels;
    let later = render(
        &frame,
        Path::Meshlets(mesh_terrain::DEFERRED),
        &mut setup.mesh,
        &one,
        Some(&seen_draws),
    )
    .pixels;
    let reference = render(&frame, Path::Classic, &mut setup.mesh, &one, None).pixels;
    assert!(nothing.as_chunks::<4>().0.iter().all(|p| *p == BACKGROUND));
    assert_ne!(pixel(&later, 32, 32), BACKGROUND);
    let counted = Frame {
        scene: &seen,
        ..frame
    };
    let classic_seen = render(&counted, Path::Classic, &mut setup.mesh, &one, None).pixels;
    assert_eq!(later, classic_seen);
    assert!(
        reference
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == BACKGROUND)
    );
}

#[test]
fn draws_group_into_one_run_per_page_counted_from_the_start_of_their_buffer() {
    let pages = [
        (0, None),
        (0, None),
        (2, None),
        (2, None),
        (2, None),
        (1, None),
        (0, None),
    ];
    let runs = |range, base| mesh_terrain::runs(&pages, range, base).collect::<Vec<_>>();
    assert_eq!(runs(0..7, 0), [(0, 0, 2), (2, 2, 3), (1, 5, 1), (0, 6, 1)]);
    assert_eq!(runs(2..6, 0), [(2, 2, 3), (1, 5, 1)]);
    assert_eq!(runs(2..6, 2), [(2, 0, 3), (1, 3, 1)]);
    assert_eq!(runs(1..3, 1), [(0, 0, 1), (2, 1, 1)]);
    assert!(runs(3..3, 0).is_empty());
}
