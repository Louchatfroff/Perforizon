use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

use horizon_bench::Frames;
use serde_json::{Value, json};

use super::{
    Control, MARKER, REMESH_KEYS, RUNNING, SCENES, SEED, SETTLE, SETTLE_CAP, TUNE_SCALES, WORLD,
    graphics_calls, replace_world, settle, step_applies, tune_steps, wait_loaded, wait_reload, yaw,
};
use crate::control::Request;

const PASS_SECONDS: f64 = 6.0;
const PASS_GAP: Duration = Duration::from_millis(700);
const MIN_NOISE: f64 = 0.02;
const BUDGET: Duration = Duration::from_secs(90 * 60);
const WEIGHTS: (f64, f64, f64) = (0.4, 0.4, 0.2);
const TITLE_LIMIT: Duration = Duration::from_secs(180);

static CANCEL: AtomicBool = AtomicBool::new(false);

pub fn cancel() -> bool {
    let running = RUNNING.load(Ordering::SeqCst);
    if running {
        CANCEL.store(true, Ordering::SeqCst);
    }
    running
}

fn cancelled() -> Result<(), String> {
    if CANCEL.load(Ordering::SeqCst) {
        Err("cancelled".into())
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fps {
    pub avg: f64,
    pub low1: f64,
    pub low01: f64,
}

pub fn scene_score(f: Fps) -> f64 {
    WEIGHTS.0 * f.avg + WEIGHTS.1 * f.low1 + WEIGHTS.2 * f.low01
}

pub fn geometric_mean(values: &[f64]) -> f64 {
    if values.is_empty() || values.iter().any(|v| *v <= 0.0) {
        return 0.0;
    }
    (values.iter().map(|v| v.ln()).sum::<f64>() / values.len() as f64).exp()
}

pub fn spread(values: &[f64]) -> f64 {
    let mean = values.iter().sum::<f64>() / values.len().max(1) as f64;
    if mean <= 0.0 {
        return 0.0;
    }
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    (max - min) / mean
}

#[derive(Clone, Debug, Default)]
pub struct Run {
    pub scenes: Vec<Vec<Fps>>,
}

impl Run {
    pub fn pass_scores(&self) -> Vec<f64> {
        let passes = self.scenes.iter().map(Vec::len).min().unwrap_or(0);
        (0..passes)
            .map(|p| {
                let per_scene: Vec<f64> = self.scenes.iter().map(|s| scene_score(s[p])).collect();
                geometric_mean(&per_scene)
            })
            .collect()
    }
    pub fn score(&self) -> f64 {
        let p = self.pass_scores();
        p.iter().sum::<f64>() / p.len().max(1) as f64
    }
    pub fn spread(&self) -> f64 {
        spread(&self.pass_scores())
    }
    pub fn scene_means(&self) -> Vec<Fps> {
        self.scenes
            .iter()
            .map(|s| {
                let n = s.len().max(1) as f64;
                Fps {
                    avg: s.iter().map(|f| f.avg).sum::<f64>() / n,
                    low1: s.iter().map(|f| f.low1).sum::<f64>() / n,
                    low01: s.iter().map(|f| f.low01).sum::<f64>() / n,
                }
            })
            .collect()
    }
    fn geomean_of(&self, pick: impl Fn(&Fps) -> f64) -> f64 {
        let v: Vec<f64> = self.scene_means().iter().map(pick).collect();
        geometric_mean(&v)
    }
}

pub fn noise(baseline: &Run, incumbent: &Run) -> f64 {
    MIN_NOISE.max(baseline.spread()).max(incumbent.spread())
}

pub fn beats(candidate: &Run, incumbent: &Run, noise: f64) -> bool {
    candidate.score() > incumbent.score() * (1.0 + noise)
}

pub fn candidates(parity: bool, status: &Value) -> Vec<super::TuneStep> {
    tune_steps(parity)
        .into_iter()
        .filter(|s| step_applies(s.0, status))
        .collect()
}

fn option_keys(step: &str) -> Vec<&str> {
    if step == "farLod" {
        vec!["farTerrainDistance", "farLodCoarse"]
    } else {
        vec![step]
    }
}

fn apply(control: &Control, status: &Value, key: &str, value: &str) -> Result<(), String> {
    for call in graphics_calls(key, value, status) {
        control.call(&call)?;
    }
    wait_reload(control, key, value);
    if REMESH_KEYS.contains(&key) {
        settle(control, SETTLE_CAP);
    }
    Ok(())
}

fn half_path(start: [f64; 3], end: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| start[i] + (end[i] - start[i]) / 2.0)
}

fn measure(control: &Control, passes: usize) -> Result<Run, String> {
    let mut run = Run::default();
    for (name, start, end, pitch) in SCENES {
        cancelled()?;
        control.call("time 6000")?;
        let [x, y, z] = start;
        control.call(&format!("tp {x} {y} {z}"))?;
        control.call(&format!("fly {x} {y} {z} {x} {y} {z} 0.1 {pitch}"))?;
        control.call(&format!("look {:.3} {pitch}", yaw(start, end)))?;
        let near = |p: &Value| {
            let p: Vec<f64> = p
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_f64)
                .collect();
            p.len() == 3 && (0..3).map(|i| (p[i] - start[i]).powi(2)).sum::<f64>() < 4.0
        };
        wait_loaded(control, near)?;
        thread::sleep(SETTLE);
        let [ex, ey, ez] = half_path(start, end);
        let mut scene = Vec::new();
        for _ in 0..passes {
            cancelled()?;
            let flight = control.call(&format!(
                "fly {x} {y} {z} {ex} {ey} {ez} {PASS_SECONDS} {pitch} raw"
            ))?;
            let frame_ms: Vec<f32> = flight["frame_ms"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_f64)
                .map(|v| v as f32)
                .collect();
            let f = Frames::of(name, 0, &frame_ms, &[], &[]).ok_or("the flight drew no frames")?;
            scene.push(Fps {
                avg: f.avg_fps,
                low1: f.low_1pct_fps,
                low01: f.low_01pct_fps,
            });
            thread::sleep(PASS_GAP);
        }
        run.scenes.push(scene);
    }
    Ok(run)
}

