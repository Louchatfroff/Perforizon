//! `horizon-client --benchmark SCENARIO`: the run `tools/perf_bench.py` drives from outside, driven
//! from inside, so anyone can measure their machine with nothing but the game. A thread sends the
//! same control commands `horizon-ctl` would, through the game's own command queue, and writes a
//! report (docs/benchmark-report.md) to `benchmarks/` in the game folder.
//!
//! The scenes, the world (seed 1234, creative) and every step and figure match `tools/perf_bench.py`,
//! so reports from the two can be compared.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant};

use horizon_bench::{App, Frames, Kind, Report};
use serde_json::Value;

use crate::control::Request;

mod precise;
pub use precise::{cancel as cancel_precise, start as precise_autotune};

pub const SEED: i64 = 1234;
/// The world the benchmark creates under `saves/`, replaced on every run.
pub const WORLD: &str = "horizon-benchmark";
/// Written into the world so a later run only ever replaces its own world.
const MARKER: &str = "horizon-benchmark.txt";

/// A straight flight over terrain: name, start, end, pitch (`tools/graphics_bench.py`'s paths).
type Scene = (&'static str, [f64; 3], [f64; 3], f64);
pub const SCENES: [Scene; 4] = [
    (
        "village",
        [-960.0, 125.0, 80.0],
        [-960.0, 125.0, 272.0],
        35.0,
    ),
    (
        "forest",
        [-1100.0, 120.0, -560.0],
        [-1100.0, 120.0, -368.0],
        25.0,
    ),
    (
        "ocean",
        [-300.0, 85.0, -420.0],
        [-300.0, 85.0, -228.0],
        10.0,
    ),
    (
        "mountain",
        [560.0, 260.0, 860.0],
        [752.0, 260.0, 860.0],
        30.0,
    ),
];
const FLIGHT_SECONDS: f64 = 12.0;
const SETTLE: Duration = Duration::from_secs(2);
const LOAD_LIMIT: Duration = Duration::from_secs(600);

/// `standard` flies every scene; a scene's name flies only that one.
pub fn scenes(scenario: &str) -> Option<Vec<Scene>> {
    match scenario {
        "standard" => Some(SCENES.to_vec()),
        name => SCENES.iter().find(|s| s.0 == name).map(|s| vec![*s]),
    }
}

/// What the game knows about the run before it starts.
pub struct Setup {
    pub scenario: String,
    pub started: Instant,
    pub app: App,
    pub simulation_distance: u32,
    pub vsync: bool,
    /// `None`: unlimited.
    pub fps_cap: Option<u32>,
    pub offscreen: bool,
    pub game_dir: PathBuf,
}

/// Starts the driver; the game quits when it is done, and the report's path is printed.
pub fn start(requests: Sender<Request>, setup: Setup) {
    thread::Builder::new()
        .name("horizon-benchmark".into())
        .spawn(move || {
            let control = Control(requests);
            match run(&control, setup) {
                Ok(path) => println!("Horizon: benchmark report {}", path.display()),
                Err(e) => eprintln!("Horizon: benchmark failed: {e}"),
            }
            let _ = control.call("quit");
        })
        .expect("spawn the benchmark thread");
}

struct Control(Sender<Request>);

impl Control {
    fn call(&self, line: &str) -> Result<Value, String> {
        let (reply, response) = mpsc::channel();
        self.0
            .send(Request {
                line: line.to_owned(),
                reply,
            })
            .map_err(|_| "the game has exited".to_owned())?;
        // Flights and loads reply when they finish.
        let text = response
            .recv_timeout(LOAD_LIMIT)
            .map_err(|_| format!("{line}: no reply"))?;
        let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        match value.get("error").and_then(Value::as_str) {
            Some(e) => Err(format!("{line}: {e}")),
            None => Ok(value),
        }
    }
    fn status(&self) -> Value {
        self.call("status").unwrap_or(Value::Null)
    }
}

fn run(control: &Control, setup: Setup) -> Result<PathBuf, String> {
    let scenes =
        scenes(&setup.scenario).ok_or_else(|| format!("unknown scenario {}", setup.scenario))?;
    let mut report = Report::new(Kind::Game, setup.app.clone());
    let pid = std::process::id();
    report.conditions.load_avg_1m = horizon_bench::load_average();
    report.conditions.power = horizon_bench::power_source().map(str::to_owned);
    let rss = RssSampler::start(pid);

    let title = poll(control, LOAD_LIMIT, |s| {
        s["screen"] == "title" && !s["renderer"].is_null()
    })?;
    report.metric("startup.title_ms", &[ms(setup.started.elapsed())]);
    let renderer = &title["renderer"];
    report.hardware.gpu = Some(horizon_bench::Gpu {
        name: text(&renderer["gpu"]),
        vendor: match text(&renderer["vendor"]) {
            // Metal reports no vendor ID.
            v if v.is_empty() && text(&renderer["gpu"]).starts_with("Apple") => "apple".into(),
            v => v,
        },
        driver: text(&renderer["driver"]),
        backend: text(&renderer["backend"]).to_lowercase(),
        kind: text(&renderer["device_type"]),
        vram_mb: None,
    });
    if let Some(gpu) = report.hardware.gpu.as_mut() {
        let vendor = horizon_telemetry::system::gpu_vendor(&gpu.name, 0);
        let vendor = if gpu.vendor.is_empty() {
            vendor
        } else {
            gpu.vendor.as_str()
        }
        .to_owned();
        if gpu.kind.contains("iscrete") {
            gpu.vram_mb = horizon_telemetry::system::vram_mb(&vendor);
        }
    }
    report.hardware.storage =
        horizon_telemetry::system::storage(&setup.game_dir).map(str::to_owned);
    report.hardware.display = serde_json::from_value(title["display"].clone()).ok();

    replace_world(&setup.game_dir)?;
    control.call(&format!("open {WORLD} {SEED} creative"))?;
    // The world opens on a worker: its folder may not exist yet when `open` replies.
    std::fs::create_dir_all(setup.game_dir.join("saves").join(WORLD)).map_err(|e| e.to_string())?;
    std::fs::write(
        setup.game_dir.join("saves").join(WORLD).join(MARKER),
        "Created by horizon-client --benchmark; replaced by the next run.\n",
    )
    .map_err(|e| e.to_string())?;
    let loaded = wait_loaded(control, |_| true)?;
    report.metric("startup.world_ms", &[ms(loaded - setup.started)]);

    let mut steady = Vec::new();
    for (name, start, end, pitch) in scenes {
        control.call("time 6000")?;
        let meshed = control.status()["meshed_sections"].as_f64().unwrap_or(0.0);
        let generated = control.status()["generated_chunks"].as_f64().unwrap_or(0.0);
        let [x, y, z] = start;
        control.call(&format!("tp {x} {y} {z}"))?;
        let teleported = Instant::now();
        // A zero-length flight holds the player at the start, flying.
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
        let loaded = wait_loaded(control, near)?;
        let load = loaded - teleported;
        report.metric(&format!("chunks.{name}.load_ms"), &[ms(load)]);
        let status = control.status();
        let meshes = status["meshed_sections"].as_f64().unwrap_or(0.0) - meshed;
        let chunks = status["generated_chunks"].as_f64().unwrap_or(0.0) - generated;
        report.metric(&format!("chunks.{name}.meshed_count"), &[meshes]);
        report.metric(&format!("chunks.{name}.generated_count"), &[chunks]);
        if load.as_secs_f64() > 0.0 {
            report.metric(
                &format!("chunks.{name}.generated_per_s"),
                &[chunks / load.as_secs_f64()],
            );
        }
        thread::sleep(SETTLE);
        let [ex, ey, ez] = end;
        let begin = Instant::now();
        let flight = control.call(&format!(
            "fly {x} {y} {z} {ex} {ey} {ez} {FLIGHT_SECONDS} {pitch} raw"
        ))?;
        steady.push((begin, Instant::now()));
        let series = |key: &str| -> Vec<f32> {
            flight[key]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_f64)
                .map(|v| v as f32)
                .collect()
        };
        let frames = Frames::of(
            name,
            0,
            &series("frame_ms"),
            &series("cpu_ms"),
            &series("gpu_ms"),
        )
        .ok_or("the flight drew no frames")?;
        report.metric(&format!("fps.{name}.avg_fps"), &[frames.avg_fps]);
        report.metric(&format!("fps.{name}.low_1pct_fps"), &[frames.low_1pct_fps]);
        report.metric(
            &format!("fps.{name}.low_01pct_fps"),
            &[frames.low_01pct_fps],
        );
        report.frames.push(frames);
        let status = control.status();
        for pass in status["renderer"]["gpu_passes"]["passes"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if let (Some(pass_name), Some(pass_ms)) = (pass["name"].as_str(), pass["ms"].as_f64()) {
                report.metric(&format!("gpu_pass.{name}.{pass_name}"), &[pass_ms]);
            }
        }
    }

    let status = control.status();
    let renderer = &status["renderer"];
    // On Wayland the window is on a screen only once it has been shown: ask again.
    if report.hardware.display.is_none() {
        report.hardware.display = serde_json::from_value(status["display"].clone()).ok();
    }
    let c = &mut report.conditions;
    c.scenario = Some(setup.scenario.clone());
    c.seed = Some(SEED.to_string());
    c.render_distance = status["render_distance"].as_u64().map(|d| d as u32);
    c.simulation_distance = Some(setup.simulation_distance);
    c.window = serde_json::from_value(renderer["size"].clone()).ok();
    c.vsync = Some(setup.vsync);
    c.fps_cap = setup.fps_cap;
    c.offscreen = Some(setup.offscreen);
    c.repeats = Some(1);
    c.warmup_s = Some(SETTLE.as_secs_f64());
    if let Some(bytes) = renderer["gpu_allocated_bytes"].as_f64() {
        report.metric("memory.gpu_mb", &[bytes / f64::from(1 << 20)]);
    }
    let samples = rss.stop();
    if let Some(peak) = samples.iter().map(|s| s.1).max() {
        report.metric("memory.rss_peak_mb", &[peak as f64 / f64::from(1 << 20)]);
    }
    let mut during: Vec<u64> = samples
        .iter()
        .filter(|(t, _)| steady.iter().any(|(a, b)| a <= t && t <= b))
        .map(|s| s.1)
        .collect();
    during.sort_unstable();
    if let Some(&median) = during.get(during.len() / 2) {
        report.metric(
            "memory.rss_steady_mb",
            &[median as f64 / f64::from(1 << 20)],
        );
    }
    if let Ok(meta) = std::env::current_exe().and_then(std::fs::metadata) {
        report.metric("game.binary_bytes", &[meta.len() as f64]);
    }
    crate::telemetry::keep_benchmark(&crate::logging::game_home(), &report);
    report
        .write_to(&setup.game_dir.join("benchmarks"))
        .map_err(|e| e.to_string())
}

