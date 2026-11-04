use super::cull::Cull;
use glam::{DMat4, DVec3, Mat4};

const MAX_LEVELS: usize = 12;
const LAST_SIZE: u32 = 8;
const MAX_JUMP: f64 = 16.0;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameData {
    view_proj: [[f32; 4]; 4],
    prev_inverse: [[f32; 4]; 4],
    delta: [f32; 4],
    cells: [u32; 4],
    counts: [u32; 4],
    levels: [[u32; 4]; MAX_LEVELS],
}

pub struct Inputs<'a> {
    pub size: [u32; 2],
    pub depth: &'a wgpu::TextureView,
    pub view_proj: Mat4,
    pub camera: DVec3,
    pub base: u32,
    pub indirect: &'a wgpu::Buffer,
    pub origins: &'a wgpu::Buffer,
    pub globals: &'a wgpu::Buffer,
}

struct Targets {
    size: [u32; 2],
    levels: Vec<[u32; 4]>,
    splat: wgpu::Buffer,
    pyramid: wgpu::Buffer,
    prev: wgpu::Buffer,
    steps: Vec<wgpu::Buffer>,
}

struct Groups {
    key: [wgpu::Buffer; 6],
    depth: wgpu::TextureView,
    snapshot: wgpu::BindGroup,
    reproject: wgpu::BindGroup,
    fill: wgpu::BindGroup,
    reduce: Vec<wgpu::BindGroup>,
    cull: wgpu::BindGroup,
    rescue: wgpu::BindGroup,
}

pub struct Hiz {
    snapshot: wgpu::ComputePipeline,
    reproject: wgpu::ComputePipeline,
    fill: wgpu::ComputePipeline,
    reduce: wgpu::ComputePipeline,
    cull: wgpu::ComputePipeline,
    frame: wgpu::Buffer,
    targets: Option<Targets>,
    tests: wgpu::Buffer,
    rescue: wgpu::Buffer,
    rescue_results: wgpu::Buffer,
    args: wgpu::Buffer,
    capacity: u64,
    groups: Option<Groups>,
    list: Vec<[u32; 4]>,
    prev: Option<(Mat4, DVec3)>,
    pending: Option<(Mat4, DVec3)>,
    snapshot_done: std::cell::Cell<bool>,
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
        usage,
        mapped_at_creation: false,
    })
}

const STORAGE: wgpu::BufferUsages = wgpu::BufferUsages::STORAGE;
const DST: wgpu::BufferUsages = wgpu::BufferUsages::COPY_DST;

pub fn levels(cells: [u32; 2]) -> Vec<[u32; 4]> {
    let (mut w, mut h) = (cells[0].max(1), cells[1].max(1));
    let mut offset = 0;
    let mut out = Vec::new();
    loop {
        out.push([offset, w, h, 0]);
        offset += w * h;
        if out.len() == MAX_LEVELS || (w <= LAST_SIZE && h <= LAST_SIZE) {
            return out;
        }
        w = w.div_ceil(2);
        h = h.div_ceil(2);
    }
}

fn dispatch_groups(n: u32) -> u32 {
    n.div_ceil(8)
}

