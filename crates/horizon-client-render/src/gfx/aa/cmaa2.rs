const MAX_SIZE: u32 = 16384;
const OUTPUT_KERNEL: u32 = 14;
const MIN_ITEMS: u64 = 1024;

pub struct Cmaa2 {
    group_layout: wgpu::BindGroupLayout,
    indirect_layout: wgpu::BindGroupLayout,
    edges: wgpu::ComputePipeline,
    args: wgpu::ComputePipeline,
    candidates: wgpu::ComputePipeline,
    apply: wgpu::ComputePipeline,
    blit: Option<Blit>,
    work: Option<Work>,
}

struct Blit {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

struct Staging {
    view: wgpu::TextureView,
    group: wgpu::BindGroup,
}

struct Work {
    size: [u32; 2],
    edges: wgpu::TextureView,
    candidates: wgpu::Buffer,
    locations: wgpu::Buffer,
    items: wgpu::Buffer,
    heads: wgpu::Buffer,
    control: wgpu::Buffer,
    indirect: wgpu::Buffer,
    indirect_group: wgpu::BindGroup,
    staging: Option<Staging>,
}

fn storage_buffer(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn storage_texture(
    binding: u32,
    access: wgpu::StorageTextureAccess,
    format: wgpu::TextureFormat,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access,
            format,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}

fn sampled_texture(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn buffer(
    device: &wgpu::Device,
    label: &str,
    size: u64,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: usage | wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    })
}

fn texture(
    device: &wgpu::Device,
    label: &str,
    size: [u32; 2],
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

fn compute_pass<'a>(encoder: &'a mut wgpu::CommandEncoder, label: &str) -> wgpu::ComputePass<'a> {
    encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some(label),
        timestamp_writes: None,
    })
}

impl Work {
    fn new(
        device: &wgpu::Device,
        size: [u32; 2],
        indirect_layout: &wgpu::BindGroupLayout,
        blit: Option<&Blit>,
    ) -> Self {
        let quads = [size[0].div_ceil(2), size[1].div_ceil(2)];
        let quad_count = u64::from(quads[0]) * u64::from(quads[1]);
        let pixels = u64::from(size[0]) * u64::from(size[1]);
        let cap = device.limits().max_storage_buffer_binding_size & !7;
        let fit = |count: u64, stride: u64| (count * stride).min(cap).max(stride);
        let indirect = buffer(
            device,
            "CMAA2 indirect arguments",
            12,
            wgpu::BufferUsages::INDIRECT,
        );
        let indirect_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("CMAA2 indirect arguments"),
            layout: indirect_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: indirect.as_entire_binding(),
            }],
        });
        let staging = blit.map(|blit| {
            let view = texture(
                device,
                "CMAA2 staging",
                size,
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            );
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("CMAA2 blit"),
                layout: &blit.layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&view),
                }],
            });
            Staging { view, group }
        });
        Self {
            size,
            edges: texture(
                device,
                "CMAA2 edges",
                [quads[0], size[1]],
                wgpu::TextureFormat::R32Uint,
                wgpu::TextureUsages::STORAGE_BINDING,
            ),
            candidates: buffer(
                device,
                "CMAA2 shape candidates",
                fit((pixels / 4).max(MIN_ITEMS), 4),
                wgpu::BufferUsages::empty(),
            ),
            locations: buffer(
                device,
                "CMAA2 blend locations",
                fit(quad_count, 4),
                wgpu::BufferUsages::empty(),
            ),
            items: buffer(
                device,
                "CMAA2 blend items",
                fit((pixels / 2).max(MIN_ITEMS), 8),
                wgpu::BufferUsages::empty(),
            ),
            heads: buffer(
                device,
                "CMAA2 blend item heads",
                fit(quad_count, 4),
                wgpu::BufferUsages::empty(),
            ),
            control: buffer(device, "CMAA2 control", 64, wgpu::BufferUsages::empty()),
            indirect,
            indirect_group,
            staging,
        }
    }
}

impl Cmaa2 {
    pub const OUTPUT_USAGE: wgpu::TextureUsages = wgpu::TextureUsages::RENDER_ATTACHMENT
        .union(wgpu::TextureUsages::TEXTURE_BINDING)
        .union(wgpu::TextureUsages::STORAGE_BINDING);

    pub fn output_usage(format: wgpu::TextureFormat) -> wgpu::TextureUsages {
        if format == wgpu::TextureFormat::Bgra8Unorm {
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING
        } else {
            Self::OUTPUT_USAGE
        }
    }