fn run_json(run: &Run) -> Value {
    let scenes: serde_json::Map<String, Value> = SCENES
        .iter()
        .zip(run.scene_means())
        .map(|(s, f)| {
            (
                s.0.to_owned(),
                json!({"avg_fps": f.avg, "low_1pct_fps": f.low1, "low_01pct_fps": f.low01}),
            )
        })
        .collect();
    json!({"score": run.score(), "pass_scores": run.pass_scores(), "spread": run.spread(), "scenes": scenes})
}

fn wait_title(control: &Control) -> Result<(), String> {
    let deadline = Instant::now() + TITLE_LIMIT;
    while Instant::now() < deadline {
        let s = control.status();
        if s["screen"] == "title" && !s["renderer"].is_null() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err("the title screen did not come back".into())
}

pub fn start(requests: Sender<Request>, target_fps: f64, passes: usize, game_dir: PathBuf) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    CANCEL.store(false, Ordering::SeqCst);
    thread::Builder::new()
        .name("horizon-autotune-precise".into())
        .spawn(move || {
            let control = Control(requests);
            let dump = control.call("graphics-dump").unwrap_or(Value::Null);
            let _ = control.call("perf-mode on");
            let done = drive(&control, target_fps, passes.max(2), &game_dir);
            let _ = control.call("perf-mode off");
            if control.status()["screen"] != "title" {
                let _ = control.call("close-world");
                let _ = wait_title(&control);
            }
            match &done {
                Ok(()) => {
                    let _ = control.call("graphics-save");
                }
                Err(_) => restore_options(&control, &dump),
            }
            RUNNING.store(false, Ordering::SeqCst);
            CANCEL.store(false, Ordering::SeqCst);
            match done {
                Ok(()) => println!("Horizon: precise autotune done"),
                Err(e) => eprintln!("Horizon: precise autotune stopped: {e}"),
            }
        })
        .expect("spawn the precise autotune thread");
}

fn restore_options(control: &Control, dump: &Value) {
    let Some(options) = dump["graphics"].as_object() else {
        return;
    };
    let mut keys: Vec<&str> = tune_steps(false)
        .iter()
        .flat_map(|s| option_keys(s.0))
        .collect();
    keys.push("resolutionScale");
    for key in keys {
        if let Some(value) = options.get(key).and_then(Value::as_str) {
            let _ = control.call(&format!("graphics {key} {value}"));
        }
    }
    let _ = control.call("graphics-save");
}

