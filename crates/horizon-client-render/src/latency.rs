use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;

const WINDOW: usize = 240;

#[derive(Default)]
struct Window(VecDeque<f32>);

impl Window {
    fn push(&mut self, ms: f32) {
        if self.0.len() == WINDOW {
            self.0.pop_front();
        }
        self.0.push_back(ms);
    }
    fn average(&self) -> f32 {
        if self.0.is_empty() {
            0.0
        } else {
            self.0.iter().sum::<f32>() / self.0.len() as f32
        }
    }
    fn max(&self) -> f32 {
        self.0.iter().copied().fold(0.0, f32::max)
    }
}

#[derive(Default)]
struct Stats {
    submit: Window,
    gpu: Window,
}

static STATS: Mutex<Option<Stats>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Stats) -> R) -> R {
    let mut guard = STATS.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Stats::default))
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub submit_avg: f32,
    pub submit_max: f32,
    pub gpu_avg: f32,
    pub gpu_max: f32,
    pub samples: usize,
}

pub fn snapshot() -> Snapshot {
    with(|s| Snapshot {
        submit_avg: s.submit.average(),
        submit_max: s.submit.max(),
        gpu_avg: s.gpu.average(),
        gpu_max: s.gpu.max(),
        samples: s.submit.0.len(),
    })
}

pub fn clear() {
    with(|s| *s = Stats::default());
}

pub fn presented(input: Instant, queue: &wgpu::Queue) {
    with(|s| s.submit.push(input.elapsed().as_secs_f32() * 1000.0));
    queue.on_submitted_work_done(move || {
        with(|s| s.gpu.push(input.elapsed().as_secs_f32() * 1000.0));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_keeps_the_latest_samples() {
        let mut w = Window::default();
        assert_eq!((w.average(), w.max()), (0.0, 0.0));
        for i in 0..WINDOW + 10 {
            w.push(i as f32);
        }
        assert_eq!(w.0.len(), WINDOW);
        assert_eq!(w.max(), (WINDOW + 9) as f32);
        let expected = (10..WINDOW + 10).sum::<usize>() as f32 / WINDOW as f32;
        assert!((w.average() - expected).abs() < 1e-3);
    }

    #[test]
    fn small_window_average() {
        let mut w = Window::default();
        for ms in [2.0, 4.0, 9.0] {
            w.push(ms);
        }
        assert_eq!(w.average(), 5.0);
        assert_eq!(w.max(), 9.0);
    }
}