    pub fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        assert!(
            matches!(
                format,
                wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
            ),
            "CMAA2 takes Rgba8Unorm or Bgra8Unorm, not {format:?}"
        );
        let direct = format == wgpu::TextureFormat::Rgba8Unorm;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("CMAA2"),
            source: wgpu::ShaderSource::Wgsl(include_str!("cmaa2.wgsl").into()),
        });
        let group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("CMAA2"),
            entries: &[
                sampled_texture(0, wgpu::ShaderStages::COMPUTE),
                storage_texture(
                    1,
                    wgpu::StorageTextureAccess::WriteOnly,
                    wgpu::TextureFormat::Rgba8Unorm,
                ),
                storage_texture(
                    2,
                    wgpu::StorageTextureAccess::ReadWrite,
                    wgpu::TextureFormat::R32Uint,
                ),
                storage_buffer(3),
                storage_buffer(4),
                storage_buffer(5),
                storage_buffer(6),
                storage_buffer(7),
            ],
        });
        let indirect_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("CMAA2 indirect arguments"),
            entries: &[storage_buffer(0)],
        });
        let main_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("CMAA2"),
            bind_group_layouts: &[Some(&group_layout)],
            ..Default::default()
        });
        let args_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("CMAA2 dispatch arguments"),
            bind_group_layouts: &[Some(&group_layout), Some(&indirect_layout)],
            ..Default::default()
        });
        let compute = |label: &str, layout: &wgpu::PipelineLayout, entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: crate::gfx::pipeline_cache::get(),
            })
        };
        let blit = (!direct).then(|| {
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("CMAA2 blit"),
                entries: &[sampled_texture(8, wgpu::ShaderStages::FRAGMENT)],
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("CMAA2 blit"),
                bind_group_layouts: &[Some(&layout)],
                ..Default::default()
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("CMAA2 blit"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_blit"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_blit"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: crate::gfx::pipeline_cache::get(),
            });
            Blit { layout, pipeline }
        });
        Self {
            edges: compute("CMAA2 edges", &main_pipeline_layout, "edges_2x2"),
            args: compute(
                "CMAA2 dispatch arguments",
                &args_pipeline_layout,
                "compute_dispatch_args",
            ),
            candidates: compute(
                "CMAA2 shape candidates",
                &main_pipeline_layout,
                "process_candidates",
            ),
            apply: compute(
                "CMAA2 deferred colour apply",
                &main_pipeline_layout,
                "deferred_color_apply",
            ),
            group_layout,
            indirect_layout,
            blit,
            work: None,
        }
    }

    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        input: &wgpu::TextureView,
        size: [u32; 2],
        output: &wgpu::TextureView,
    ) {
        assert!(
            size.iter().all(|&s| (1..=MAX_SIZE).contains(&s)),
            "CMAA2 size {size:?} is outside 1..={MAX_SIZE}"
        );
        if self.work.as_ref().map(|w| w.size) != Some(size) {
            self.work = Some(Work::new(
                device,
                size,
                &self.indirect_layout,
                self.blit.as_ref(),
            ));
        }
        let Some(work) = &self.work else {
            return;
        };
        let target = work.staging.as_ref().map_or(output, |s| &s.view);
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("CMAA2"),
            layout: &self.group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(target),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&work.edges),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: work.candidates.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: work.locations.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: work.items.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: work.heads.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: work.control.as_entire_binding(),
                },
            ],
        });
        let quads = [size[0].div_ceil(2), size[1].div_ceil(2)];
        {
            let mut pass = compute_pass(encoder, "CMAA2 edge detection");
            pass.set_pipeline(&self.edges);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(
                quads[0].div_ceil(OUTPUT_KERNEL),
                quads[1].div_ceil(OUTPUT_KERNEL),
                1,
            );
        }
        {
            let mut pass = compute_pass(encoder, "CMAA2 candidate dispatch arguments");
            pass.set_pipeline(&self.args);
            pass.set_bind_group(0, &group, &[]);
            pass.set_bind_group(1, &work.indirect_group, &[]);
            pass.dispatch_workgroups(2, 1, 1);
        }
        {
            let mut pass = compute_pass(encoder, "CMAA2 shape candidates");
            pass.set_pipeline(&self.candidates);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups_indirect(&work.indirect, 0);
        }
        {
            let mut pass = compute_pass(encoder, "CMAA2 apply dispatch arguments");
            pass.set_pipeline(&self.args);
            pass.set_bind_group(0, &group, &[]);
            pass.set_bind_group(1, &work.indirect_group, &[]);
            pass.dispatch_workgroups(1, 2, 1);
        }
        {
            let mut pass = compute_pass(encoder, "CMAA2 deferred colour apply");
            pass.set_pipeline(&self.apply);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups_indirect(&work.indirect, 0);
        }
        if let (Some(blit), Some(staging)) = (&self.blit, &work.staging) {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("CMAA2 blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&blit.pipeline);
            pass.set_bind_group(0, &staging.group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle().with_env());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
        pollster::block_on(adapter.request_device(&Default::default())).ok()
    }

    fn is_light(pattern: u32, x: u32, y: u32) -> bool {
        match pattern {
            0 => y > x,
            _ => y * 4 > x + 8,
        }
    }

    const DARK: [u8; 3] = [10, 20, 30];
    const LIGHT: [u8; 3] = [240, 160, 80];

    fn alpha(x: u32, y: u32) -> u8 {
        ((x * 7 + y * 13) % 251 + 4) as u8
    }

    fn rgba(pattern: u32, size: u32) -> Vec<[u8; 4]> {
        (0..size * size)
            .map(|i| {
                let (x, y) = (i % size, i / size);
                let c = if is_light(pattern, x, y) { LIGHT } else { DARK };
                [c[0], c[1], c[2], alpha(x, y)]
            })
            .collect()
    }

    fn run(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        size: u32,
        pixels: &[[u8; 4]],
    ) -> Vec<[u8; 4]> {
        let swap = format == wgpu::TextureFormat::Bgra8Unorm;
        let order = |p: &[u8; 4]| if swap { [p[2], p[1], p[0], p[3]] } else { *p };
        let bytes: Vec<u8> = pixels.iter().flat_map(order).collect();
        let extent = wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        };
        let make = |usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let input = make(wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST);
        let output = make(Cmaa2::output_usage(format) | wgpu::TextureUsages::COPY_SRC);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &input,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size * 4),
                rows_per_image: Some(size),
            },
            extent,
        );
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(size * size * 4),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut cmaa2 = Cmaa2::new(device, queue, format);
        let mut encoder = device.create_command_encoder(&Default::default());
        cmaa2.encode(
            device,
            &mut encoder,
            &input.create_view(&Default::default()),
            [size, size],
            &output.create_view(&Default::default()),
        );
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &output,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size * 4),
                    rows_per_image: None,
                },
            },
            extent,
        );
        queue.submit([encoder.finish()]);
        readback.map_async(wgpu::MapMode::Read, .., |r| r.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let data = readback.get_mapped_range(..).unwrap().to_vec();
        data.as_chunks::<4>().0.iter().map(order).collect()
    }

    fn check(pattern: u32, format: wgpu::TextureFormat) {
        let Some((device, queue)) = test_device() else {
            return;
        };
        let size = 64u32;
        let input = rgba(pattern, size);
        let output = run(&device, &queue, format, size, &input);
        let mut intermediate = 0;
        for y in 0..size {
            for x in 0..size {
                let i = (y * size + x) as usize;
                assert_eq!(output[i][3], input[i][3], "alpha at {x},{y}");
                let near_edge = (-3i32..=3).any(|dy| {
                    (-3i32..=3).any(|dx| {
                        let (nx, ny) = (
                            (x as i32 + dx).clamp(0, size as i32 - 1) as u32,
                            (y as i32 + dy).clamp(0, size as i32 - 1) as u32,
                        );
                        is_light(pattern, nx, ny) != is_light(pattern, x, y)
                    })
                });
                if !near_edge {
                    assert_eq!(output[i], input[i], "flat pixel {x},{y} changed");
                } else if (0..3).all(|c| {
                    output[i][c] > DARK[c].min(LIGHT[c]) && output[i][c] < DARK[c].max(LIGHT[c])
                }) {
                    intermediate += 1;
                }
            }
        }
        assert!(
            intermediate >= 20,
            "pattern {pattern} {format:?}: only {intermediate} intermediate pixels"
        );
    }

    #[test]
    fn staircase_is_smoothed_in_rgba() {
        for pattern in 0..2 {
            check(pattern, wgpu::TextureFormat::Rgba8Unorm);
        }
    }

    #[test]
    fn staircase_is_smoothed_in_bgra() {
        for pattern in 0..2 {
            check(pattern, wgpu::TextureFormat::Bgra8Unorm);
        }
    }

    #[test]
    fn flat_image_is_untouched() {
        let Some((device, queue)) = test_device() else {
            return;
        };
        let input: Vec<[u8; 4]> = (0..64 * 64)
            .map(|i| [90, 40, 200, (i % 256) as u8])
            .collect();
        let output = run(&device, &queue, wgpu::TextureFormat::Rgba8Unorm, 64, &input);
        assert_eq!(output, input);
    }

    #[test]
    fn shader_validates() {
        let module = naga::front::wgsl::parse_str(include_str!("cmaa2.wgsl")).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
    }
}
