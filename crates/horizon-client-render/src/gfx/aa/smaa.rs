const AREA_BYTES: &[u8] = include_bytes!("area_tex.bin");
const SEARCH_BYTES: &[u8] = include_bytes!("search_tex.bin");
const EDGES_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Unorm;
const BLEND_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

struct Targets {
    size: [u32; 2],
    edges: wgpu::TextureView,
    blend: wgpu::TextureView,
}

pub struct Smaa {
    edges_layout: wgpu::BindGroupLayout,
    weights_layout: wgpu::BindGroupLayout,
    blend_layout: wgpu::BindGroupLayout,
    edges_pipeline: wgpu::RenderPipeline,
    weights_pipeline: wgpu::RenderPipeline,
    blend_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    area: wgpu::TextureView,
    search: wgpu::TextureView,
    targets: Option<Targets>,
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn lookup_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    format: wgpu::TextureFormat,
    size: [u32; 2],
    bytes_per_texel: u32,
    data: &[u8],
) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
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
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size[0] * bytes_per_texel),
            rows_per_image: Some(size[1]),
        },
        texture.size(),
    );
    texture.create_view(&Default::default())
}

fn render_target(
    device: &wgpu::Device,
    label: &str,
    format: wgpu::TextureFormat,
    size: [u32; 2],
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

fn bind_group(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::BindGroupLayout,
    resources: &[(u32, wgpu::BindingResource)],
) -> wgpu::BindGroup {
    let entries: Vec<_> = resources
        .iter()
        .map(|(binding, resource)| wgpu::BindGroupEntry {
            binding: *binding,
            resource: resource.clone(),
        })
        .collect();
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &entries,
    })
}

fn run_pass(
    encoder: &mut wgpu::CommandEncoder,
    label: &str,
    pipeline: &wgpu::RenderPipeline,
    group: &wgpu::BindGroup,
    target: &wgpu::TextureView,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.draw(0..3, 0..1);
}

