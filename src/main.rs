use daimon_integrators::*;
use serde_json::{json, Value};
use std::{
    env, fs,
    io::{Read, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
#[cfg(feature = "ui")]
mod ui;

pub struct Engine {
    snapshot: Snapshot,
    child: Option<Child>,
    pending: Option<(String, u32, String, Instant)>,
    stop_at: Option<Instant>,
    devices: Devices,
    ui_ready: Arc<AtomicBool>,
}
impl Engine {
    fn new(devices: Devices, ready: Arc<AtomicBool>) -> Self {
        Self {
            snapshot: Snapshot::default(),
            child: None,
            pending: None,
            stop_at: None,
            devices,
            ui_ready: ready,
        }
    }
    fn request(&mut self, request: Request) -> Value {
        match request {
            Request::Status => json!({"ok":true,"snapshot":self.snapshot}),
            Request::View { compact } => {
                self.snapshot.compact = compact;
                json!({"ok":true,"stage":"view_requested","compact":compact})
            }
            Request::Stop => {
                if let Some(child) = &mut self.child {
                    match child.try_wait() {
                        Ok(None) => {
                            // The group belongs to this still-unreaped child, never an arbitrary PID.
                            unsafe {
                                libc::kill(-(child.id() as i32), libc::SIGINT);
                            }
                            self.stop_at.get_or_insert_with(Instant::now);
                            self.snapshot.managed.phase = "stopping".into();
                            json!({"ok":true,"stage":"stop_requested"})
                        }
                        Ok(Some(_)) => json!({"ok":true,"stage":"already_exited"}),
                        Err(e) => json!({"ok":false,"error":e.to_string()}),
                    }
                } else if self.pending.take().is_some() {
                    self.snapshot.managed.phase = "cancelled_before_capture".into();
                    self.snapshot.managed.mode = None;
                    json!({"ok":true,"stage":"cancelled_before_capture"})
                } else {
                    json!({"ok":true,"stage":"no_managed_capture"})
                }
            }
            Request::Capture {
                mode,
                seconds,
                output,
            } => {
                if let Err(e) = validate_capture(&mode, seconds, &output) {
                    return json!({"ok":false,"error":e});
                }
                if self.child.is_some() || self.pending.is_some() {
                    return json!({"ok":false,"error":"managed capture already active"});
                }
                self.ui_ready.store(false, Ordering::SeqCst);
                self.snapshot.managed = Managed {
                    mode: Some(mode.clone()),
                    phase: "preparing".into(),
                    started_at: now(),
                    output: Some(output.clone()),
                    ..Default::default()
                };
                self.pending = Some((mode, seconds, output, Instant::now()));
                json!({"ok":true,"stage":"preparing","effect_observed":false})
            }
        }
    }
    fn tick(&mut self) {
        if let Some((_, _, _, since)) = &self.pending {
            if since.elapsed() > Duration::from_secs(5) && !self.ui_ready.load(Ordering::SeqCst) {
                self.pending = None;
                self.snapshot.managed.phase = "failed".into();
                self.snapshot.managed.error =
                    Some("panel did not acknowledge preparing state; no capture started".into());
            } else if since.elapsed() > Duration::from_millis(700)
                && self.ui_ready.load(Ordering::SeqCst)
            {
                let (mode, seconds, output, _) = self.pending.take().unwrap();
                let mut cmd = Command::new("ffmpeg");
                cmd.args(["-hide_banner", "-loglevel", "error", "-n"]);
                if mode != "audio" {
                    cmd.args([
                        "-thread_queue_size",
                        "512",
                        "-f",
                        "v4l2",
                        "-input_format",
                        "mjpeg",
                        "-video_size",
                        "640x480",
                        "-framerate",
                        "30",
                        "-i",
                        &self.devices.camera,
                    ]);
                }
                if mode != "video" {
                    cmd.args([
                        "-thread_queue_size",
                        "512",
                        "-f",
                        "alsa",
                        "-ar",
                        "48000",
                        "-i",
                        &self.devices.audio,
                    ]);
                }
                cmd.args(["-t", &seconds.to_string()]);
                if mode != "audio" {
                    cmd.args([
                        "-c:v",
                        "libx264",
                        "-preset",
                        "ultrafast",
                        "-pix_fmt",
                        "yuv420p",
                    ]);
                }
                if mode != "video" {
                    cmd.args(["-c:a", "aac"]);
                }
                if mode != "audio" {
                    cmd.args(["-movflags", "+faststart"]);
                }
                let log = Path::new(&output).with_extension("capture.log");
                match fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(log)
                {
                    Ok(log) => {
                        cmd.arg(output)
                            .stdin(Stdio::null())
                            .stdout(Stdio::null())
                            .stderr(log)
                            .process_group(0);
                        match cmd.spawn() {
                            Ok(child) => {
                                self.snapshot.managed.pid = Some(child.id());
                                self.snapshot.managed.started_at = now();
                                self.snapshot.managed.phase = "capturing".into();
                                self.child = Some(child);
                            }
                            Err(e) => {
                                self.snapshot.managed.phase = "failed".into();
                                self.snapshot.managed.error = Some(e.to_string());
                            }
                        }
                    }
                    Err(e) => {
                        self.snapshot.managed.phase = "failed".into();
                        self.snapshot.managed.error = Some(e.to_string());
                    }
                }
            }
        }
        if let Some(child) = &mut self.child {
            if self
                .stop_at
                .is_some_and(|at| at.elapsed() > Duration::from_secs(3))
            {
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    self.snapshot.managed.phase = if self.stop_at.is_some() {
                        "stopped"
                    } else if status.success() {
                        "completed"
                    } else {
                        "failed"
                    }
                    .into();
                    if !status.success() && self.stop_at.is_none() {
                        self.snapshot.managed.error = Some(format!(
                            "capture exited {status}; consult local capture log"
                        ));
                    }
                    self.snapshot.managed.pid = None;
                    self.child = None;
                    self.stop_at = None;
                }
                Ok(None) => {}
                Err(e) => {
                    self.snapshot.managed.error = Some(e.to_string());
                }
            }
        }
        self.snapshot.at = now();
    }
}
use std::os::unix::fs::OpenOptionsExt;
fn runtime() -> PathBuf {
    PathBuf::from(
        env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() })),
    )
    .join("daimon-integrators")
}
fn observation_path() -> PathBuf {
    env::var_os("INTEGRATOR_OBSERVATION_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/run/daimon-integrators/observed.json"))
}
fn devices() -> Devices {
    read_json(Path::new("/etc/daimon-integrators/devices.json")).unwrap_or_default()
}
fn control(request: Request) -> Result<Value, Box<dyn std::error::Error>> {
    let mut s = UnixStream::connect(runtime().join("control.sock"))?;
    s.set_read_timeout(Some(Duration::from_secs(4)))?;
    s.write_all(&serde_json::to_vec(&request)?)?;
    s.shutdown(std::net::Shutdown::Write)?;
    let mut data = String::new();
    s.take(65536).read_to_string(&mut data)?;
    Ok(serde_json::from_str(&data)?)
}
#[derive(Default, Clone)]
struct PipeState {
    ok: bool,
    microphone: bool,
    camera: bool,
}
fn pipewire_monitor() -> Arc<Mutex<PipeState>> {
    let result = Arc::new(Mutex::new(PipeState::default()));
    let shared = result.clone();
    thread::spawn(move || loop {
        let child = Command::new("pw-dump")
            .args(["--monitor", "--no-colors"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        if let Ok(mut child) = child {
            let output = child.stdout.take().unwrap();
            let reader = std::io::BufReader::new(output);
            let mut nodes = std::collections::BTreeMap::<u64, Value>::new();
            for update in serde_json::Deserializer::from_reader(reader).into_iter::<Value>() {
                let Ok(Value::Array(batch)) = update else {
                    break;
                };
                for patch in batch {
                    let Some(id) = patch["id"].as_u64() else {
                        continue;
                    };
                    if patch.get("info") == Some(&Value::Null) {
                        nodes.remove(&id);
                    } else if let Some(node) = nodes.get_mut(&id) {
                        merge_graph_value(node, patch);
                    } else {
                        nodes.insert(id, patch);
                    }
                }
                if nodes.len() > 4096 {
                    break;
                }
                let graph = Value::Array(nodes.values().cloned().collect());
                let (microphone, camera) = pipewire_activity(&graph);
                *shared.lock().unwrap() = PipeState {
                    ok: true,
                    microphone,
                    camera,
                };
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        *shared.lock().unwrap() = PipeState::default();
        thread::sleep(Duration::from_secs(3));
    });
    result
}
fn serve(engine: Arc<Mutex<Engine>>) -> std::io::Result<()> {
    let dir = runtime();
    fs::create_dir_all(&dir)?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    let socket = dir.join("control.sock");
    if UnixStream::connect(&socket).is_ok() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "panel already running",
        ));
    }
    let _ = fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket)?;
    fs::set_permissions(socket, fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let pipewire = pipewire_monitor();
    thread::spawn(move || {
        let mut last_written = Vec::new();
        let mut observed = Instant::now() - Duration::from_secs(5);
        loop {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
                let mut data = String::new();
                let result = match (&mut stream).take(8192).read_to_string(&mut data) {
                    Ok(_) => match serde_json::from_str(&data) {
                        Ok(r) => engine.lock().unwrap().request(r),
                        Err(_) => json!({"ok":false,"error":"invalid request"}),
                    },
                    Err(_) => json!({"ok":false,"error":"request timed out"}),
                };
                let _ = stream.write_all(result.to_string().as_bytes());
            }
            if observed.elapsed() > Duration::from_secs(1) {
                let obs = read_json(&observation_path()).unwrap_or_default();
                let pipe = pipewire.lock().unwrap().clone();
                let remote: Option<Value> = read_json(&dir.join("activity.json"));
                let remote = remote.filter(|v| {
                    v["at"]
                        .as_u64()
                        .is_some_and(|at| now().saturating_sub(at) <= 15 && at <= now() + 1)
                });
                let mut e = engine.lock().unwrap();
                e.snapshot.observation = obs;
                e.snapshot.pipewire_ok = pipe.ok;
                e.snapshot.pipewire_microphone = pipe.microphone;
                e.snapshot.pipewire_camera = pipe.camera;
                e.snapshot.remote_activity = remote;
                observed = Instant::now();
            }
            let mut e = engine.lock().unwrap();
            e.tick();
            let serialized = serde_json::to_vec(&e.snapshot).unwrap_or_default();
            if serialized != last_written {
                let _ = atomic_json(&dir.join("status.json"), &e.snapshot);
                last_written = serialized;
            }
            drop(e);
            thread::sleep(Duration::from_millis(100));
        }
    });
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("observe") => {
            let path = observation_path();
            fs::create_dir_all(path.parent().ok_or("observation path requires a parent")?)?;
            loop {
                atomic_json(&path, &observe(&devices().camera))?;
                thread::sleep(Duration::from_secs(1));
            }
        }
        Some("status") => println!(
            "{}",
            serde_json::to_string_pretty(&control(Request::Status)?)?
        ),
        Some("stop") => println!("{}", control(Request::Stop)?),
        Some("view") if args.len() == 3 && ["compact", "expanded"].contains(&args[2].as_str()) => {
            println!(
                "{}",
                control(Request::View {
                    compact: args[2] == "compact"
                })?
            )
        }
        Some("capture") if args.len() == 5 => {
            let req = Request::Capture {
                mode: args[2].clone(),
                seconds: args[3].parse()?,
                output: args[4].clone(),
            };
            let response = control(req)?;
            println!("{response}");
            if response["ok"] != true {
                std::process::exit(1);
            }
        }
        Some("panel") => {
            #[cfg(feature = "ui")]
            {
                let ready = Arc::new(AtomicBool::new(false));
                let engine = Arc::new(Mutex::new(Engine::new(devices(), ready.clone())));
                serve(engine.clone())?;
                ui::run(engine, ready)?;
            }
            #[cfg(not(feature = "ui"))]
            return Err("built without UI".into());
        }
        _ => {
            eprintln!("Usage: daimon-integrators panel|observe|status|stop|capture MODE SECONDS ABSOLUTE_OUTPUT");
            std::process::exit(2);
        }
    }
    Ok(())
}

