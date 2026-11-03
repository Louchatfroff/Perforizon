use glam::{Vec3, Vec4};

const STORAGE: wgpu::BufferUsages = wgpu::BufferUsages::STORAGE;
const DST: wgpu::BufferUsages = wgpu::BufferUsages::COPY_DST;

pub const EVERYTHING: [Vec3; 2] = [Vec3::splat(-1.0e9), Vec3::splat(1.0e9)];

pub struct ShadowCull {
    pipeline: wgpu::ComputePipeline,
    planes: wgpu::Buffer,
    range: wgpu::Buffer,
    boxes: wgpu::Buffer,
    capacity: u64,
    staging: Vec<[f32; 4]>,
}

impl ShadowCull {
    pub fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow cull"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shadow_cull.wgsl").into()),
        });
        let uniform = |label, size| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::UNIFORM | DST,
                mapped_at_creation: false,
            })
        };
        Self {
            pipeline: device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Shadow cull"),
                layout: None,
                module: &module,
                entry_point: Some("cull"),
                compilation_options: Default::default(),
                cache: crate::gfx::pipeline_cache::get(),
            }),
            planes: uniform("Shadow cull planes", 96),
            range: uniform("Shadow cull range", 16),
            boxes: Self::boxes_buffer(device, 1),
            capacity: 1,
            staging: Vec::new(),
        }
    }

    fn boxes_buffer(device: &wgpu::Device, draws: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow cull boxes"),
            size: draws * 32,
            usage: STORAGE | DST,
            mapped_at_creation: false,
        })
    }

    pub fn begin(&mut self) {
        self.staging.clear();
    }

    pub fn push(&mut self, [lo, hi]: [Vec3; 2]) {
        self.staging.push(lo.extend(0.0).to_array());
        self.staging.push(hi.extend(0.0).to_array());
    }

    pub fn run(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        draws: &wgpu::Buffer,
        planes: &[Vec4; 6],
        first: u32,
    ) {
        let count = (self.staging.len() / 2) as u32;
        debug_assert!(
            (u64::from(first) + u64::from(count)) * 20 <= draws.size(),
            "shadow cull boxes outnumber the draws"
        );
        if count == 0 {
            return;
        }
        if u64::from(count) > self.capacity {
            self.capacity = u64::from(count).next_power_of_two();
            self.boxes = Self::boxes_buffer(device, self.capacity);
        }
        queue.write_buffer(&self.boxes, 0, bytemuck::cast_slice(&self.staging));
        queue.write_buffer(
            &self.planes,
            0,
            bytemuck::cast_slice(&planes.map(|p| p.to_array())),
        );
        queue.write_buffer(&self.range, 0, bytemuck::cast_slice(&[first, count, 0, 0]));
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow cull"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.boxes.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: draws.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.planes.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.range.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Shadow cull"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(count.div_ceil(64), 1, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Mat4;

    fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
        pollster::block_on(adapter.request_device(&Default::default())).ok()
    }

    fn read(device: &wgpu::Device, queue: &wgpu::Queue, buffer: &wgpu::Buffer, n: u64) -> Vec<u32> {
        let out = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: n * 4,
            usage: wgpu::BufferUsages::MAP_READ | DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(buffer, 0, &out, 0, n * 4);
        queue.submit([encoder.finish()]);
        out.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let data = out.slice(..).get_mapped_range().unwrap();
        bytemuck::cast_slice(&data).to_vec()
    }

    #[test]
    fn instance_counts_drop_exactly_where_the_box_is_outside_the_light_box() {
        let Some((device, queue)) = device() else {
            return;
        };
        let light = Mat4::orthographic_rh(-50.0, 50.0, -50.0, 50.0, 0.0, 200.0);
        let planes = super::super::frustum(light);
        let boxes = [
            ([-8.0, -8.0, -108.0], [8.0, 8.0, -92.0], true),
            ([60.0, -8.0, -108.0], [76.0, 8.0, -92.0], false),
            ([40.0, 40.0, -108.0], [56.0, 56.0, -92.0], true),
            ([-8.0, -8.0, 4.0], [8.0, 8.0, 20.0], false),
            ([-8.0, -8.0, -230.0], [8.0, 8.0, -210.0], false),
            ([-1.0e9; 3], [1.0e9; 3], true),
        ];
        let mut cull = ShadowCull::new(&device);
        cull.begin();
        for (lo, hi, _) in boxes {
            cull.push([Vec3::from(lo), Vec3::from(hi)]);
        }
        let draws: Vec<u32> = (0..2)
            .flat_map(|_| [0u32; 5])
            .chain((0..boxes.len()).flat_map(|i| [6, 1, 0, 0, i as u32]))
            .collect();
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: draws.len() as u64 * 4,
            usage: STORAGE | DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&draws));
        let mut encoder = device.create_command_encoder(&Default::default());
        cull.run(&device, &queue, &mut encoder, &buffer, &planes, 2);
        queue.submit([encoder.finish()]);
        let out = read(&device, &queue, &buffer, draws.len() as u64);
        assert!(out[..10].iter().all(|&w| w == 0));
        for (i, (_, _, kept)) in boxes.iter().enumerate() {
            let draw = &out[(2 + i) * 5..(3 + i) * 5];
            assert_eq!(draw[1], u32::from(*kept), "box {i}");
            assert_eq!([draw[0], draw[4]], [6, i as u32], "box {i}");
        }
    }
}