impl Smaa {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Horizon SMAA"),
            source: wgpu::ShaderSource::Wgsl(include_str!("smaa.wgsl").into()),
        });
        let layout = |label: &str, entries: &[wgpu::BindGroupLayoutEntry]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries,
            })
        };
        let edges_layout = layout("Horizon SMAA edges", &[texture_entry(0)]);
        let weights_layout = layout(
            "Horizon SMAA weights",
            &[
                texture_entry(1),
                texture_entry(2),
                texture_entry(3),
                sampler_entry(5),
            ],
        );
        let blend_layout = layout(
            "Horizon SMAA blend",
            &[texture_entry(0), texture_entry(4), sampler_entry(5)],
        );
        let pipeline = |label: &str,
                        layout: &wgpu::BindGroupLayout,
                        entry: &str,
                        format: wgpu::TextureFormat| {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(layout)],
                ..Default::default()
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: crate::gfx::pipeline_cache::get(),
            })
        };
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Horizon SMAA"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            edges_pipeline: pipeline(
                "Horizon SMAA edges",
                &edges_layout,
                "fs_edges",
                EDGES_FORMAT,
            ),
            weights_pipeline: pipeline(
                "Horizon SMAA weights",
                &weights_layout,
                "fs_weights",
                BLEND_FORMAT,
            ),
            blend_pipeline: pipeline("Horizon SMAA blend", &blend_layout, "fs_blend", format),
            edges_layout,
            weights_layout,
            blend_layout,
            sampler,
            area: lookup_texture(
                device,
                queue,
                "Horizon SMAA area",
                wgpu::TextureFormat::Rg8Unorm,
                [160, 560],
                2,
                AREA_BYTES,
            ),
            search: lookup_texture(
                device,
                queue,
                "Horizon SMAA search",
                wgpu::TextureFormat::R8Unorm,
                [64, 16],
                1,
                SEARCH_BYTES,
            ),
            targets: None,
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
        if size[0] == 0 || size[1] == 0 {
            return;
        }
        if self.targets.as_ref().is_none_or(|t| t.size != size) {
            self.targets = Some(Targets {
                size,
                edges: render_target(device, "Horizon SMAA edges", EDGES_FORMAT, size),
                blend: render_target(device, "Horizon SMAA blend", BLEND_FORMAT, size),
            });
        }
        let Some(targets) = &self.targets else {
            return;
        };
        let edges_group = bind_group(
            device,
            "Horizon SMAA edges",
            &self.edges_layout,
            &[(0, wgpu::BindingResource::TextureView(input))],
        );
        let weights_group = bind_group(
            device,
            "Horizon SMAA weights",
            &self.weights_layout,
            &[
                (1, wgpu::BindingResource::TextureView(&targets.edges)),
                (2, wgpu::BindingResource::TextureView(&self.area)),
                (3, wgpu::BindingResource::TextureView(&self.search)),
                (5, wgpu::BindingResource::Sampler(&self.sampler)),
            ],
        );
        let blend_group = bind_group(
            device,
            "Horizon SMAA blend",
            &self.blend_layout,
            &[
                (0, wgpu::BindingResource::TextureView(input)),
                (4, wgpu::BindingResource::TextureView(&targets.blend)),
                (5, wgpu::BindingResource::Sampler(&self.sampler)),
            ],
        );
        run_pass(
            encoder,
            "Horizon SMAA edges",
            &self.edges_pipeline,
            &edges_group,
            &targets.edges,
        );
        run_pass(
            encoder,
            "Horizon SMAA weights",
            &self.weights_pipeline,
            &weights_group,
            &targets.blend,
        );
        run_pass(
            encoder,
            "Horizon SMAA blend",
            &self.blend_pipeline,
            &blend_group,
            output,
        );
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

    const SIDE: u32 = 64;

    fn edge_y(x: u32) -> u32 {
        20 + x / 3
    }

    fn source_pixels() -> Vec<u8> {
        let mut pixels = Vec::new();
        for y in 0..SIDE {
            for x in 0..SIDE {
                let v = if y > edge_y(x) { 255 } else { 0 };
                pixels.extend_from_slice(&[v, v, v, (x * 5 + y * 3) as u8 | 1]);
            }
        }
        pixels
    }

    fn run(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        smaa: &mut Smaa,
        format: wgpu::TextureFormat,
        pixels: &[u8],
    ) -> Vec<u8> {
        let make = |usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: SIDE,
                    height: SIDE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let input = make(wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST);
        let output = make(
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        );
        queue.write_texture(
            input.as_image_copy(),
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SIDE * 4),
                rows_per_image: Some(SIDE),
            },
            input.size(),
        );
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(SIDE * SIDE * 4),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        smaa.encode(
            device,
            &mut encoder,
            &input.create_view(&Default::default()),
            [SIDE, SIDE],
            &output.create_view(&Default::default()),
        );
        encoder.copy_texture_to_buffer(
            output.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(SIDE * 4),
                    rows_per_image: Some(SIDE),
                },
            },
            output.size(),
        );
        queue.submit([encoder.finish()]);
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        readback.slice(..).get_mapped_range().unwrap().to_vec()
    }

    #[test]
    fn lookup_tables_match_the_reference_arrays() {
        assert_eq!(AREA_BYTES.len(), 160 * 560 * 2);
        assert_eq!(SEARCH_BYTES.len(), 64 * 16);
        assert_eq!(SEARCH_BYTES[..6], [0xfe, 0xfe, 0x00, 0x7f, 0x7f, 0x00]);
        let row = 160 * 2;
        let first_nonzero = AREA_BYTES.iter().position(|&b| b != 0).unwrap();
        assert_eq!(first_nonzero, 33);
        assert_eq!(AREA_BYTES[33], 0x1f);
        assert_eq!(AREA_BYTES[..row][..32], [0; 32]);
    }

    #[test]
    fn staircase_edge_is_smoothed_and_flat_areas_are_untouched() {
        let Some((device, queue)) = test_device() else {
            return;
        };
        let pixels = source_pixels();
        for format in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Bgra8Unorm,
        ] {
            let mut smaa = Smaa::new(&device, &queue, format);
            for _ in 0..2 {
                let out = run(&device, &queue, &mut smaa, format, &pixels);
                let mut intermediate = 0;
                for y in 0..SIDE {
                    for x in 0..SIDE {
                        let i = ((y * SIDE + x) * 4) as usize;
                        assert_eq!(out[i + 3], pixels[i + 3], "alpha at {x},{y} in {format:?}");
                        let dist = (y as i32 - edge_y(x) as i32).abs();
                        if (1..=2).contains(&dist) || dist == 0 {
                            if out[i] != 0 && out[i] != 255 {
                                intermediate += 1;
                            }
                        } else {
                            assert_eq!(out[i..i + 3], pixels[i..i + 3], "flat {x},{y}");
                        }
                        assert!(
                            out[i] == out[i + 1] && out[i + 1] == out[i + 2],
                            "grey stays grey at {x},{y}"
                        );
                    }
                }
                assert!(intermediate >= 30, "{intermediate} smoothed pixels");
            }
        }
    }

    #[test]
    fn flat_image_is_unchanged() {
        let Some((device, queue)) = test_device() else {
            return;
        };
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut smaa = Smaa::new(&device, &queue, format);
        let pixels: Vec<u8> = (0..SIDE * SIDE)
            .flat_map(|i| [90, 140, 200, (i % 251) as u8])
            .collect();
        assert_eq!(run(&device, &queue, &mut smaa, format, &pixels), pixels);
    }
}