#[cfg(test)]
mod engine_tests {
    use super::*;
    #[test]
    fn no_capture_if_ui_cannot_acknowledge_preparing() {
        let mut e = Engine::new(Devices::default(), Arc::new(AtomicBool::new(false)));
        e.pending = Some((
            "both".into(),
            8,
            "/nonexistent/panel-test.mp4".into(),
            Instant::now() - Duration::from_secs(6),
        ));
        e.snapshot.managed.phase = "preparing".into();
        e.tick();
        assert_eq!(e.snapshot.managed.phase, "failed");
        assert!(e.child.is_none());
    }
    #[test]
    fn stop_reaps_only_our_managed_process_group() {
        let mut unrelated = Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        let child = Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        let mut e = Engine::new(Devices::default(), Arc::new(AtomicBool::new(false)));
        e.child = Some(child);
        e.snapshot.managed.phase = "capturing".into();
        assert_eq!(e.request(Request::Stop)["stage"], "stop_requested");
        for _ in 0..40 {
            e.tick();
            if e.child.is_none() {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(e.child.is_none());
        assert_eq!(e.snapshot.managed.phase, "stopped");
        assert!(unrelated.try_wait().unwrap().is_none());
        unrelated.kill().unwrap();
        unrelated.wait().unwrap();
        assert_eq!(e.request(Request::Stop)["stage"], "no_managed_capture");
    }
}
