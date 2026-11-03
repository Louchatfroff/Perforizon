use wgpu::util::DeviceExt;

const MID_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    scale: [f32; 2],
    limit: [i32; 2],
    sharpness: f32,
    pad: [f32; 3],
}

struct Mid {
    size: [u32; 2],
    view: wgpu::TextureView,
}

pub struct Fsr {
    layout: wgpu::BindGroupLayout,
    easu_mid: wgpu::RenderPipeline,
    easu_out: wgpu::RenderPipeline,
    rcas: wgpu::RenderPipeline,
    mid: Option<Mid>,
}

impl Fsr {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Horizon FSR"),
            source: wgpu::ShaderSource::Wgsl(include_str!("fsr.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Horizon FSR bindings"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Horizon FSR"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let pipeline = |entry: &str, format| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Horizon FSR"),
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
        Self {
            easu_mid: pipeline("fs_easu", MID_FORMAT),
            easu_out: pipeline("fs_easu", format),
            rcas: pipeline("fs_rcas", format),
            layout,
            mid: None,
        }
    }

    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        input: &wgpu::TextureView,
        in_size: [u32; 2],
        output: &wgpu::TextureView,
        out_size: [u32; 2],
        sharpness: f32,
    ) {
        if in_size.contains(&0) || out_size.contains(&0) {
            return;
        }
        let easu = Params {
            scale: [
                in_size[0] as f32 / out_size[0] as f32,
                in_size[1] as f32 / out_size[1] as f32,
            ],
            limit: [in_size[0] as i32 - 1, in_size[1] as i32 - 1],
            sharpness: 0.0,
            pad: [0.0; 3],
        };
        if sharpness.is_nan() || sharpness <= 0.0 {
            self.pass(device, encoder, &self.easu_out, input, &easu, output);
            return;
        }
        if self.mid.as_ref().is_none_or(|mid| mid.size != out_size) {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Horizon FSR upscaled"),
                size: wgpu::Extent3d {
                    width: out_size[0],
                    height: out_size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: MID_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.mid = Some(Mid {
                size: out_size,
                view: texture.create_view(&Default::default()),
            });
        }
        let Some(mid) = &self.mid else { return };
        let rcas = Params {
            scale: [1.0, 1.0],
            limit: [out_size[0] as i32 - 1, out_size[1] as i32 - 1],
            sharpness: 2.0 - sharpness.min(2.0),
            pad: [0.0; 3],
        };
        self.pass(device, encoder, &self.easu_mid, input, &easu, &mid.view);
        self.pass(device, encoder, &self.rcas, &mid.view, &rcas, output);
    }

    fn pass(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::RenderPipeline,
        source: &wgpu::TextureView,
        params: &Params,
        target: &wgpu::TextureView,
    ) {
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Horizon FSR"),
            contents: bytemuck::bytes_of(params),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Horizon FSR"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        let attachments = [Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Horizon FSR"),
            color_attachments: &attachments,
            ..Default::default()
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IN: u32 = 32;
    const OUT: u32 = 64;
    const SHARPNESS: [f32; 4] = [0.0, 0.25, 1.0, 2.0];

    fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle().with_env());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
        pollster::block_on(adapter.request_device(&Default::default())).ok()
    }

    fn run(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        input: &[[u8; 4]],
        sharpness: f32,
    ) -> Vec<[f32; 4]> {
        let extent = |size| wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        };
        let texture = |size, format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: extent(size),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let source = texture(
            IN,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        queue.write_texture(
            source.as_image_copy(),
            bytemuck::cast_slice(input),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(IN * 4),
                rows_per_image: Some(IN),
            },
            extent(IN),
        );
        let target = texture(
            OUT,
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (OUT * OUT * 8) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut fsr = Fsr::new(device, wgpu::TextureFormat::Rgba16Float);
        let mut encoder = device.create_command_encoder(&Default::default());
        fsr.encode(
            device,
            &mut encoder,
            &source.create_view(&Default::default()),
            [IN, IN],
            &target.create_view(&Default::default()),
            [OUT, OUT],
            sharpness,
        );
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(OUT * 8),
                    rows_per_image: Some(OUT),
                },
            },
            extent(OUT),
        );
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback.map_async(wgpu::MapMode::Read, .., move |r| tx.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let mapped = readback.get_mapped_range(..).unwrap();
        let halves: &[u16] = bytemuck::cast_slice(&mapped);
        halves
            .chunks(4)
            .map(|p| std::array::from_fn(|i| half::f16::from_bits(p[i]).to_f32()))
            .collect()
    }

    fn image(f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<[u8; 4]> {
        (0..IN * IN).map(|i| f(i % IN, i / IN)).collect()
    }

    fn alpha(x: u32, y: u32) -> u8 {
        ((x * 7 + y * 13) % 200 + 20) as u8
    }

    #[test]
    fn shader_validates() {
        let module = naga::front::wgsl::parse_str(include_str!("fsr.wgsl")).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap();
    }

    #[test]
    fn flat_colour_stays_flat() {
        let Some((device, queue)) = test_device() else {
            return;
        };
        let colour = [77u8, 153, 230];
        let input = image(|x, y| [colour[0], colour[1], colour[2], alpha(x, y)]);
        for sharpness in SHARPNESS {
            let out = run(&device, &queue, &input, sharpness);
            for (i, p) in out.iter().enumerate() {
                for c in 0..3 {
                    let want = colour[c] as f32 / 255.0;
                    assert!(
                        (p[c] - want).abs() <= 1.0 / 255.0,
                        "sharpness {sharpness} pixel {i} channel {c}: {} vs {want}",
                        p[c]
                    );
                }
            }
        }
    }

    #[test]
    fn black_and_white_flats_stay_flat() {
        let Some((device, queue)) = test_device() else {
            return;
        };
        for v in [0u8, 255] {
            let input = image(|_, _| [v, v, v, 255]);
            for sharpness in SHARPNESS {
                let out = run(&device, &queue, &input, sharpness);
                let want = v as f32 / 255.0;
                assert!(
                    out.iter()
                        .all(|p| (0..3).all(|c| (p[c] - want).abs() <= 1.0 / 255.0))
                );
            }
        }
    }

    #[test]
    fn alpha_comes_from_the_nearest_input_texel() {
        let Some((device, queue)) = test_device() else {
            return;
        };
        let input = image(|x, y| {
            let v = if (x + y) % 2 == 0 { 255 } else { 0 };
            [v, 255 - v, v / 2, alpha(x, y)]
        });
        for sharpness in SHARPNESS {
            let out = run(&device, &queue, &input, sharpness);
            for y in 0..OUT {
                for x in 0..OUT {
                    let got = (out[(y * OUT + x) as usize][3] * 255.0).round() as u8;
                    assert_eq!(got, alpha(x / 2, y / 2), "sharpness {sharpness} at {x},{y}");
                }
            }
        }
    }

    #[test]
    fn diagonal_edge_stays_in_range_and_keeps_its_sides() {
        let Some((device, queue)) = test_device() else {
            return;
        };
        let input = image(|x, y| if x + y < IN { [255; 4] } else { [0, 0, 0, 255] });
        for sharpness in SHARPNESS {
            let out = run(&device, &queue, &input, sharpness);
            assert!(out.iter().flatten().all(|v| (0.0..=1.0).contains(v)));
            let at = |x: u32, y: u32| out[(y * OUT + x) as usize][0];
            assert!(at(4, 4) > 0.99, "sharpness {sharpness}");
            assert!(at(OUT - 5, OUT - 5) < 0.01, "sharpness {sharpness}");
            let soft = (0..OUT)
                .map(|x| at(x, OUT - 1 - x))
                .filter(|v| (0.05..0.95).contains(v))
                .count();
            assert!(soft > 0, "sharpness {sharpness}: no intermediate pixels");
        }
    }

    #[test]
    fn checker_stays_in_range() {
        let Some((device, queue)) = test_device() else {
            return;
        };
        let input = image(|x, y| {
            let v = if (x / 4 + y / 4) % 2 == 0 { 255 } else { 0 };
            [v, v, v, 255]
        });
        for sharpness in SHARPNESS {
            let out = run(&device, &queue, &input, sharpness);
            assert!(out.iter().flatten().all(|v| (0.0..=1.0).contains(v)));
            assert!(out[(2 * OUT + 2) as usize][0] > 0.99);
            assert!(out[(2 * OUT + 10) as usize][0] < 0.01);
        }
    }

    #[test]
    fn rcas_sharpens_an_edge() {
        let Some((device, queue)) = test_device() else {
            return;
        };
        let input = image(|x, _| {
            let v = if x < IN / 2 { 80 } else { 170 };
            [v, v, v, 255]
        });
        let soft = run(&device, &queue, &input, 0.0);
        let sharp = run(&device, &queue, &input, 0.25);
        let row = (OUT / 2 * OUT) as usize;
        let mid = (OUT / 2) as usize;
        let contrast = |o: &[[f32; 4]]| o[row + mid + 1][0] - o[row + mid - 2][0];
        assert!(contrast(&sharp) >= contrast(&soft));
        assert!(sharp.iter().flatten().all(|v| (0.0..=1.0).contains(v)));
    }
}
