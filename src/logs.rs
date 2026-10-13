//! Retain only this owner's numbered input logs; never recordings or symlinks.
use std::{fs, io, os::unix::fs::MetadataExt, path::Path};
pub const INPUT_LOGS: usize = 20;
pub fn retain(directory: &Path, current: &Path, limit: usize) -> io::Result<usize> {
    if limit == 0 {
        return Err(io::Error::other(
            "input retention must preserve the current log",
        ));
    }
    let mut logs = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(number) = name
            .strip_prefix("input-")
            .and_then(|n| n.strip_suffix(".log"))
            .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|n| n.parse::<u64>().ok())
        else {
            continue;
        };
        if entry.metadata()?.uid() != unsafe { libc::getuid() } {
            continue;
        }
        let path = entry.path();
        logs.push((path == current, number, path));
    }
    logs.sort_by_key(|a| (a.0, a.1));
    let remove = logs.len().saturating_sub(limit);
    for (_, _, path) in logs.into_iter().take(remove) {
        fs::remove_file(path)?;
    }
    Ok(remove)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_current_despite_clock_regression_and_preserves_unrelated_paths() {
        let dir = std::env::temp_dir().join(format!("integrator-retention-{}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        for n in 1..=25 {
            fs::write(dir.join(format!("input-{n}.log")), b"summary").unwrap();
        }
        for name in [
            "capture.log",
            "clip.mp4",
            "input-archive.log",
            "input-+100.log",
        ] {
            fs::write(dir.join(name), b"preserve").unwrap();
        }
        fs::create_dir(dir.join("input-100.log")).unwrap();
        std::os::unix::fs::symlink("clip.mp4", dir.join("input-200.log")).unwrap();
        assert_eq!(retain(&dir, &dir.join("input-1.log"), 20).unwrap(), 5);
        assert!(dir.join("input-1.log").exists());
        assert!(!dir.join("input-2.log").exists());
        assert!(dir.join("input-25.log").exists());
        for name in [
            "capture.log",
            "clip.mp4",
            "input-archive.log",
            "input-+100.log",
            "input-100.log",
            "input-200.log",
        ] {
            assert!(dir.join(name).exists());
        }
        assert_eq!(retain(&dir, &dir.join("input-1.log"), 20).unwrap(), 0);
        assert!(retain(&dir, &dir.join("input-1.log"), 0).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