impl Hiz {
    pub fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Hi-Z"),
            source: wgpu::ShaderSource::Wgsl(include_str!("hiz.wgsl").into()),
        });
        let pipeline = |entry: &'static str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: None,
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: crate::gfx::pipeline_cache::get(),
            })
        };
        Self {
            snapshot: pipeline("snapshot"),
            reproject: pipeline("reproject"),
            fill: pipeline("fill"),
            reduce: pipeline("reduce"),
            cull: pipeline("cull"),
            frame: buffer(
                device,
                "Hi-Z frame",
                std::mem::size_of::<FrameData>() as u64,
                wgpu::BufferUsages::UNIFORM | DST,
            ),
            targets: None,
            tests: buffer(device, "Hi-Z tests", 16, STORAGE | DST),
            rescue: buffer(device, "Hi-Z rescue", 48, STORAGE),
            rescue_results: buffer(device, "Hi-Z rescue results", 4, STORAGE),
            args: buffer(
                device,
                "Hi-Z rescue arguments",
                16,
                STORAGE | DST | wgpu::BufferUsages::INDIRECT,
            ),
            capacity: 1,
            groups: None,
            list: Vec::new(),
            prev: None,
            pending: None,
            snapshot_done: std::cell::Cell::new(false),
        }
    }

    pub fn begin(&mut self) {
        self.list.clear();
    }

    pub fn push(&mut self, index: u32, first: u32, draws: u32, packed_box: u32) {
        if draws > 0 {
            self.list.push([index, first, draws, packed_box]);
        }
    }

    fn targets(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let cells = [size[0].div_ceil(2), size[1].div_ceil(2)];
        let levels = levels(cells);
        let total: u32 = levels.iter().map(|l| l[1] * l[2]).sum();
        let steps = (0..levels.len().saturating_sub(1))
            .map(|i| {
                let b = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Hi-Z step"),
                    size: 16,
                    usage: wgpu::BufferUsages::UNIFORM,
                    mapped_at_creation: true,
                });
                b.slice(..)
                    .get_mapped_range_mut()
                    .expect("mapped at creation")
                    .copy_from_slice(bytemuck::bytes_of(&[i as u32, 0, 0, 0]));
                b.unmap();
                b
            })
            .collect();
        self.targets = Some(Targets {
            size,
            levels,
            splat: buffer(
                device,
                "Hi-Z splat",
                u64::from(cells[0] * cells[1]) * 4,
                STORAGE | DST,
            ),
            pyramid: buffer(device, "Hi-Z pyramid", u64::from(total) * 4, STORAGE),
            prev: buffer(
                device,
                "Hi-Z previous depth",
                u64::from(cells[0] * cells[1]) * 4,
                STORAGE,
            ),
            steps,
        });
        self.groups = None;
        self.prev = None;
    }

    pub fn run(
        &mut self,
        device: &wgpu::Device,
        uploads: &mut super::upload::Uploads,
        encoder: &mut wgpu::CommandEncoder,
        cull: &Cull,
        inputs: Inputs<'_>,
    ) {
        self.targets(device, inputs.size);
        let n = self.list.len() as u64;
        if n > self.capacity {
            self.capacity = n.next_power_of_two();
            self.tests = buffer(device, "Hi-Z tests", self.capacity * 16, STORAGE | DST);
            self.rescue = buffer(device, "Hi-Z rescue", self.capacity * 48, STORAGE);
            self.rescue_results = buffer(device, "Hi-Z rescue results", self.capacity * 4, STORAGE);
            self.groups = None;
        }
        let key = [
            inputs.indirect.clone(),
            inputs.origins.clone(),
            inputs.globals.clone(),
            cull.phase2.clone(),
            self.tests.clone(),
            self.rescue.clone(),
        ];
        if self
            .groups
            .as_ref()
            .is_none_or(|g| g.key != key || g.depth != *inputs.depth)
        {
            self.groups = Some(self.make_groups(device, cull, &inputs, key));
        }
        let targets = self.targets.as_ref().expect("targets were made");
        let jump = self
            .prev
            .map(|(_, camera)| (camera - inputs.camera).length());
        let usable = self.prev.filter(|_| jump.is_some_and(|j| j < MAX_JUMP));
        let tests = if usable.is_some() { self.list.len() } else { 0 };
        let (prev_inverse, delta) = match usable {
            Some((vp, camera)) => (
                DMat4::from_cols_array(&vp.to_cols_array().map(f64::from))
                    .inverse()
                    .as_mat4(),
                (camera - inputs.camera).as_vec3(),
            ),
            None => (Mat4::IDENTITY, glam::Vec3::ZERO),
        };
        let cells = [inputs.size[0].div_ceil(2), inputs.size[1].div_ceil(2)];
        let mut levels = [[0u32; 4]; MAX_LEVELS];
        levels[..targets.levels.len()].copy_from_slice(&targets.levels);
        let data = FrameData {
            view_proj: inputs.view_proj.to_cols_array_2d(),
            prev_inverse: prev_inverse.to_cols_array_2d(),
            delta: delta.extend(0.0).to_array(),
            cells: [cells[0], cells[1], inputs.size[0], inputs.size[1]],
            counts: [tests as u32, targets.levels.len() as u32, inputs.base, 0],
            levels,
        };
        uploads.buffer(encoder, &self.frame, 0, bytemuck::bytes_of(&data));
        uploads.buffer(
            encoder,
            &self.args,
            0,
            bytemuck::cast_slice(&[36u32, 0, 0, 0]),
        );
        self.pending = Some((inputs.view_proj, inputs.camera));
        self.snapshot_done.set(false);
        if tests == 0 {
            return;
        }
        uploads.buffer(encoder, &self.tests, 0, bytemuck::cast_slice(&self.list));
        let groups = self.groups.as_ref().expect("groups were made");
        encoder.clear_buffer(&targets.splat, 0, None);
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Hi-Z"),
            timestamp_writes: None,
        });
        let (gx, gy) = (dispatch_groups(cells[0]), dispatch_groups(cells[1]));
        pass.set_pipeline(&self.reproject);
        pass.set_bind_group(0, &groups.reproject, &[]);
        pass.dispatch_workgroups(gx, gy, 1);
        pass.set_pipeline(&self.fill);
        pass.set_bind_group(0, &groups.fill, &[]);
        pass.dispatch_workgroups(gx, gy, 1);
        pass.set_pipeline(&self.reduce);
        for (i, group) in groups.reduce.iter().enumerate() {
            let to = targets.levels[i + 1];
            pass.set_bind_group(0, group, &[]);
            pass.dispatch_workgroups(dispatch_groups(to[1]), dispatch_groups(to[2]), 1);
        }
        pass.set_pipeline(&self.cull);
        pass.set_bind_group(0, &groups.cull, &[]);
        pass.dispatch_workgroups((tests as u32).div_ceil(64), 1, 1);
    }

    pub fn snapshot(&self, encoder: &mut wgpu::CommandEncoder) {
        let (Some(groups), Some(targets)) = (&self.groups, &self.targets) else {
            return;
        };
        let cells = [targets.size[0].div_ceil(2), targets.size[1].div_ceil(2)];
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Hi-Z snapshot"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.snapshot);
        pass.set_bind_group(0, &groups.snapshot, &[]);
        pass.dispatch_workgroups(dispatch_groups(cells[0]), dispatch_groups(cells[1]), 1);
        self.snapshot_done.set(true);
    }

    pub fn test(&self, pass: &mut wgpu::RenderPass<'_>, cull: &Cull) {
        if let Some(groups) = &self.groups {
            cull.test_indirect(pass, &groups.rescue, &self.args);
        }
    }

    pub fn end_frame(&mut self) {
        self.prev = if self.snapshot_done.replace(false) {
            self.pending.take()
        } else {
            self.pending = None;
            None
        };
    }

    fn make_groups(
        &self,
        device: &wgpu::Device,
        cull: &Cull,
        inputs: &Inputs<'_>,
        key: [wgpu::Buffer; 6],
    ) -> Groups {
        let targets = self.targets.as_ref().expect("targets were made");
        fn entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry {
                binding,
                resource: buffer.as_entire_binding(),
            }
        }
        let group = |pipeline: &wgpu::ComputePipeline, entries: &[wgpu::BindGroupEntry<'_>]| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Hi-Z"),
                layout: &pipeline.get_bind_group_layout(0),
                entries,
            })
        };
        let frame = entry(0, &self.frame);
        Groups {
            snapshot: group(
                &self.snapshot,
                &[
                    frame.clone(),
                    entry(3, &targets.prev),
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::TextureView(inputs.depth),
                    },
                ],
            ),
            reproject: group(
                &self.reproject,
                &[
                    frame.clone(),
                    entry(1, &targets.splat),
                    entry(3, &targets.prev),
                ],
            ),
            fill: group(
                &self.fill,
                &[
                    frame.clone(),
                    entry(1, &targets.splat),
                    entry(2, &targets.pyramid),
                ],
            ),
            reduce: targets
                .steps
                .iter()
                .map(|step| {
                    group(
                        &self.reduce,
                        &[frame.clone(), entry(2, &targets.pyramid), entry(4, step)],
                    )
                })
                .collect(),
            cull: group(
                &self.cull,
                &[
                    frame.clone(),
                    entry(2, &targets.pyramid),
                    entry(8, &self.tests),
                    entry(10, inputs.indirect),
                    entry(11, &self.rescue),
                    entry(12, &self.args),
                    entry(13, inputs.origins),
                ],
            ),
            rescue: cull.rescue_group(
                device,
                &self.rescue,
                &self.rescue_results,
                inputs.origins,
                inputs.globals,
            ),
            depth: inputs.depth.clone(),
            key,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_halve_with_rounding_up_until_eight_cells() {
        let l = levels([960, 540]);
        let dims: Vec<[u32; 2]> = l.iter().map(|l| [l[1], l[2]]).collect();
        assert_eq!(
            dims,
            [
                [960, 540],
                [480, 270],
                [240, 135],
                [120, 68],
                [60, 34],
                [30, 17],
                [15, 9],
                [8, 5]
            ]
        );
        assert_eq!(l[0][0], 0);
        assert_eq!(l[1][0], 960 * 540);
        assert_eq!(l[2][0], 960 * 540 + 480 * 270);
    }

    #[test]
    fn tiny_targets_have_one_level_and_big_ones_stop_at_the_cap() {
        assert_eq!(levels([4, 3]).len(), 1);
        assert_eq!(levels([1, 1]), vec![[0, 1, 1, 0]]);
        assert!(levels([8192, 8192]).len() <= MAX_LEVELS);
    }

    #[test]
    fn shader_validates() {
        let module = naga::front::wgsl::parse_str(include_str!("hiz.wgsl")).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{e:?}"));
    }
}
