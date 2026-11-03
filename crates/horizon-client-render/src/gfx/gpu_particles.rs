use super::{Vertex, upload::Uploads};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct ParticleInstance {
    pub center: [f32; 3],
    pub size: f32,
    pub uv: [f32; 4],
    pub roll: f32,
    pub color: u32,
    pub light: u32,
    pub layer: u32,
}

const CORNERS: [([f32; 2], [bool; 2]); 4] = [
    ([1.0, -1.0], [true, true]),
    ([1.0, 1.0], [true, false]),
    ([-1.0, 1.0], [false, false]),
    ([-1.0, -1.0], [false, true]),
];

impl ParticleInstance {
    pub fn vertices(&self, right: [f32; 3], up: [f32; 3]) -> [Vertex; 4] {
        let (sin, cos) = self.roll.sin_cos();
        CORNERS.map(|([nx, ny], [far_u, far_v])| {
            let x = (nx * cos - ny * sin) * self.size;
            let y = (nx * sin + ny * cos) * self.size;
            Vertex {
                normal: [0; 4],
                pos: std::array::from_fn(|i| self.center[i] + right[i] * x + up[i] * y),
                uv: [
                    if far_u { self.uv[2] } else { self.uv[0] },
                    if far_v { self.uv[3] } else { self.uv[1] },
                ],
                layer: self.layer,
                color: self.color,
                light: self.light,
                glint_uv: [0.0, 0.0],
            }
        })
    }
}

#[derive(Default)]
pub struct ParticleInstances {
    pub opaque: Vec<ParticleInstance>,
    pub translucent: Vec<ParticleInstance>,
    pub right: [f32; 3],
    pub up: [f32; 3],
}

pub static NONE: ParticleInstances = ParticleInstances {
    opaque: Vec::new(),
    translucent: Vec::new(),
    right: [0.0; 3],
    up: [0.0; 3],
};

impl ParticleInstances {
    pub fn clear(&mut self) {
        self.opaque.clear();
        self.translucent.clear();
    }
    pub fn len(&self) -> usize {
        self.opaque.len() + self.translucent.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

const AXES_BYTES: u64 = 80;

pub struct GpuParticles {
    buffer: wgpu::Buffer,
    capacity: u64,
    axes: wgpu::Buffer,
    group: wgpu::BindGroup,
    opaque: u32,
    translucent: u32,
}

pub fn instance_attributes() -> [wgpu::VertexAttribute; 6] {
    wgpu::vertex_attr_array![0=>Float32x4,1=>Float32x4,2=>Float32,3=>Uint32,4=>Uint32,5=>Uint32]
}

fn instance_buffer(device: &wgpu::Device, capacity: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("GPU particle instances"),
        size: capacity * std::mem::size_of::<ParticleInstance>() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

impl GpuParticles {
    pub fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let axes = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GPU particle axes"),
            size: AXES_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GPU particle resources"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: axes.as_entire_binding(),
            }],
        });
        let capacity = 256;
        Self {
            buffer: instance_buffer(device, capacity),
            capacity,
            axes,
            group,
            opaque: 0,
            translucent: 0,
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        uploads: &mut Uploads,
        encoder: &mut wgpu::CommandEncoder,
        input: &ParticleInstances,
    ) {
        self.opaque = input.opaque.len() as u32;
        self.translucent = input.translucent.len() as u32;
        let total = input.len() as u64;
        if total == 0 {
            return;
        }
        if total > self.capacity {
            self.capacity = total.next_power_of_two();
            self.buffer = instance_buffer(device, self.capacity);
        }
        let size = std::mem::size_of::<ParticleInstance>() as u64;
        uploads.buffer(
            encoder,
            &self.buffer,
            0,
            bytemuck::cast_slice(&input.opaque),
        );
        uploads.buffer(
            encoder,
            &self.buffer,
            input.opaque.len() as u64 * size,
            bytemuck::cast_slice(&input.translucent),
        );
        let mut axes = [[0.0f32; 4]; 5];
        axes[0] = [input.right[0], input.right[1], input.right[2], 0.0];
        axes[1] = [input.up[0], input.up[1], input.up[2], 0.0];
        uploads.buffer(encoder, &self.axes, 0, bytemuck::cast_slice(&axes));
    }

    pub fn count(&self, translucent: bool) -> u32 {
        if translucent {
            self.translucent
        } else {
            self.opaque
        }
    }

    pub fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        pipeline: &wgpu::RenderPipeline,
        entity_group: &wgpu::BindGroup,
        indices: &wgpu::Buffer,
        translucent: bool,
    ) {
        let count = self.count(translucent);
        if count == 0 {
            return;
        }
        let first = if translucent { self.opaque } else { 0 };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, entity_group, &[]);
        pass.set_bind_group(1, &self.group, &[]);
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        pass.draw_indexed(0..6, 0, first..first + count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_instance_is_48_bytes_and_the_attributes_cover_it() {
        assert_eq!(std::mem::size_of::<ParticleInstance>(), 48);
        let attributes = instance_attributes();
        assert_eq!(attributes.last().unwrap().offset, 44);
    }

    #[test]
    fn corners_follow_the_cpu_quad_order_without_roll() {
        let instance = ParticleInstance {
            center: [1.0, 2.0, 3.0],
            size: 0.5,
            uv: [0.25, 0.5, 0.75, 1.0],
            roll: 0.0,
            color: 7,
            light: 9,
            layer: 15,
        };
        let v = instance.vertices([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert_eq!(v[0].pos, [1.5, 1.5, 3.0]);
        assert_eq!(v[0].uv, [0.75, 1.0]);
        assert_eq!(v[1].pos, [1.5, 2.5, 3.0]);
        assert_eq!(v[1].uv, [0.75, 0.5]);
        assert_eq!(v[2].pos, [0.5, 2.5, 3.0]);
        assert_eq!(v[2].uv, [0.25, 0.5]);
        assert_eq!(v[3].pos, [0.5, 1.5, 3.0]);
        assert_eq!(v[3].uv, [0.25, 1.0]);
        assert!(
            v.iter()
                .all(|v| v.layer == 15 && v.color == 7 && v.light == 9)
        );
    }

    #[test]
    fn a_quarter_turn_roll_swings_the_first_corner_to_the_second() {
        let instance = ParticleInstance {
            center: [0.0; 3],
            size: 2.0,
            uv: [0.0; 4],
            roll: std::f32::consts::FRAC_PI_2,
            color: 0,
            light: 0,
            layer: 0,
        };
        let v = instance.vertices([0.0, 0.0, 1.0], [0.0, 1.0, 0.0]);
        let expected = [
            [0.0, 2.0, 2.0],
            [0.0, 2.0, -2.0],
            [0.0, -2.0, -2.0],
            [0.0, -2.0, 2.0],
        ];
        for (vertex, want) in v.iter().zip(expected) {
            for axis in 0..3 {
                assert!(
                    (vertex.pos[axis] - want[axis]).abs() < 1e-5,
                    "{vertex:?} {want:?}"
                );
            }
        }
    }
}
