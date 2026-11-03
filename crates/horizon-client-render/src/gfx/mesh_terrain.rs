use super::mesh_shader::{IMMEDIATE_BYTES, Shape};

const TERRAIN: &str = include_str!("../terrain.wgsl");
const MESH: &str = include_str!("mesh_terrain.wgsl");

pub const FRAME: usize = 0;
pub const DEFERRED: usize = 1;

pub fn source(shape: Shape) -> String {
    format!(
        "enable wgpu_mesh_shader;\n{TERRAIN}\n\
         const MESHLET_QUADS: u32 = {}u;\n\
         const MESHLET_VERTICES: u32 = {}u;\n\
         const MESHLET_PRIMITIVES: u32 = {}u;\n\
         const MESH_THREADS: u32 = {}u;\n\
         const TASK_THREADS: u32 = {}u;\n\
         {MESH}",
        shape.quads,
        shape.vertices(),
        shape.primitives(),
        shape.mesh_threads,
        shape.task_threads,
    )
}

fn entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    ty: wgpu::BindingType,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty,
        count: None,
    }
}

fn storage(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    entry(
        binding,
        visibility,
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
    )
}

pub fn terrain_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    use wgpu::ShaderStages as S;
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
    let mut entries = vec![
        entry(
            0,
            S::TASK | S::MESH | S::FRAGMENT,
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
        ),
        entry(1, S::FRAGMENT, sprites),
        entry(2, S::FRAGMENT, sampler),
        entry(3, S::MESH | S::FRAGMENT, plain),
        entry(4, S::MESH | S::FRAGMENT, sampler),
    ];
    entries.extend((5..=11).map(|binding| entry(binding, S::FRAGMENT, sprites)));
    entries.push(entry(12, S::FRAGMENT, plain));
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Terrain meshlet bindings"),
        entries: &entries,
    })
}

pub fn runs(
    pages: &[(u32, Option<u32>)],
    range: std::ops::Range<u32>,
    base: u32,
) -> impl Iterator<Item = (u32, u32, u32)> + '_ {
    let mut at = range.start as usize;
    let end = range.end as usize;
    std::iter::from_fn(move || {
        if at >= end {
            return None;
        }
        let page = pages[at].0;
        let run = pages[at..end].iter().take_while(|p| p.0 == page).count();
        let first = at as u32 - base;
        at += run;
        Some((page, first, run as u32))
    })
}

struct Scene {
    draws: wgpu::Buffer,
    origins: wgpu::Buffer,
    group: wgpu::BindGroup,
}

struct Page {
    buffer: wgpu::Buffer,
    group: wgpu::BindGroup,
}

pub struct MeshTerrain {
    pipeline: wgpu::RenderPipeline,
    terrain_layout: wgpu::BindGroupLayout,
    scene_layout: wgpu::BindGroupLayout,
    page_layout: wgpu::BindGroupLayout,
    scenes: [Option<Scene>; 2],
    pages: Vec<Option<Page>>,
    dispatch: u32,
}

impl MeshTerrain {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        sprites: [f64; 2],
        shape: Shape,
    ) -> Result<Self, String> {
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
        let terrain_layout = terrain_layout(device);
        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Terrain meshlet draws"),
            entries: &[
                storage(2, wgpu::ShaderStages::TASK),
                storage(3, wgpu::ShaderStages::TASK),
            ],
        });
        let page_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Terrain meshlet vertices"),
            entries: &[storage(0, wgpu::ShaderStages::MESH)],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Terrain meshlets"),
            bind_group_layouts: &[
                Some(&terrain_layout),
                Some(&scene_layout),
                Some(&page_layout),
            ],
            immediate_size: IMMEDIATE_BYTES,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Terrain meshlets"),
            source: wgpu::ShaderSource::Wgsl(source(shape).into()),
        });
        let constants = [("SPRITE_ARRAYS", sprites[0]), ("SPRITE_SPLIT", sprites[1])];
        let pipeline = device.create_mesh_pipeline(&wgpu::MeshPipelineDescriptor {
            label: Some("Terrain solid (meshlets)"),
            layout: Some(&layout),
            task: Some(wgpu::TaskState {
                module: &module,
                entry_point: Some("ts_terrain"),
                compilation_options: Default::default(),
            }),
            mesh: wgpu::MeshState {
                module: &module,
                entry_point: Some("ms_terrain"),
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
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
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });
        let internal = pollster::block_on(internal.pop());
        let validation = pollster::block_on(validation.pop());
        if let Some(error) = validation.or(internal) {
            return Err(error.to_string());
        }
        let limits = device.limits();
        Ok(Self {
            pipeline,
            terrain_layout,
            scene_layout,
            page_layout,
            scenes: [None, None],
            pages: Vec::new(),
            dispatch: limits
                .max_task_workgroups_per_dimension
                .min(limits.max_task_workgroup_total_count)
                .max(1),
        })
    }

    pub fn terrain_layout(&self) -> &wgpu::BindGroupLayout {
        &self.terrain_layout
    }

    pub fn prepare<'a>(
        &mut self,
        device: &wgpu::Device,
        draws: [Option<&wgpu::Buffer>; 2],
        origins: &wgpu::Buffer,
        pages: impl IntoIterator<Item = (u32, &'a wgpu::Buffer)>,
    ) {
        for (scene, draws) in self.scenes.iter_mut().zip(draws) {
            let Some(draws) = draws else {
                *scene = None;
                continue;
            };
            if scene
                .as_ref()
                .is_some_and(|s| s.draws == *draws && s.origins == *origins)
            {
                continue;
            }
            *scene = Some(Scene {
                draws: draws.clone(),
                origins: origins.clone(),
                group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Terrain meshlet draws"),
                    layout: &self.scene_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: draws.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: origins.as_entire_binding(),
                        },
                    ],
                }),
            });
        }
        let mut live = vec![false; self.pages.len()];
        for (id, buffer) in pages {
            let id = id as usize;
            if self.pages.len() <= id {
                self.pages.resize_with(id + 1, || None);
                live.resize(id + 1, false);
            }
            live[id] = true;
            if self.pages[id].as_ref().is_some_and(|p| p.buffer == *buffer) {
                continue;
            }
            self.pages[id] = Some(Page {
                buffer: buffer.clone(),
                group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Terrain meshlet vertices"),
                    layout: &self.page_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.as_entire_binding(),
                    }],
                }),
            });
        }
        for (page, live) in self.pages.iter_mut().zip(live) {
            if !live {
                *page = None;
            }
        }
    }

    pub fn clear(&mut self) {
        self.scenes = [None, None];
        self.pages.clear();
    }

    pub fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        terrain: &wgpu::BindGroup,
        scene: usize,
        runs: impl IntoIterator<Item = (u32, u32, u32)>,
    ) {
        let Some(scene) = &self.scenes[scene] else {
            return;
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, terrain, &[]);
        pass.set_bind_group(1, &scene.group, &[]);
        for (page, mut first, mut left) in runs {
            let Some(Some(page)) = self.pages.get(page as usize) else {
                continue;
            };
            pass.set_bind_group(2, &page.group, &[]);
            while left > 0 {
                let count = left.min(self.dispatch);
                pass.set_immediates(0, &first.to_ne_bytes());
                pass.draw_mesh_tasks(count, 1, 1);
                first += count;
                left -= count;
            }
        }
    }
}