/// Removes the previous run's world, never one the benchmark didn't create.
fn replace_world(game_dir: &Path) -> Result<(), String> {
    let world = game_dir.join("saves").join(WORLD);
    if !world.exists() {
        return Ok(());
    }
    if !world.join(MARKER).exists() {
        return Err(format!(
            "{} exists and was not created by the benchmark: rename it",
            world.display()
        ));
    }
    std::fs::remove_dir_all(&world).map_err(|e| e.to_string())
}

fn poll(
    control: &Control,
    limit: Duration,
    ready: impl Fn(&Value) -> bool,
) -> Result<Value, String> {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        let status = control.status();
        if ready(&status) {
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(format!("not ready after {} s", limit.as_secs()))
}

/// Fewer section meshes than this in a second is the game remeshing blocks that change (random
/// ticks), not loading.
const QUIET_MESHES_PER_S: f64 = 100.0;

/// Waits until the player is spawned where `near` says, nothing is queued, the resident and
/// generated chunk counts have not changed for a second and the last sections are drawn (they wait
/// for their neighbours' light, so after the counts settle); returns when the last of these
/// happened (`tools/perf_bench.py`'s `wait_loaded`). With `status.streaming`, the sections are
/// drawn once no stage has held work for half a second: blocks changing after the load (fluids
/// settling, random ticks) only remesh in bursts. Without it, meshing has to go quiet.
///
/// Right after a teleport (and at spawn) the game reports nothing queued and steady counts for over
/// a second before it starts loading: the counts must change once, or stay full for 5 s, and the
/// resident chunks reach [`view_chunks`], before "steady" means loaded.
fn wait_loaded(control: &Control, near: impl Fn(&Value) -> bool) -> Result<Instant, String> {
    let deadline = Instant::now() + LOAD_LIMIT;
    let (mut last, mut changed, mut meshing) = (None, Instant::now(), Instant::now());
    let mut previous: Option<(Instant, f64)> = None;
    let (mut first, mut moved, began) = (None, false, Instant::now());
    while Instant::now() < deadline {
        let s = control.status();
        let now = Instant::now();
        let position = &s["player"]["position"];
        if position.is_null()
            || !near(position)
            || s["pending_chunks"] != 0
            || s["screen"] == "title"
        {
            (last, changed, previous, meshing) = (None, now, None, now);
        } else {
            let counts = (s["chunks"].as_u64(), s["generated_chunks"].as_u64());
            let resident = counts.0.unwrap_or(0);
            let min_chunks = view_chunks(s["render_distance"].as_u64().unwrap_or(0));
            let first = *first.get_or_insert(counts);
            moved = moved
                || counts != first
                || (min_chunks > 0
                    && resident >= min_chunks
                    && now - began > Duration::from_secs(5));
            if min_chunks > 0 && (!moved || resident < min_chunks) {
                (last, changed, previous, meshing) = (None, now, None, now);
                thread::sleep(Duration::from_millis(50));
                continue;
            }
            if last != Some(counts) {
                (last, changed) = (Some(counts), now);
            }
            let meshed = s["meshed_sections"].as_f64().unwrap_or(0.0);
            let quiet = if let Some(stages) = s["streaming"].as_object() {
                // Protos outside the view distance wait for neighbours that are never generated.
                if stages
                    .iter()
                    .any(|(k, v)| k != "protos" && v.as_u64().unwrap_or(0) > 0)
                {
                    meshing = now;
                }
                Duration::from_millis(500)
            } else {
                if let Some((at, before)) = previous
                    && meshed - before > QUIET_MESHES_PER_S * (now - at).as_secs_f64()
                {
                    meshing = now;
                }
                Duration::from_secs(1)
            };
            previous = Some((now, meshed));
            if now - changed >= Duration::from_secs(1) && now - meshing >= quiet {
                return Ok(changed.max(meshing));
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(format!("not loaded after {} s", LOAD_LIMIT.as_secs()))
}

/// 80% of the view circle (pi x RD^2): the game keeps 637 columns at RD 12 and 3725 at RD 32
/// (`tools/perf_bench.py`'s `view_chunks`).
fn view_chunks(distance: u64) -> u64 {
    (0.8 * std::f64::consts::PI * (distance * distance) as f64) as u64
}

/// Vanilla's yaw facing from `start` to `end`.
fn yaw(start: [f64; 3], end: [f64; 3]) -> f64 {
    (-(end[0] - start[0])).atan2(end[2] - start[2]).to_degrees()
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_owned()
}

/// Samples the process's resident memory four times a second, as `tools/perf_bench.py` does.
struct RssSampler {
    stop: Sender<()>,
    thread: thread::JoinHandle<Vec<(Instant, u64)>>,
}

impl RssSampler {
    fn start(pid: u32) -> Self {
        let (stop, stopped) = mpsc::channel();
        let thread = thread::spawn(move || {
            let mut samples = Vec::new();
            while let Err(mpsc::RecvTimeoutError::Timeout) =
                stopped.recv_timeout(Duration::from_millis(250))
            {
                samples.extend(horizon_bench::rss_bytes(pid).map(|b| (Instant::now(), b)));
            }
            samples
        });
        Self { stop, thread }
    }
    fn stop(self) -> Vec<(Instant, u64)> {
        let _ = self.stop.send(());
        self.thread.join().unwrap_or_default()
    }
}

const TUNE_SECONDS: f64 = 3.0;
const TUNE_BUDGET: Duration = Duration::from_secs(110);
const SETTLE_CAP: Duration = Duration::from_secs(12);
const RELOAD_CAP: Duration = Duration::from_secs(90);
type TuneStep = (&'static str, &'static [&'static str]);
const TUNE_STEPS: [TuneStep; 7] = [
    ("vertexPool", &["false", "true"]),
    ("meshShaders", &["false", "true"]),
    ("hizOcclusion", &["false", "true"]),
    ("caveCulling", &["true", "false"]),
    ("entityCulling", &["true", "false"]),
    ("shadowCache", &["false", "true"]),
    ("shadowIndirect", &["false", "true"]),
];
const LATENCY_TUNE_STEPS: [TuneStep; 2] = [
    ("presentMode", &["auto", "mailbox"]),
    ("frameLatency", &["0", "2"]),
];
const LATENCY_TIE: f64 = 0.97;
fn is_latency_step(key: &str) -> bool {
    LATENCY_TUNE_STEPS.iter().any(|s| s.0 == key)
}
fn lowest_latency_within_tie(scores: &[f64]) -> Option<usize> {
    let best = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    scores.iter().position(|&s| s >= best * LATENCY_TIE)
}
const LOSSY_TUNE_STEPS: [TuneStep; 5] = [
    ("textureCompression", &["false", "true"]),
    ("greedyMeshing", &["true", "false"]),
    ("binaryGreedy", &["false", "true"]),
    ("gpuParticles", &["false", "true"]),
    ("leafDistance", &["5", "3", "0"]),
];
const FAR_LOD_STEP: TuneStep = ("farLod", &["off", "16c", "10c"]);
fn tune_steps(parity: bool) -> Vec<TuneStep> {
    let by_name = |name: &str| {
        *LOSSY_TUNE_STEPS
            .iter()
            .find(|s| s.0 == name)
            .expect("a lossy step")
    };
    if parity {
        return TUNE_STEPS
            .iter()
            .chain(&LATENCY_TUNE_STEPS)
            .copied()
            .collect();
    }
    let mut steps = vec![FAR_LOD_STEP, TUNE_STEPS[0]];
    steps.extend(["greedyMeshing", "binaryGreedy", "leafDistance"].map(by_name));
    steps.extend(&TUNE_STEPS[1..]);
    steps.extend(["textureCompression", "gpuParticles"].map(by_name));
    steps.extend(&LATENCY_TUNE_STEPS);
    steps
}
const REMESH_KEYS: [&str; 6] = [
    "vertexPool",
    "greedyMeshing",
    "binaryGreedy",
    "meshShaders",
    "leafDistance",
    "farLod",
];
fn graphics_calls(key: &str, value: &str, status: &Value) -> Vec<String> {
    match (key, value) {
        ("farLod", "off") => vec![
            format!(
                "graphics farTerrainDistance {}",
                status["lod"]["far_terrain_distance"].as_u64().unwrap_or(0)
            ),
            format!(
                "graphics farLodCoarse {}",
                status["lod"]["far_lod_coarse"].as_bool().unwrap_or(false)
            ),
        ],
        ("farLod", chunks) => vec![
            format!(
                "graphics farTerrainDistance {}",
                chunks.trim_end_matches('c')
            ),
            format!("graphics farLodCoarse {}", chunks.ends_with('c')),
        ],
        _ => vec![format!("graphics {key} {value}")],
    }
}
const MANY_PARTICLES: u64 = 2000;
fn many_particles(status: &Value) -> bool {
    status["live_particles"].as_u64() >= Some(MANY_PARTICLES)
}
fn step_applies(key: &str, status: &Value) -> bool {
    match key {
        "textureCompression" => status["renderer"]["texture_compression_supported"] == true,
        "shadowIndirect" => status["renderer"]["shadow_pass"] == true,
        "gpuParticles" => many_particles(status),
        "presentMode" | "frameLatency" => status["renderer"]["offscreen"] != true,
        _ => true,
    }
}
const TUNE_SCALES: [u32; 5] = [90, 80, 67, 60, 50];

/// Tries the performance settings one at a time in the world the player is in, keeps what
/// draws the most frames, then lowers the resolution scale until `target_fps` is met, and
/// saves the result to the options; the steps are printed and written to `autotune.json`.
pub fn autotune(requests: Sender<Request>, target_fps: f64, game_dir: PathBuf) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    thread::Builder::new()
        .name("horizon-autotune".into())
        .spawn(move || {
            let control = Control(requests);
            let before = control.status();
            let _ = control.call("perf-mode on");
            let done = tune(&control, target_fps, &game_dir);
            let _ = control.call("perf-mode off");
            restore(&control, &before);
            RUNNING.store(false, Ordering::SeqCst);
            match done {
                Ok(()) => println!("Horizon: autotune done"),
                Err(e) => eprintln!("Horizon: autotune failed: {e}"),
            }
        })
        .expect("spawn the autotune thread");
}

static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn autotune_running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

fn restore(control: &Control, before: &Value) {
    let position: Vec<f64> = before["player"]["position"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_f64)
        .collect();
    if let [x, y, z] = position[..] {
        let _ = control.call(&format!("tp {x} {y} {z}"));
    }
    if let (Some(yaw), Some(pitch)) = (before["yaw"].as_f64(), before["pitch"].as_f64()) {
        let _ = control.call(&format!("look {yaw} {pitch}"));
    }
    if let Some(flying) = before["player"]["flying"].as_bool() {
        let _ = control.call(&format!("flying {}", if flying { "on" } else { "off" }));
    }
}

fn settle(control: &Control, cap: Duration) {
    let deadline = Instant::now() + cap;
    let mut quiet = 0;
    while quiet < 3 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(500));
        let s = control.status();
        let tiles_ready =
            s["lod"]["sampled_target_tiles"].as_u64() >= s["lod"]["target_tiles"].as_u64();
        quiet = if s["pending_chunks"] == 0 && tiles_ready {
            quiet + 1
        } else {
            0
        };
    }
}

fn wait_reload(control: &Control, key: &str, value: &str) {
    if key != "textureCompression" {
        return;
    }
    let on = value == "true";
    let deadline = Instant::now() + RELOAD_CAP;
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(500));
        if control.status()["renderer"]["texture_compression"].as_bool() == Some(on) {
            break;
        }
    }
    settle(control, SETTLE_CAP);
}

fn tune_scales(parity: bool) -> &'static [u32] {
    if parity { &[] } else { &TUNE_SCALES }
}

