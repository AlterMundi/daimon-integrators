//! Local resource measurements and fixed receiving power profiles.
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SystemStatus {
    pub at: u64,
    pub cpu_percent: Option<f64>,
    pub temperature_c: Option<f64>,
    pub fan_rpm: Option<u64>,
    pub cpu_max_mhz: Option<u64>,
    pub power_mode: String,
}
#[derive(Default)]
pub struct Sampler {
    previous: Option<(u64, u64)>,
}
pub fn cpu_counters(stat: &str) -> Option<(u64, u64)> {
    let line = stat.lines().next()?;
    let mut words = line.split_whitespace();
    if words.next()? != "cpu" {
        return None;
    }
    let v: Vec<u64> = words
        .take(8)
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    if v.len() != 8 {
        return None;
    }
    Some((v.iter().sum(), v[3] + v[4]))
}
pub fn cpu_delta(prior: (u64, u64), next: (u64, u64)) -> Option<f64> {
    let total = next.0.checked_sub(prior.0)?;
    let idle = next.1.checked_sub(prior.1)?;
    if total == 0 || idle > total {
        return None;
    }
    Some(100.0 * (total - idle) as f64 / total as f64)
}
fn number(path: impl AsRef<Path>) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}
fn fan_dir() -> Option<PathBuf> {
    fs::read_dir("/sys/class/hwmon")
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            fs::read_to_string(p.join("name"))
                .ok()
                .is_some_and(|s| s.trim() == "pwmfan")
        })
}
pub fn observed_mode(
    enable: Option<u64>,
    max_mhz: Option<u64>,
    trips: Option<[u64; 4]>,
) -> &'static str {
    match (enable, max_mhz, trips) {
        (Some(0), Some(n), _) if n <= 1500 => "silent",
        (Some(0), _, _) => "fan_off",
        (Some(1..=3), Some(n), Some([60000, 70000, 77000, 80000])) if n <= 2000 => "quiet",
        (Some(1..=3), Some(_), Some([50000, 60000, 67500, 75000])) => "auto",
        (None, _, _) => "unknown",
        _ => "custom",
    }
}
fn trips() -> Option<[u64; 4]> {
    Some([
        number("/sys/class/thermal/thermal_zone0/trip_point_1_temp")?,
        number("/sys/class/thermal/thermal_zone0/trip_point_2_temp")?,
        number("/sys/class/thermal/thermal_zone0/trip_point_3_temp")?,
        number("/sys/class/thermal/thermal_zone0/trip_point_4_temp")?,
    ])
}
impl Sampler {
    pub fn sample(&mut self) -> SystemStatus {
        let next = fs::read_to_string("/proc/stat")
            .ok()
            .and_then(|s| cpu_counters(&s));
        let percent = self.previous.zip(next).and_then(|(a, b)| cpu_delta(a, b));
        self.previous = next;
        let fan = fan_dir();
        let max =
            number("/sys/devices/system/cpu/cpufreq/policy0/scaling_max_freq").map(|v| v / 1000);
        let enable = fan.as_ref().and_then(|p| number(p.join("pwm1_enable")));
        SystemStatus {
            at: crate::now(),
            cpu_percent: percent,
            temperature_c: number("/sys/class/thermal/thermal_zone0/temp")
                .filter(|n| *n <= 150000)
                .map(|n| n as f64 / 1000.0),
            fan_rpm: fan.and_then(|p| number(p.join("fan1_input"))),
            cpu_max_mhz: max,
            power_mode: observed_mode(enable, max, trips()).into(),
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerMode {
    Auto,
    Quiet,
    Silent,
}
impl PowerMode {
    pub fn parse(s: &str) -> io::Result<Self> {
        match s {
            "auto" => Ok(Self::Auto),
            "quiet" => Ok(Self::Quiet),
            "silent" => Ok(Self::Silent),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "mode must be auto, quiet or silent",
            )),
        }
    }
    fn config(self) -> (u64, [u64; 4], u64) {
        match self {
            Self::Auto => (2400000, [50000, 60000, 67500, 75000], 1),
            Self::Quiet => (2000000, [60000, 70000, 77000, 80000], 1),
            Self::Silent => (1500000, [60000, 70000, 77000, 80000], 0),
        }
    }
}
fn error(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, msg)
}
pub fn apply_power(mode: PowerMode, save: bool) -> io::Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "power-apply requires the installed administrator grant",
        ));
    }
    let model = fs::read_to_string("/sys/firmware/devicetree/base/model")?;
    if !model.starts_with("Raspberry Pi 5") {
        return Err(error("power profiles currently qualified for Pi 5 only"));
    }
    let fan = fan_dir().ok_or_else(|| error("PWM fan unavailable"))?;
    let (max, steps, enable) = mode.config();
    let cpu = Path::new("/sys/devices/system/cpu/cpufreq/policy0/scaling_max_freq");
    let prior = serde_json::json!({"cpu_max":number(cpu),"fan_enable":number(fan.join("pwm1_enable")),"trips":trips()});
    fs::create_dir_all("/var/lib/daimon-integrators")?;
    let backup = Path::new("/var/lib/daimon-integrators/power-before.json");
    if !backup.exists() {
        crate::atomic_json(backup, &prior)?;
    }
    // The initial manual experiment used firmware arm_freq; remove only its exact
    // owned block so saved mode restoration can choose all supported frequencies.
    let boot = Path::new("/boot/firmware/config.txt");
    let config = fs::read_to_string(boot)?;
    const BEGIN: &str = "# BEGIN COMPAII QUIET PROFILE";
    const END: &str = "# END COMPAII QUIET PROFILE";
    if let Some(start) = config.find(BEGIN) {
        let end = config[start..]
            .find(END)
            .ok_or_else(|| error("incomplete managed boot block"))?
            + start
            + END.len();
        let path = Path::new("/var/lib/daimon-integrators/config-before-power-mode.txt");
        if !path.exists() {
            fs::write(path, &config)?;
        }
        let mut updated = config.clone();
        updated.replace_range(start..end, "");
        fs::write(boot, updated)?;
    }
    fs::write(cpu, format!("{max}\n"))?;
    for (i, temp) in steps.iter().enumerate() {
        fs::write(
            format!("/sys/class/thermal/thermal_zone0/trip_point_{}_temp", i + 1),
            format!("{temp}\n"),
        )?;
    }
    if enable == 0 {
        fs::write(fan.join("pwm1"), "0\n")?;
    }
    fs::write(fan.join("pwm1_enable"), format!("{enable}\n"))?;
    if enable != 0 {
        // Re-synchronize the fan's physical PWM with its thermal cooling state.
        let cooling = fs::read_dir("/sys/class/thermal")?
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                fs::read_to_string(p.join("type"))
                    .ok()
                    .is_some_and(|s| s.trim() == "pwm-fan")
            })
            .ok_or_else(|| error("fan cooling device missing"))?;
        let state = number(cooling.join("cur_state")).unwrap_or(1);
        fs::write(cooling.join("cur_state"), format!("{state}\n"))?;
    }
    if save {
        fs::create_dir_all("/etc/daimon-integrators")?;
        crate::atomic_json(Path::new("/etc/daimon-integrators/power-mode.json"), &mode)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aggregate_cpu_excludes_duplicate_guest_counters() {
        assert_eq!(
            cpu_counters("cpu 10 20 30 40 50 60 70 80 900 1000\ncpu0 ignored"),
            Some((360, 90))
        );
        assert_eq!(cpu_delta((100, 50), (200, 100)), Some(50.0));
        assert_eq!(cpu_delta((200, 100), (100, 50)), None);
        assert_eq!(cpu_delta((100, 50), (100, 50)), None);
        assert_eq!(cpu_counters("cpu broken"), None);
    }
    #[test]
    fn mode_comes_from_actual_fan_limit_and_thermal_settings() {
        let quiet = Some([60000, 70000, 77000, 80000]);
        assert_eq!(observed_mode(Some(0), Some(1500), quiet), "silent");
        assert_eq!(observed_mode(Some(0), Some(2400), quiet), "fan_off");
        assert_eq!(observed_mode(Some(1), Some(2000), quiet), "quiet");
        assert_eq!(observed_mode(Some(1), Some(2400), quiet), "custom");
        assert_eq!(observed_mode(None, Some(1500), quiet), "unknown");
        assert!(PowerMode::parse("silent").is_ok());
        assert!(PowerMode::parse("../../unsafe").is_err());
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workload {
    pub unit: String,
    pub normal_quota: String,
}
fn valid_unit(unit: &str) -> bool {
    !unit.starts_with('-')
        && unit.ends_with(".service")
        && unit.len() <= 128
        && unit
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.@-".contains(&b))
}
pub fn workload_quota(mode: PowerMode, normal: &str) -> io::Result<String> {
    let normal_value = if normal == "infinity" {
        None
    } else {
        let n: u32 = normal
            .strip_suffix('%')
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| error("invalid normal CPU quota"))?;
        if !(1..=1000).contains(&n) {
            return Err(error("normal CPU quota out of range"));
        }
        Some(n)
    };
    match mode {
        PowerMode::Auto => Ok(normal.into()),
        PowerMode::Silent => Ok(format!("{}%", normal_value.unwrap_or(100).min(100))),
        PowerMode::Quiet => Ok(format!("{}%", normal_value.unwrap_or(200).min(200))),
    }
}
/// Receiving-selected same-user background units only; no discovery or service start.
pub fn apply_workloads(mode: PowerMode) -> io::Result<()> {
    use std::process::Command;
    let path = Path::new("/etc/daimon-integrators/power-workloads.json");
    if !path.exists() {
        return Ok(());
    }
    let workloads: Vec<Workload> = serde_json::from_slice(&fs::read(path)?)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    if workloads.len() > 16 {
        return Err(error("too many background units"));
    }
    for w in &workloads {
        if !valid_unit(&w.unit) {
            return Err(error("invalid background unit"));
        }
        workload_quota(mode, &w.normal_quota)?;
    }
    for w in workloads {
        let quota = workload_quota(mode, &w.normal_quota)?;
        // systemd show reports infinity, but assignment resets through an empty value.
        let value = if quota == "infinity" { "" } else { &quota };
        let status = Command::new("systemctl")
            .args([
                "--user",
                "set-property",
                "--runtime",
                &w.unit,
                &format!("CPUQuota={value}"),
            ])
            .status()?;
        if !status.success() {
            return Err(error(
                "background quota not accepted; host mode was applied",
            ));
        }
    }
    Ok(())
}
#[cfg(test)]
mod workload_tests {
    use super::*;
    #[test]
    fn workload_names_and_baselines_are_bounded() {
        assert!(valid_unit("background-encoder.service"));
        assert!(!valid_unit("--unsafe.service"));
        assert!(!valid_unit("../../other.service"));
        assert_eq!(
            workload_quota(PowerMode::Silent, "infinity").unwrap(),
            "100%"
        );
        assert_eq!(workload_quota(PowerMode::Quiet, "400%").unwrap(), "200%");
        assert_eq!(workload_quota(PowerMode::Auto, "125%").unwrap(), "125%");
        assert!(workload_quota(PowerMode::Auto, "command text").is_err());
        assert_eq!(workload_quota(PowerMode::Quiet, "50%").unwrap(), "50%");
    }
}
