use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const CAPACITY: u32 = 64;
const WINDOW: usize = 1024;
const STATS_AT: u64 = wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
const STATS_BYTES: u64 = 32;

enum Stage {
    Idle,
    Recording,
    Running(Arc<AtomicBool>),
    Reading(Arc<AtomicBool>, u64),
}

pub struct GpuPasses {
    queries: wgpu::QuerySet,
    stats: Option<wgpu::QuerySet>,
    stats_open: Cell<bool>,
    stats_used: Cell<bool>,
    stats_samples: [VecDeque<f32>; 4],
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    period_ns: f32,
    stage: Stage,
    recording: Cell<bool>,
    names: RefCell<Vec<&'static str>>,
    samples: Vec<(&'static str, VecDeque<f32>)>,
    totals: VecDeque<f32>,
}

impl GpuPasses {
    pub fn enabled() -> bool {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(|| std::env::var_os("HORIZON_GPU_PASSES").is_some())
    }

    pub fn features(adapter: &wgpu::Adapter) -> wgpu::Features {
        adapter.features()
            & (wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES
                | wgpu::Features::PIPELINE_STATISTICS_QUERY)
    }

    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        let needs = wgpu::Features::TIMESTAMP_QUERY
            | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS
            | wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES;
        if !device.features().contains(needs) {
            return None;
        }
        let size = STATS_AT + STATS_BYTES;
        let buffer = |label, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        Some(Self {
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("GPU pass stamps"),
                ty: wgpu::QueryType::Timestamp,
                count: CAPACITY,
            }),
            stats: device
                .features()
                .contains(wgpu::Features::PIPELINE_STATISTICS_QUERY)
                .then(|| {
                    device.create_query_set(&wgpu::QuerySetDescriptor {
                        label: Some("GPU world statistics"),
                        ty: wgpu::QueryType::PipelineStatistics(
                            wgpu::PipelineStatisticsTypes::VERTEX_SHADER_INVOCATIONS
                                | wgpu::PipelineStatisticsTypes::CLIPPER_INVOCATIONS
                                | wgpu::PipelineStatisticsTypes::CLIPPER_PRIMITIVES_OUT
                                | wgpu::PipelineStatisticsTypes::FRAGMENT_SHADER_INVOCATIONS,
                        ),
                        count: 1,
                    })
                }),
            stats_open: Cell::new(false),
            stats_used: Cell::new(false),
            stats_samples: Default::default(),
            resolve: buffer(
                "GPU pass stamps resolve",
                wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            ),
            read: buffer(
                "GPU pass stamps read",
                wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            ),
            period_ns: queue.get_timestamp_period(),
            stage: Stage::Idle,
            recording: Cell::new(false),
            names: RefCell::new(Vec::new()),
            samples: Vec::new(),
            totals: VecDeque::new(),
        })
    }

    pub fn begin(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        match &self.stage {
            Stage::Recording => self.stage = Stage::Idle,
            Stage::Running(done) if done.load(Ordering::Acquire) => {
                let count = self.names.borrow().len() as u32;
                let bytes = u64::from(count) * 8;
                let mut encoder = device.create_command_encoder(&Default::default());
                encoder.resolve_query_set(&self.queries, 0..count, &self.resolve, 0);
                encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, bytes);
                let mut bytes = bytes;
                if let (Some(stats), true) = (&self.stats, self.stats_used.get()) {
                    encoder.resolve_query_set(stats, 0..1, &self.resolve, STATS_AT);
                    encoder.copy_buffer_to_buffer(
                        &self.resolve,
                        STATS_AT,
                        &self.read,
                        STATS_AT,
                        STATS_BYTES,
                    );
                    bytes = STATS_AT + STATS_BYTES;
                }
                queue.submit([encoder.finish()]);
                let mapped = Arc::new(AtomicBool::new(false));
                let flag = mapped.clone();
                self.read
                    .slice(..bytes)
                    .map_async(wgpu::MapMode::Read, move |r| {
                        flag.store(r.is_ok(), Ordering::Release)
                    });
                self.stage = Stage::Reading(mapped, bytes);
            }
            Stage::Reading(mapped, bytes) if mapped.load(Ordering::Acquire) => {
                let bytes = *bytes;
                self.collect(bytes);
                self.stage = Stage::Idle;
            }
            _ => {}
        }
        if matches!(self.stage, Stage::Idle) {
            self.names.borrow_mut().clear();
            self.stats_used.set(false);
            self.stats_open.set(false);
            self.stage = Stage::Recording;
            self.recording.set(true);
        } else {
            self.recording.set(false);
        }
    }

    fn next(&self, name: &'static str) -> Option<u32> {
        if !self.recording.get() {
            return None;
        }
        let mut names = self.names.borrow_mut();
        let i = names.len() as u32;
        if i >= CAPACITY {
            return None;
        }
        names.push(name);
        Some(i)
    }

    pub fn encoder(&self, encoder: &mut wgpu::CommandEncoder, name: &'static str) {
        if let Some(i) = self.next(name) {
            encoder.write_timestamp(&self.queries, i);
        }
    }

    pub fn pass(&self, pass: &mut wgpu::RenderPass<'_>, name: &'static str) {
        if matches!(name, "terrain_p1" | "terrain_solid") {
            self.stats_end(pass);
        }
        if let Some(i) = self.next(name) {
            pass.write_timestamp(&self.queries, i);
        }
        if name == "sky" {
            self.stats_begin(pass);
        }
    }

    pub fn stats_begin(&self, pass: &mut wgpu::RenderPass<'_>) {
        if let (Some(stats), true, false) =
            (&self.stats, self.recording.get(), self.stats_used.get())
        {
            pass.begin_pipeline_statistics_query(stats, 0);
            self.stats_open.set(true);
            self.stats_used.set(true);
        }
    }

    pub fn stats_end(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.stats_open.replace(false) {
            pass.end_pipeline_statistics_query();
        }
    }

    pub fn submitted(&mut self, queue: &wgpu::Queue) {
        self.recording.set(false);
        if !matches!(self.stage, Stage::Recording) {
            return;
        }
        if self.names.borrow().len() < 2 {
            self.stage = Stage::Idle;
            return;
        }
        let done = Arc::new(AtomicBool::new(false));
        let flag = done.clone();
        queue.on_submitted_work_done(move || flag.store(true, Ordering::Release));
        self.stage = Stage::Running(done);
    }

    fn collect(&mut self, bytes: u64) {
        let Ok(data) = self.read.slice(..bytes).get_mapped_range() else {
            return;
        };
        let all: Vec<u64> = data
            .as_chunks::<8>()
            .0
            .iter()
            .map(|b| u64::from_le_bytes(*b))
            .collect();
        drop(data);
        let at = (STATS_AT / 8) as usize;
        if all.len() >= at + 4 {
            for (window, v) in self.stats_samples.iter_mut().zip(&all[at..at + 4]) {
                push(window, *v as f32);
            }
        }
        let stamps = all[..all.len().min(at)].to_vec();
        self.read.unmap();
        let names = self.names.borrow().clone();
        let mut frame: Vec<(&'static str, f32)> = Vec::new();
        for i in 1..stamps.len().min(names.len()) {
            if stamps[i] == 0 || stamps[i - 1] == 0 || stamps[i] < stamps[i - 1] {
                continue;
            }
            let ms = (stamps[i] - stamps[i - 1]) as f32 * self.period_ns / 1e6;
            match frame.iter_mut().find(|f| f.0 == names[i]) {
                Some(f) => f.1 += ms,
                None => frame.push((names[i], ms)),
            }
        }
        let total: f32 = frame.iter().map(|f| f.1).sum();
        push(&mut self.totals, total);
        for (name, ms) in frame {
            let i = match self.samples.iter().position(|s| s.0 == name) {
                Some(i) => i,
                None => {
                    self.samples.push((name, VecDeque::new()));
                    self.samples.len() - 1
                }
            };
            push(&mut self.samples[i].1, ms);
        }
    }

    pub fn medians(&self) -> Vec<(&'static str, f32)> {
        self.samples
            .iter()
            .filter(|s| !s.1.is_empty())
            .map(|(name, v)| (*name, median(v)))
            .collect()
    }

    pub fn world_statistics(&self) -> Option<[f32; 4]> {
        if self.stats_samples[0].is_empty() {
            return None;
        }
        Some(std::array::from_fn(|i| median(&self.stats_samples[i])))
    }

    pub fn total(&self) -> f32 {
        median(&self.totals)
    }
}

fn push(window: &mut VecDeque<f32>, ms: f32) {
    if window.len() == WINDOW {
        window.pop_front();
    }
    window.push_back(ms);
}

fn median(window: &VecDeque<f32>) -> f32 {
    let mut v: Vec<f32> = window.iter().copied().collect();
    v.sort_by(f32::total_cmp);
    v.get(v.len() / 2).copied().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::median;
    use std::collections::VecDeque;

    #[test]
    fn median_of_window() {
        let v: VecDeque<f32> = [3.0, 1.0, 2.0].into();
        assert_eq!(median(&v), 2.0);
        assert_eq!(median(&VecDeque::new()), 0.0);
    }
}
