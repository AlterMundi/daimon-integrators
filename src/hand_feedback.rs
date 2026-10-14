//! Bounded metadata-only local feedback. Never opens sensors or an input device.
use serde::Deserialize;
use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpStream},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Deserialize)]
pub struct CalibrationState {
    pub stage: String,
    pub samples: usize,
    pub corner_target: Option<[f64; 2]>,
    pub corners_verified: bool,
    pub message: String,
    pub output: [u32; 2],
}
#[derive(Clone, Debug, Deserialize)]
pub struct Guidance {
    pub step: usize,
    pub phase: String,
    pub remaining_ms: Option<u64>,
    pub instruction: String,
    pub result: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Frame {
    pub sequence: u32,
    #[serde(default)]
    pub guided: Option<Guidance>,
    #[serde(default)]
    pub input_enabled: Option<bool>,
    #[serde(default)]
    pub screen_target: Option<[f64; 2]>,
    #[serde(default)]
    pub calibration: Option<CalibrationState>,
    pub width: f64,
    pub height: f64,
    pub landmarks: Option<Vec<[f64; 2]>>,
    pub fresh: bool,
    pub age_ms: f64,
    pub pinch_state: String,
    #[serde(default, deserialize_with = "nullable_reason")]
    pub blocked_reason: String,
    pub presses: u64,
    pub test_phase: String,
    pub remaining_ms: Option<u64>,
}
fn nullable_reason<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}
impl Frame {
    pub fn parse(body: &[u8]) -> io::Result<Self> {
        let frame: Self = serde_json::from_slice(body)?;
        if frame.guided.as_ref().is_some_and(|g| {
            g.step > 5
                || !matches!(
                    g.phase.as_str(),
                    "waiting" | "move" | "hold" | "complete" | "retry"
                )
                || g.remaining_ms.is_some_and(|n| n > 20_000)
                || g.instruction.len() > 128
                || g.result.as_ref().is_some_and(|s| s.len() > 256)
        }) {
            return Err(io::Error::other("invalid guided feedback"));
        }
        if frame.screen_target.is_some_and(|p| !valid_target(p))
            || frame.calibration.as_ref().is_some_and(|c| {
                !matches!(
                    c.stage.as_str(),
                    "left"
                        | "right"
                        | "top"
                        | "bottom"
                        | "upper_left"
                        | "lower_right"
                        | "review"
                        | "active"
                        | "uncalibrated"
                ) || c.samples > 24
                    || c.message.len() > 512
                    || c.output.iter().any(|n| !(1..=8192).contains(n))
                    || c.corner_target.is_some_and(|p| !valid_target(p))
            })
        {
            return Err(io::Error::other("invalid calibration feedback"));
        }
        if !(1.0..=8192.0).contains(&frame.width)
            || !(1.0..=8192.0).contains(&frame.height)
            || !frame.age_ms.is_finite()
            || frame.age_ms < 0.0
            || frame.remaining_ms.is_some_and(|ms| ms > 120_000)
            || frame.pinch_state.len() > 64
            || frame.blocked_reason.len() > 64
            || !["preparing", "testing"].contains(&frame.test_phase.as_str())
            || frame.landmarks.as_ref().is_some_and(|p| {
                p.len() != 21
                    || p.iter()
                        .any(|v| v.iter().any(|n| !n.is_finite() || n.abs() > 32768.0))
            })
        {
            return Err(io::Error::other("invalid hand feedback"));
        }
        Ok(frame)
    }
    pub fn visible(&self, received: Instant, now: Instant) -> bool {
        self.fresh
            && self.age_ms + now.saturating_duration_since(received).as_secs_f64() * 1000.0 < 150.0
    }
    // Visual continuity may retain an aged hand, with delayed coloring, while
    // input freshness and press indication keep their stricter 150 ms bound.
    pub fn renderable(&self, received: Instant, now: Instant) -> bool {
        self.age_ms + now.saturating_duration_since(received).as_secs_f64() * 1000.0 < 300.0
    }
    // Mirrored camera coordinates, independent of the relative system pointer.
    pub fn screen_point(&self, point: [f64; 2], width: f64, height: f64) -> [f64; 2] {
        [
            (1.0 - point[0] / self.width) * width,
            point[1] / self.height * height,
        ]
    }
}
/// Recognition-event feedback, independent of button hold and input freshness.
#[derive(Default)]
pub struct PinchPulse {
    seen: Option<u64>,
    pulse: Option<([f64; 2], Instant)>,
}
impl PinchPulse {
    pub fn observe(&mut self, presses: u64, origin: [f64; 2], now: Instant) {
        if self.seen.is_some_and(|last| presses > last) {
            self.pulse = Some((origin, now));
        } else if self.seen.is_some_and(|last| presses < last) {
            self.pulse = None;
        }
        self.seen = Some(presses);
    }
    pub fn state(&self, now: Instant) -> Option<([f64; 2], f64, f64)> {
        let (origin, at) = self.pulse?;
        let progress = now.saturating_duration_since(at).as_secs_f64() / 0.7;
        if progress >= 1.0 {
            return None;
        }
        Some((origin, 18.0 + 72.0 * progress, 1.0 - progress))
    }
}
/// Retain at most one metadata skeleton. Never refresh its timestamp or press
/// state when the incoming observation is missing; input uses the live frame.
pub fn retain_hand(
    last: &mut Option<(Frame, Instant)>,
    current: &Frame,
    received: Instant,
) -> Option<(Frame, Instant)> {
    if current.landmarks.is_some()
        && last
            .as_ref()
            .is_none_or(|(old, _)| old.sequence != current.sequence)
    {
        *last = Some((current.clone(), received));
    }
    last.clone()
}
fn valid_target(p: [f64; 2]) -> bool {
    p.iter().all(|n| n.is_finite() && (0.0..=1.0).contains(n))
}
pub fn request(port: u16, method: &str, path: &str) -> io::Result<Vec<u8>> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(200))?;
    stream.set_read_timeout(Some(Duration::from_millis(250)))?;
    stream.set_write_timeout(Some(Duration::from_millis(250)))?;
    write!(stream, "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")?;
    let mut response = Vec::new();
    stream.take(16385).read_to_end(&mut response)?;
    decode_response(&response)
}
fn decode_response(response: &[u8]) -> io::Result<Vec<u8>> {
    if response.len() > 16384 {
        return Err(io::Error::other("oversized feedback response"));
    }
    let split = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| io::Error::other("missing HTTP headers"))?;
    let headers = std::str::from_utf8(&response[..split]).map_err(io::Error::other)?;
    if !headers.starts_with("HTTP/1.1 200 ") {
        return Err(io::Error::other("feedback not ready"));
    }
    let length: usize = headers
        .lines()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                .map(|(_, v)| v.trim().parse())
        })
        .ok_or_else(|| io::Error::other("missing length"))?
        .map_err(io::Error::other)?;
    let body = &response[split + 4..];
    if body.len() != length || body.len() > 8192 {
        return Err(io::Error::other("invalid feedback length"));
    }
    Ok(body.to_vec())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame() -> Frame {
        Frame::parse(br#"{"sequence":1,"width":640,"height":480,"landmarks":null,"fresh":true,"age_ms":50,"pinch_state":"open","blocked_reason":"close_threshold","presses":0,"test_phase":"testing","remaining_ms":1000}"#).unwrap()
    }
    #[test]
    fn pinch_pulse_triggers_once_per_counter_increase_and_expires_after_release() {
        let at = Instant::now();
        let mut pulse = PinchPulse::default();
        pulse.observe(2, [10.0, 20.0], at);
        assert!(pulse.state(at).is_none());
        pulse.observe(3, [30.0, 40.0], at);
        let initial = pulse.state(at).unwrap();
        assert_eq!(initial.0, [30.0, 40.0]);
        pulse.observe(3, [90.0, 90.0], at + Duration::from_millis(100));
        let later = pulse.state(at + Duration::from_millis(350)).unwrap();
        assert_eq!(later.0, initial.0);
        assert!(later.1 > initial.1);
        assert!(later.2 < initial.2);
        assert!(pulse.state(at + Duration::from_millis(699)).is_some());
        assert!(pulse.state(at + Duration::from_millis(700)).is_none());
        pulse.observe(0, [10.0, 20.0], at + Duration::from_secs(1));
        assert!(pulse.state(at + Duration::from_secs(1)).is_none());
    }
    #[test]
    fn missing_geometry_retains_last_hand_without_refreshing_age_or_input() {
        let at = Instant::now();
        let mut f = frame();
        f.landmarks = Some(vec![[1.0, 2.0]; 21]);
        let mut last = None;
        let held = retain_hand(&mut last, &f, at).unwrap();
        let mut missing = f.clone();
        missing.sequence += 1;
        missing.landmarks = None;
        missing.fresh = false;
        let retained = retain_hand(&mut last, &missing, at + Duration::from_secs(1)).unwrap();
        assert_eq!(retained.0.sequence, held.0.sequence);
        assert_eq!(retained.1, at);
        assert!(!retained.0.visible(retained.1, at + Duration::from_secs(1)));
        assert!(missing.landmarks.is_none());
        assert!(!missing.visible(at, at));
        let mut next = f.clone();
        next.sequence += 2;
        assert_eq!(
            retain_hand(&mut last, &next, at + Duration::from_secs(2))
                .unwrap()
                .0
                .sequence,
            next.sequence
        );
    }
    #[test]
    fn pressed_feedback_accepts_no_block_reason_and_guided_bounds_are_checked() {
        let mut value = serde_json::json!({"sequence":1,"width":640,"height":480,"landmarks":null,"fresh":true,"age_ms":50,"pinch_state":"pressed","blocked_reason":null,"presses":1,"test_phase":"testing","remaining_ms":1000,
            "guided":{"step":0,"phase":"waiting","remaining_ms":null,"instruction":"Move upper left","result":null}});
        let f = Frame::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(f.pinch_state, "pressed");
        assert!(f.blocked_reason.is_empty());
        assert_eq!(f.guided.unwrap().phase, "waiting");
        value["guided"]["step"] = 6.into();
        assert!(Frame::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    #[test]
    fn stale_feedback_cannot_keep_a_hand_visible() {
        let frame = frame();
        let at = Instant::now();
        assert!(frame.visible(at, at + Duration::from_millis(99)));
        assert!(!frame.visible(at, at + Duration::from_millis(100)));
        assert_eq!(
            frame.screen_point([0.0, 480.0], 1920.0, 1080.0),
            [1920.0, 1080.0]
        );
    }
    #[test]
    fn malformed_and_unbounded_feedback_is_rejected() {
        let mut value=serde_json::to_value(serde_json::json!({"sequence":1,"width":640,"height":480,"landmarks":[[1,2]],"fresh":true,"age_ms":50,"pinch_state":"open","blocked_reason":"","presses":0,"test_phase":"testing","remaining_ms":1000})).unwrap();
        assert!(Frame::parse(&serde_json::to_vec(&value).unwrap()).is_err());
        value["landmarks"] = serde_json::Value::Null;
        value["remaining_ms"] = 120001.into();
        assert!(Frame::parse(&serde_json::to_vec(&value).unwrap()).is_err());
        assert!(decode_response(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\n{}").is_err());
        assert!(decode_response(&vec![0; 16385]).is_err());
        assert_eq!(
            decode_response(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}").unwrap(),
            b"{}"
        );
    }
    #[test]
    fn visual_retention_does_not_extend_input_or_accept_invalid_screen_target() {
        let mut f = frame();
        let at = Instant::now();
        assert!(!f.visible(at, at + Duration::from_millis(150)));
        assert!(f.renderable(at, at + Duration::from_millis(150)));
        assert!(!f.renderable(at, at + Duration::from_millis(250)));
        f.fresh = false;
        assert!(!f.visible(at, at));
        assert!(f.renderable(at, at));
        let bytes=br#"{"sequence":1,"width":640,"height":480,"landmarks":null,"fresh":true,"age_ms":0,"pinch_state":"open","blocked_reason":"","presses":0,"test_phase":"testing","remaining_ms":1000,"screen_target":[1.1,0.5]}"#;
        assert!(Frame::parse(bytes).is_err());
    }
}