fn drive(control: &Control, target_fps: f64, passes: usize, game_dir: &Path) -> Result<(), String> {
    let began = Instant::now();
    let notice = |line: String| {
        let _ = control.call(&format!("notice Precise auto-tune: {line}"));
    };
    if control.status()["screen"] != "title" {
        control.call("close-world")?;
        wait_title(control)?;
    }
    replace_world(game_dir)?;
    control.call(&format!("open {WORLD} {SEED} creative"))?;
    std::fs::create_dir_all(game_dir.join("saves").join(WORLD)).map_err(|e| e.to_string())?;
    std::fs::write(
        game_dir.join("saves").join(WORLD).join(MARKER),
        "Created by the precise auto-tune; replaced by the next run.\n",
    )
    .map_err(|e| e.to_string())?;
    wait_loaded(control, |_| true)?;
    let status = control.status();
    let parity = status["optimization_mode"].as_str() == Some("parity");
    let steps = candidates(parity, &status);
    let total = steps.iter().map(|s| s.1.len() - 1).sum::<usize>();
    notice(format!(
        "started: {} candidates, {passes} passes over 4 scenes{}. Run `autotune --cancel` to stop.",
        total,
        if parity { " (Exact parity)" } else { "" }
    ));
    for (key, values) in &steps {
        apply(control, &status, key, values[0])?;
    }
    let baseline = measure(control, passes)?;
    let mut tries = vec![json!({"step": "baseline", "run": run_json(&baseline)})];
    let mut incumbent = baseline.clone();
    let mut chosen: Vec<(&str, &str)> = Vec::new();
    let mut done = 0;
    for (key, values) in &steps {
        let mut best = values[0];
        let mut last = values[0];
        for &value in &values[1..] {
            done += 1;
            if began.elapsed() > BUDGET {
                break;
            }
            apply(control, &status, key, value)?;
            last = value;
            let run = measure(control, passes)?;
            let margin = noise(&baseline, &incumbent);
            let wins = beats(&run, &incumbent, margin);
            tries.push(json!({"step": format!("{key}={value}"), "run": run_json(&run), "noise": margin, "kept": wins}));
            notice(format!(
                "{done}/{total} {key}={value}: {:+.1}% (needs {:.1}%){}",
                (run.score() / incumbent.score() - 1.0) * 100.0,
                margin * 100.0,
                if wins { ", kept" } else { "" }
            ));
            if wins {
                best = value;
                incumbent = run;
            }
        }
        if last != best {
            apply(control, &status, key, best)?;
        }
        if best != values[0] {
            chosen.push((key, best));
        }
    }
    let mut reverted = false;
    let mut validation = Value::Null;
    if !chosen.is_empty() {
        notice("validating the chosen set against the defaults".into());
        for (key, values) in &steps {
            apply(control, &status, key, values[0])?;
        }
        let defaults = measure(control, passes)?;
        for (key, value) in &chosen {
            apply(control, &status, key, value)?;
        }
        let picked = measure(control, passes)?;
        reverted = picked.score() <= defaults.score();
        validation = json!({"defaults": run_json(&defaults), "chosen": run_json(&picked), "reverted": reverted});
        if reverted {
            for (key, values) in &steps {
                apply(control, &status, key, values[0])?;
            }
            chosen.clear();
            incumbent = defaults;
            notice("the chosen set did not beat the defaults: back to the defaults".into());
        } else {
            incumbent = picked;
        }
    }
    let mut kept: Vec<String> = chosen.iter().map(|(k, v)| format!("{k}={v}")).collect();
    if !parity {
        for scale in TUNE_SCALES {
            if incumbent.geomean_of(|f| f.avg) >= target_fps
                && incumbent.geomean_of(|f| f.low1) >= target_fps * 0.5
                || began.elapsed() > BUDGET
            {
                break;
            }
            control.call(&format!("graphics resolutionScale {scale}"))?;
            incumbent = measure(control, passes)?;
            tries.push(
                json!({"step": format!("resolutionScale={scale}"), "run": run_json(&incumbent)}),
            );
            kept.retain(|k| !k.starts_with("resolutionScale"));
            kept.push(format!("resolutionScale={scale}"));
        }
    }
    let seconds = began.elapsed().as_secs_f64();
    let report = json!({
        "mode": "precise", "parity": parity, "passes": passes, "target_fps": target_fps,
        "weights": {"avg": WEIGHTS.0, "low_1pct": WEIGHTS.1, "low_01pct": WEIGHTS.2},
        "scenes": SCENES.iter().map(|s| s.0).collect::<Vec<_>>(),
        "baseline_spread": baseline.spread(), "kept": kept, "reverted": reverted,
        "validation": validation, "final": run_json(&incumbent), "seconds": seconds, "tries": tries,
    });
    std::fs::write(
        game_dir.join("autotune.json"),
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    notice(format!(
        "done in {:.0} s; kept {}",
        seconds,
        if kept.is_empty() {
            "nothing (defaults)".into()
        } else {
            kept.join(", ")
        }
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(passes: &[[f64; 4]]) -> Run {
        let mut scenes = vec![Vec::new(); 4];
        for pass in passes {
            for (s, v) in scenes.iter_mut().zip(pass) {
                s.push(Fps {
                    avg: *v,
                    low1: *v,
                    low01: *v,
                });
            }
        }
        Run { scenes }
    }

    #[test]
    fn scene_score_weights_lows_more_than_the_average() {
        let f = Fps {
            avg: 1000.0,
            low1: 500.0,
            low01: 200.0,
        };
        assert!((scene_score(f) - (400.0 + 200.0 + 40.0)).abs() < 1e-9);
    }

    #[test]
    fn geometric_mean_of_known_values() {
        assert!((geometric_mean(&[2.0, 8.0]) - 4.0).abs() < 1e-9);
        assert!((geometric_mean(&[1.0, 10.0, 100.0]) - 10.0).abs() < 1e-9);
        assert_eq!(geometric_mean(&[]), 0.0);
        assert_eq!(geometric_mean(&[5.0, 0.0]), 0.0);
    }

    #[test]
    fn run_score_is_the_mean_over_passes_of_the_geomean_over_scenes() {
        let r = run(&[[2.0, 8.0, 2.0, 8.0], [4.0, 16.0, 4.0, 16.0]]);
        assert_eq!(r.pass_scores().len(), 2);
        assert!((r.score() - (4.0 + 8.0) / 2.0).abs() < 1e-9);
    }

    #[test]
    fn spread_is_range_over_mean() {
        assert!((spread(&[90.0, 100.0, 110.0]) - 0.2).abs() < 1e-9);
        assert_eq!(spread(&[100.0, 100.0]), 0.0);
    }

    #[test]
    fn a_gain_inside_the_baseline_spread_is_not_kept() {
        let base = run(&[[100.0; 4], [90.0; 4], [110.0; 4]]);
        let n = noise(&base, &base);
        assert!((n - 0.2).abs() < 1e-9);
        let better = run(&[[115.0; 4], [110.0; 4], [120.0; 4]]);
        assert!(!beats(&better, &base, n));
        let much = run(&[[130.0; 4], [125.0; 4], [135.0; 4]]);
        assert!(beats(&much, &base, n));
    }

    #[test]
    fn noise_never_drops_below_the_floor() {
        let steady = run(&[[100.0; 4], [100.0; 4], [100.0; 4]]);
        assert_eq!(noise(&steady, &steady), MIN_NOISE);
        let up = run(&[[101.0; 4], [101.0; 4], [101.0; 4]]);
        assert!(!beats(&up, &steady, MIN_NOISE));
        let up = run(&[[103.0; 4], [103.0; 4], [103.0; 4]]);
        assert!(beats(&up, &steady, MIN_NOISE));
    }

    #[test]
    fn parity_candidates_exclude_pixel_changing_options_and_gates_apply() {
        let status = json!({"renderer": {"texture_compression_supported": false, "shadow_pass": false}, "live_particles": 0});
        let keys =
            |parity| -> Vec<&str> { candidates(parity, &status).iter().map(|s| s.0).collect() };
        assert!(!keys(true).contains(&"farLod"));
        assert!(!keys(true).contains(&"binaryGreedy"));
        assert!(keys(false).contains(&"binaryGreedy"));
        assert!(keys(true).contains(&"vertexPool"));
        assert!(!keys(false).contains(&"textureCompression"));
        assert!(!keys(false).contains(&"shadowIndirect"));
        assert!(!keys(false).contains(&"gpuParticles"));
        let all = json!({"renderer": {"texture_compression_supported": true, "shadow_pass": true}, "live_particles": 5000});
        let with: Vec<&str> = candidates(false, &all).iter().map(|s| s.0).collect();
        assert!(with.contains(&"textureCompression") && with.contains(&"gpuParticles"));
    }

    #[test]
    fn candidates_keep_the_quick_tune_order_with_the_default_value_first() {
        let all = json!({"renderer": {"texture_compression_supported": true, "shadow_pass": true}, "live_particles": 5000});
        let steps = candidates(false, &all);
        assert_eq!(steps[0].0, "farLod");
        assert_eq!(steps[0].1[0], "off");
        let pos = |k: &str| steps.iter().position(|s| s.0 == k).unwrap();
        assert!(pos("greedyMeshing") < pos("binaryGreedy"));
        assert!(pos("vertexPool") < pos("presentMode"));
        assert!(steps.iter().all(|s| s.1.len() >= 2));
    }

    #[test]
    fn the_far_lod_step_covers_both_of_its_options() {
        assert_eq!(
            option_keys("farLod"),
            ["farTerrainDistance", "farLodCoarse"]
        );
        assert_eq!(option_keys("hizOcclusion"), ["hizOcclusion"]);
    }

    #[test]
    fn the_flight_is_half_the_standard_path() {
        assert_eq!(
            half_path([0.0, 10.0, 0.0], [0.0, 10.0, 192.0]),
            [0.0, 10.0, 96.0]
        );
    }
}
