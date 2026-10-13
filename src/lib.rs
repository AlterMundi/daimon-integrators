pub mod system;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Observation {
    pub at: u64,
    pub camera_present: bool,
    pub camera_open: bool,
    pub microphone_present: bool,
    pub microphone_running: bool,
    pub complete: bool,
    pub camera_users: Vec<String>,
    pub microphone_users: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Devices {
    pub camera: String,
    pub audio: String,
}
impl Default for Devices {
    fn default() -> Self {
        Self {
            camera: "/dev/video0".into(),
            audio: "plughw:CARD=USB,DEV=0".into(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Managed {
    #[serde(default)]
    pub activity: String,
    pub mode: Option<String>,
    pub phase: String,
    pub started_at: u64,
    pub pid: Option<u32>,
    pub output: Option<String>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Snapshot {
    pub compact: bool,
    pub ui_compact: Option<bool>,
    pub at: u64,
    pub observation: Observation,
    pub pipewire_ok: bool,
    pub pipewire_microphone: bool,
    pub pipewire_camera: bool,
    pub managed: Managed,
    pub remote_activity: Option<Value>,
    #[serde(default)]
    pub system: system::SystemStatus,
}

pub fn observation_fresh(at: u64, time: u64) -> bool {
    at > 0 && at <= time.saturating_add(1) && time.saturating_sub(at) <= 4
}

pub fn sensor_state(
    present: bool,
    active: bool,
    complete: bool,
    at: u64,
    time: u64,
) -> &'static str {
    if !observation_fresh(at, time) {
        return "Desconocido · sin observación reciente";
    }
    if !present {
        return "Desconectado";
    }
    if active {
        return "En uso";
    }
    if complete {
        "Sin uso detectado"
    } else {
        "Desconocido · observación incompleta"
    }
}

pub fn pipewire_activity(nodes: &Value) -> (bool, bool) {
    let Some(nodes) = nodes.as_array() else {
        return (false, false);
    };
    let mut microphone = false;
    let mut camera = false;
    for n in nodes {
        if n["type"].as_str() != Some("PipeWire:Interface:Node")
            || n["info"]["state"].as_str() != Some("running")
        {
            continue;
        }
        let class = n["info"]["props"]["media.class"].as_str().unwrap_or("");
        // A running physical source indicates signal flow; it does not establish recording.
        if class == "Audio/Source" {
            microphone = true;
        }
        if class == "Video/Source" {
            camera = true;
        }
    }
    (microphone, camera)
}

pub fn read_json<T: for<'a> Deserialize<'a>>(path: &Path) -> Option<T> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}
pub fn atomic_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    let tmp = path.with_extension("new");
    fs::write(&tmp, serde_json::to_vec(value)?)?;
    fs::rename(tmp, path)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Request {
    Status,
    Stop,
    Input {
        seconds: u32,
        #[serde(default = "mouse_mode")]
        mode: String,
    },
    View {
        compact: bool,
    },
    Capture {
        mode: String,
        seconds: u32,
        output: String,
    },
}
pub fn validate_capture(mode: &str, seconds: u32, output: &str) -> Result<(), &'static str> {
    if !["audio", "video", "both"].contains(&mode) {
        return Err("mode must be audio, video or both");
    }
    if !(1..=120).contains(&seconds) {
        return Err("capture must last 1–120 seconds");
    }
    if !Path::new(output).is_absolute() {
        return Err("output must be an absolute local path");
    }
    if Path::new(output).exists() {
        return Err("output already exists; refusing overwrite");
    }
    Ok(())
}

pub fn alsa_card_directory(name: &str) -> bool {
    name.strip_prefix("card")
        .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()))
}

pub fn observe(camera: &str) -> Observation {
    let mut o = Observation {
        at: now(),
        complete: true,
        camera_present: Path::new(camera).exists(),
        ..Default::default()
    };
    let camera_path = fs::canonicalize(camera).ok();
    // Only examine FD target names and ALSA state. Never read camera/audio samples.
    if let Ok(processes) = fs::read_dir("/proc") {
        for entry in processes.flatten() {
            if entry.file_name().to_string_lossy().parse::<u32>().is_err() {
                continue;
            }
            let p = entry.path();
            match fs::read_dir(p.join("fd")) {
                Ok(fds) => {
                    for fd in fds.flatten() {
                        match fs::read_link(fd.path()) {
                            Ok(target) => {
                                if camera_path.as_ref() == Some(&target) {
                                    o.camera_open = true;
                                    let name = fs::read_to_string(p.join("comm"))
                                        .unwrap_or_else(|_| "unknown".into());
                                    o.camera_users.push(name.trim().to_owned());
                                }
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                                o.complete = false
                            }
                            Err(_) => {}
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => o.complete = false,
                Err(_) => {}
            }
        }
    } else {
        o.complete = false;
    }
    if let Ok(cards) = fs::read_dir("/proc/asound") {
        for card in cards.flatten() {
            let name = card.file_name();
            let name = name.to_string_lossy();
            if !alsa_card_directory(&name) {
                continue;
            }
            let Ok(pcms) = fs::read_dir(card.path()) else {
                o.complete = false;
                continue;
            };
            for pcm in pcms.flatten() {
                if !pcm.file_name().to_string_lossy().ends_with('c') {
                    continue;
                }
                o.microphone_present = true;
                let Ok(subs) = fs::read_dir(pcm.path()) else {
                    o.complete = false;
                    continue;
                };
                for sub in subs.flatten() {
                    if !sub.file_name().to_string_lossy().starts_with("sub") {
                        continue;
                    }
                    match fs::read_to_string(sub.path().join("status")) {
                        Ok(s) => {
                            if s.contains("state: RUNNING") || s.contains("state: DRAINING") {
                                o.microphone_running = true;
                                o.microphone_users.push(name.to_string());
                            }
                        }
                        Err(_) => o.complete = false,
                    }
                }
            }
        }
    } else {
        o.complete = false;
    }
    o.camera_users.sort();
    o.camera_users.dedup();
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_and_incomplete_are_not_inactive() {
        assert!(sensor_state(true, false, true, 10, 16).starts_with("Desconocido"));
        assert!(!observation_fresh(99, 10));
        assert!(!observation_fresh(0, 10));
        assert!(sensor_state(true, false, false, 10, 10).starts_with("Desconocido"));
        assert_eq!(sensor_state(true, false, true, 10, 10), "Sin uso detectado");
        assert!(sensor_state(true, true, false, 10, 10).starts_with("En uso"));
        assert_eq!(sensor_state(false, false, true, 10, 10), "Desconectado");
    }
    #[test]
    fn pipewire_playback_and_idle_sources_do_not_count_as_capture() {
        let graph = serde_json::json!([
            {"type":"PipeWire:Interface:Node","info":{"state":"running","props":{"media.class":"Audio/Sink"}}},
            {"type":"PipeWire:Interface:Node","info":{"state":"idle","props":{"media.class":"Audio/Source"}}}
        ]);
        assert_eq!(pipewire_activity(&graph), (false, false));
        let source = serde_json::json!([{"type":"PipeWire:Interface:Node","info":{"state":"running","props":{"media.class":"Audio/Source"}}}]);
        assert_eq!(pipewire_activity(&source), (true, false));
    }
    #[test]
    fn captures_are_bounded_and_do_not_overwrite() {
        assert!(validate_capture("both", 8, "/nonexistent/integrator-test.mp4").is_ok());
        assert!(validate_capture("both", 121, "/nonexistent/integrator-test.mp4").is_err());
        assert!(validate_capture("both", 0, "/nonexistent/integrator-test.mp4").is_err());
        assert!(validate_capture("other", 8, "/nonexistent/integrator-test.mp4").is_err());
        assert!(validate_capture("audio", 8, "relative.wav").is_err());
        assert!(validate_capture("audio", 8, "/etc/passwd").is_err());
    }
}

/// Merge incremental pw-dump monitor updates without dropping unchanged props.
pub fn merge_graph_value(target: &mut Value, patch: Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                if let Some(prior) = target.get_mut(&key) {
                    merge_graph_value(prior, value);
                } else {
                    target.insert(key, value);
                }
            }
        }
        (target, patch) => *target = patch,
    }
}
#[cfg(test)]
mod graph_tests {
    use super::*;
    #[test]
    fn state_updates_preserve_source_class_and_can_clear_flow() {
        let mut node = serde_json::json!({"type":"PipeWire:Interface:Node","info":{"state":"running","props":{"media.class":"Audio/Source"}}});
        assert_eq!(
            pipewire_activity(&serde_json::json!([node.clone()])),
            (true, false)
        );
        merge_graph_value(&mut node, serde_json::json!({"info":{"state":"idle"}}));
        assert_eq!(node["info"]["props"]["media.class"], "Audio/Source");
        assert_eq!(
            pipewire_activity(&serde_json::json!([node])),
            (false, false)
        );
    }
}

#[cfg(test)]
mod alsa_tests {
    use super::*;
    #[test]
    fn ignore_alsa_summary_files_when_enumerating_card_directories() {
        assert!(alsa_card_directory("card0"));
        assert!(alsa_card_directory("card12"));
        assert!(!alsa_card_directory("cards"));
        assert!(!alsa_card_directory("card"));
        assert!(!alsa_card_directory("pcm"));
    }
}

fn mouse_mode() -> String {
    "mouse".into()
}