fn tune(control: &Control, target_fps: f64, game_dir: &Path) -> Result<(), String> {
    let status = control.status();
    let parity = status["optimization_mode"].as_str() == Some("parity");
    let notice = |line: String| {
        let _ = control.call(&format!("notice Auto-tune: {line}"));
    };
    notice(if parity {
        "started (Exact parity: pixel-changing options stay off)".into()
    } else {
        "started".into()
    });
    let position: Vec<f64> = status["player"]["position"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_f64)
        .collect();
    let [x, y, z] = position[..] else {
        return Err("no player position".into());
    };
    let pitch = status["pitch"].as_f64().unwrap_or(10.0);
    let yaw = status["yaw"].as_f64().unwrap_or(0.0);
    let measure = |label: &str| -> Result<(f64, f64), String> {
        thread::sleep(Duration::from_millis(1500));
        control.call(&format!("tp {x} {y} {z}"))?;
        control.call(&format!("look {yaw} {pitch}"))?;
        let flight = control.call(&format!(
            "fly {x} {y} {z} {x} {y} {} {TUNE_SECONDS} {pitch} raw",
            z + 48.0
        ))?;
        let frame_ms: Vec<f32> = flight["frame_ms"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_f64)
            .map(|v| v as f32)
            .collect();
        let frames = Frames::of(label, 0, &frame_ms, &[], &[]).ok_or("no frames drawn")?;
        let result = (frames.avg_fps, frames.low_1pct_fps);
        println!(
            "Horizon: autotune {label}: {:.0} fps, 1% low {:.0}",
            result.0, result.1
        );
        Ok(result)
    };
    let score = |(avg, low): (f64, f64)| avg * 0.5 + low * 0.5;
    let mut tries = Vec::new();
    settle(control, Duration::from_secs(60));
    let began = Instant::now();
    let mut best = measure("start")?;
    tries.push(serde_json::json!({"step": "start", "avg": best.0, "low": best.1}));
    let mut kept = Vec::new();
    for (key, values) in tune_steps(parity) {
        if !step_applies(key, &status) {
            continue;
        }
        let mut chosen = None;
        let mut measured = Vec::new();
        let mut last = "";
        for &value in values {
            if began.elapsed() > TUNE_BUDGET {
                break;
            }
            last = value;
            for call in graphics_calls(key, value, &status) {
                control.call(&call)?;
            }
            wait_reload(control, key, value);
            if REMESH_KEYS.contains(&key) {
                settle(control, SETTLE_CAP);
            }
            let result = measure(&format!("{key}={value}"))?;
            tries.push(serde_json::json!({"step": format!("{key}={value}"), "avg": result.0, "low": result.1}));
            measured.push((value, result));
            if chosen.is_none_or(|(_, r)| score(result) > score(r) * 1.03) {
                chosen = Some((value, result));
            }
        }
        if is_latency_step(key) {
            let scores: Vec<f64> = measured.iter().map(|&(_, r)| score(r)).collect();
            chosen = lowest_latency_within_tie(&scores).map(|i| measured[i]);
        }
        if let Some((value, result)) = chosen {
            for call in graphics_calls(key, value, &status) {
                control.call(&call)?;
            }
            if value != last {
                wait_reload(control, key, value);
            }
            if value != last && REMESH_KEYS.contains(&key) {
                settle(control, SETTLE_CAP);
            }
            best = result;
            kept.push(format!("{key}={value}"));
            notice(format!("{key} {value}, {:.0} fps", best.0));
        }
    }
    for scale in tune_scales(parity) {
        if began.elapsed() > TUNE_BUDGET || (best.0 >= target_fps && best.1 >= target_fps * 0.5) {
            break;
        }
        control.call(&format!("graphics resolutionScale {scale}"))?;
        best = measure(&format!("resolutionScale={scale}"))?;
        tries.push(serde_json::json!({"step": format!("resolutionScale={scale}"), "avg": best.0, "low": best.1}));
    }
    control.call("graphics-save")?;
    notice(format!(
        "done in {:.0}s, {:.0} fps (1% low {:.0}); kept {}",
        began.elapsed().as_secs_f64(),
        best.0,
        best.1,
        kept.join(", ")
    ));
    let report = serde_json::json!({"target_fps": target_fps, "final": {"avg": best.0, "low": best.1}, "kept": kept, "seconds": began.elapsed().as_secs_f64(), "tries": tries});
    std::fs::write(
        game_dir.join("autotune.json"),
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parity_mode_never_tries_pixel_changing_options() {
        let keys = |parity| -> Vec<&str> { tune_steps(parity).iter().map(|s| s.0).collect() };
        assert!(!keys(true).contains(&"greedyMeshing"));
        assert!(keys(false).contains(&"greedyMeshing"));
        assert!(!keys(true).contains(&"leafDistance"));
        assert!(keys(false).contains(&"leafDistance"));
        assert_eq!(keys(false).len(), keys(true).len() + 6);
    }

    #[test]
    fn gpu_particles_is_a_pixel_changing_candidate_tried_only_with_many_particles() {
        let steps = tune_steps(false);
        let step = steps.iter().find(|s| s.0 == "gpuParticles").unwrap();
        assert_eq!(step.1, &["false", "true"]);
        assert!(!tune_steps(true).iter().any(|s| s.0 == "gpuParticles"));
        assert!(!REMESH_KEYS.contains(&"gpuParticles"));
        let few = serde_json::json!({"live_particles": 1999});
        let many = serde_json::json!({"live_particles": 2000});
        assert!(!many_particles(&few));
        assert!(many_particles(&many));
        assert!(!many_particles(&serde_json::json!({})));
    }

    #[test]
    fn texture_compression_is_a_pixel_changing_candidate_with_off_as_baseline() {
        let steps = tune_steps(false);
        let step = steps.iter().find(|s| s.0 == "textureCompression").unwrap();
        assert_eq!(step.1, &["false", "true"]);
        assert!(!tune_steps(true).iter().any(|s| s.0 == "textureCompression"));
    }

    #[test]
    fn vertex_pool_is_tried_in_both_modes_with_off_as_baseline_and_settles() {
        for parity in [true, false] {
            let steps = tune_steps(parity);
            let step = steps.iter().find(|s| s.0 == "vertexPool").unwrap();
            assert_eq!(step.1, &["false", "true"]);
        }
        assert!(REMESH_KEYS.contains(&"vertexPool"));
    }

    #[test]
    fn hiz_occlusion_is_tried_in_both_modes_with_off_as_baseline_without_remeshing() {
        for parity in [true, false] {
            let steps = tune_steps(parity);
            let step = steps.iter().find(|s| s.0 == "hizOcclusion").unwrap();
            assert_eq!(step.1, &["false", "true"]);
        }
        assert!(!REMESH_KEYS.contains(&"hizOcclusion"));
    }

    #[test]
    fn shadow_indirect_is_tried_in_both_modes_with_off_as_baseline_without_remeshing() {
        for parity in [true, false] {
            let steps = tune_steps(parity);
            let step = steps.iter().find(|s| s.0 == "shadowIndirect").unwrap();
            assert_eq!(step.1, &["false", "true"]);
        }
        assert!(!REMESH_KEYS.contains(&"shadowIndirect"));
    }

    #[test]
    fn binary_greedy_is_tried_right_after_greedy_meshing_with_off_as_baseline() {
        let steps = tune_steps(false);
        let greedy = steps.iter().position(|s| s.0 == "greedyMeshing").unwrap();
        assert_eq!(steps[greedy + 1], ("binaryGreedy", &["false", "true"][..]));
        assert!(!tune_steps(true).iter().any(|s| s.0 == "binaryGreedy"));
    }

    #[test]
    fn latency_steps_run_in_parity_mode_with_the_lowest_latency_value_first() {
        for parity in [true, false] {
            let steps = tune_steps(parity);
            for (key, values) in LATENCY_TUNE_STEPS {
                assert!(steps.iter().any(|s| s.0 == key));
                assert!(is_latency_step(key));
                assert!(!REMESH_KEYS.contains(&key));
                assert!(values.len() >= 2);
            }
        }
        assert_eq!(LATENCY_TUNE_STEPS[0].1[0], "auto");
        assert_eq!(LATENCY_TUNE_STEPS[1].1[0], "0");
        assert!(!is_latency_step("shadowCache"));
    }

    #[test]
    fn equal_frame_rates_keep_the_lower_latency_candidate() {
        assert_eq!(lowest_latency_within_tie(&[1000.0, 1010.0]), Some(0));
        assert_eq!(lowest_latency_within_tie(&[1000.0, 1029.0]), Some(0));
        assert_eq!(lowest_latency_within_tie(&[1000.0, 1040.0]), Some(1));
        assert_eq!(lowest_latency_within_tie(&[900.0, 1100.0, 1090.0]), Some(1));
        assert_eq!(lowest_latency_within_tie(&[]), None);
    }

    #[test]
    fn every_pixel_changing_mesh_option_remeshes_before_measuring() {
        assert!(REMESH_KEYS.contains(&"leafDistance"));
        assert!(REMESH_KEYS.contains(&"binaryGreedy"));
        let (_, values) = LOSSY_TUNE_STEPS
            .iter()
            .find(|s| s.0 == "leafDistance")
            .unwrap();
        assert_eq!(values[0], "5");
    }

    #[test]
    fn far_lod_is_a_pixel_changing_candidate_with_off_as_baseline_and_remeshes() {
        let steps = tune_steps(false);
        let step = steps.iter().find(|s| s.0 == "farLod").unwrap();
        assert_eq!(step.1, &["off", "16c", "10c"]);
        assert!(REMESH_KEYS.contains(&"farLod"));
        assert!(!tune_steps(true).iter().any(|s| s.0 == "farLod"));
        assert_eq!(steps[0].0, "farLod");
    }

    #[test]
    fn far_lod_baseline_keeps_the_players_setting_and_latency_skips_offscreen() {
        let live = serde_json::json!({"lod": {"far_terrain_distance": 24, "far_lod_coarse": true}});
        assert_eq!(
            graphics_calls("farLod", "off", &live),
            [
                "graphics farTerrainDistance 24",
                "graphics farLodCoarse true"
            ]
        );
        let off = serde_json::json!({"renderer": {"offscreen": true}});
        let on = serde_json::json!({"renderer": {"offscreen": false}});
        for key in ["presentMode", "frameLatency"] {
            assert!(!step_applies(key, &off));
            assert!(step_applies(key, &on));
        }
        assert!(step_applies("vertexPool", &off));
    }

    #[test]
    fn far_lod_values_expand_to_the_two_options_and_other_keys_pass_through() {
        assert_eq!(
            graphics_calls("farLod", "off", &Value::Null),
            [
                "graphics farTerrainDistance 0",
                "graphics farLodCoarse false"
            ]
        );
        assert_eq!(
            graphics_calls("farLod", "16", &Value::Null),
            [
                "graphics farTerrainDistance 16",
                "graphics farLodCoarse false"
            ]
        );
        assert_eq!(
            graphics_calls("farLod", "16c", &Value::Null),
            [
                "graphics farTerrainDistance 16",
                "graphics farLodCoarse true"
            ]
        );
        assert_eq!(
            graphics_calls("vertexPool", "true", &Value::Null),
            ["graphics vertexPool true"]
        );
    }

    #[test]
    fn parity_mode_never_lowers_the_resolution_scale() {
        assert!(tune_scales(true).is_empty());
        assert!(!tune_scales(false).is_empty());
    }

    #[test]
    fn scenarios() {
        assert_eq!(scenes("standard").unwrap().len(), 4);
        assert_eq!(scenes("ocean").unwrap()[0].0, "ocean");
        assert!(scenes("nether").is_none());
    }

    #[test]
    fn view_chunks_matches_perf_bench() {
        assert_eq!(view_chunks(12), 361);
        assert_eq!(view_chunks(32), 2573);
        assert_eq!(view_chunks(0), 0);
    }
    /// The status a fake control answers with: the player is where it should
    /// be, nothing is queued, and the world holds `chunks` resident chunks.
    fn status(chunks: u64) -> String {
        serde_json::json!({
            "player": {"position": [0.0, 64.0, 0.0]},
            "pending_chunks": 0,
            "screen": "playing",
            "render_distance": 12,
            "chunks": chunks,
            "generated_chunks": chunks,
            "meshed_sections": 0,
            "streaming": {},
        })
        .to_string()
    }

    #[test]
    fn the_load_waits_for_the_chunks_a_teleport_starts() {
        // Right after a teleport the game reports steady counts and nothing
        // queued for over a second, before the integrated server has sent the
        // first chunk: the quiet rule alone would call that loaded, and the
        // scene would be measured over an empty view (#1749).
        const LATE: usize = 30;
        let (tx, rx) = mpsc::channel::<Request>();
        let polls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = std::sync::Arc::clone(&polls);
        let server = thread::spawn(move || {
            let mut seen = 0;
            while let Ok(request) = rx.recv() {
                if request.line == "status" {
                    let chunks = if seen < LATE { 0 } else { 400 };
                    seen += 1;
                    counted.store(seen, std::sync::atomic::Ordering::SeqCst);
                    let _ = request.reply.send(status(chunks));
                } else {
                    let _ = request.reply.send("{}".to_owned());
                }
            }
        });
        let control = Control(tx);
        wait_loaded(&control, |_| true).expect("the scene loads");
        assert!(
            polls.load(std::sync::atomic::Ordering::SeqCst) > LATE,
            "the load finished before the chunks arrived"
        );
        drop(control);
        let _ = server.join();
    }

    #[test]
    fn yaw_matches_vanilla() {
        // South (+Z) is 0, west (-X) 90, east (+X) -90.
        assert!((yaw([0.0; 3], [0.0, 0.0, 10.0])).abs() < 1e-9);
        assert!((yaw([0.0; 3], [-10.0, 0.0, 0.0]) - 90.0).abs() < 1e-9);
        assert!((yaw([0.0; 3], [10.0, 0.0, 0.0]) + 90.0).abs() < 1e-9);
    }

    #[test]
    fn only_replaces_its_own_world() {
        let dir =
            std::env::temp_dir().join(format!("horizon-benchmark-test-{}", std::process::id()));
        let world = dir.join("saves").join(WORLD);
        std::fs::create_dir_all(&world).unwrap();
        assert!(
            replace_world(&dir).is_err(),
            "a player's world of that name stays"
        );
        assert!(world.exists());
        std::fs::write(world.join(MARKER), "").unwrap();
        replace_world(&dir).unwrap();
        assert!(!world.exists());
        replace_world(&dir).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}
