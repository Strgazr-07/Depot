use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Recursively sum the size (in bytes) of every file under `path`.
/// Symlinks are not followed, and unreadable entries are skipped so a
/// permission error deep in the tree never aborts the whole measurement.
pub fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    for entry in walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file() {
            if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

/// Last-modified time of `path` itself (cheap, non-recursive).
pub fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

/// The more recent of two optional timestamps.
pub fn newest(a: Option<SystemTime>, b: Option<SystemTime>) -> Option<SystemTime> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

/// Seconds since the Unix epoch, for stable JSON output.
pub fn unix_secs(t: SystemTime) -> Option<u64> {
    t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// Human-readable byte size, e.g. `1.5 GB`.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if bytes == 0 {
        return "0 B".to_string();
    }
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{:.1} {}", size, UNITS[unit])
    }
}

/// Format a modification time as a coarse relative age, e.g. `12d ago`.
pub fn relative_age(t: Option<SystemTime>) -> String {
    let Some(t) = t else {
        return "—".to_string();
    };
    let Ok(elapsed) = t.elapsed() else {
        return "—".to_string();
    };
    let secs = elapsed.as_secs();
    let days = secs / 86_400;
    if days >= 1 {
        format!("{}d ago", days)
    } else if secs / 3_600 >= 1 {
        format!("{}h ago", secs / 3_600)
    } else {
        "today".to_string()
    }
}
