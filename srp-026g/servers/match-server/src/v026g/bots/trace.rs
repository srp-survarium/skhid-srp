//! The decision trace (`player_bots_research.md` B8 point 6): one JSON line
//! per bot when its play changes and every `trace_every_ms`, with the
//! time, the pose, the play and its step, the top candidates with each
//! consideration's input and result, the reason, the team's modifiers, the
//! skill and the tuning's hash. One line answers "why did it do that".
//!
//! Written to `<SRP_BOT_TRACE_DIR>/bots_<match>.jsonl` when that is set;
//! otherwise nothing is kept (the log has the play changes). The traces
//! over `SRP_BOT_TRACE_MAX_MB` (default [`DEFAULT_MAX_MB`]) together go,
//! the oldest first, when a match opens its own ([`prune`]).

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// `SRP_BOT_TRACE_MAX_MB` when unset: a match of 19 bots writes some tens
/// of MB.
pub const DEFAULT_MAX_MB: u32 = 2_000;

pub struct Trace {
    out: Option<BufWriter<File>>,
    pub tuning_hash: u32,
}

impl Trace {
    pub fn none(tuning_hash: u32) -> Self {
        Self {
            out: None,
            tuning_hash,
        }
    }

    /// A trace file for match `match_id` in `dir`; none if it cannot be
    /// made (logged once). The older traces there are cut to `max_bytes`
    /// together first ([`prune`]).
    pub fn open(dir: &Path, match_id: u32, tuning_hash: u32, max_bytes: u64) -> Self {
        prune(dir, max_bytes);
        let path = dir.join(format!("bots_{match_id:08x}.jsonl"));
        let out = std::fs::create_dir_all(dir)
            .and_then(|()| File::create(&path))
            .map(BufWriter::new)
            .map_err(|e| log::warn!("bot trace {}: {e}", path.display()))
            .ok();
        Self { out, tuning_hash }
    }

    pub fn enabled(&self) -> bool {
        self.out.is_some()
    }

    pub fn line(&mut self, mut value: serde_json::Value) {
        let Some(out) = self.out.as_mut() else {
            return;
        };
        if let Some(object) = value.as_object_mut() {
            object.insert("tuning".into(), format!("{:08x}", self.tuning_hash).into());
        }
        if writeln!(out, "{value}").is_err() {
            self.out = None;
        }
    }
}

impl Drop for Trace {
    fn drop(&mut self) {
        if let Some(out) = self.out.as_mut() {
            let _ = out.flush();
        }
    }
}

/// The traces in `dir` (`bots_*.jsonl`) go, the least recently written
/// first, until the rest are `max_bytes` together at most.
pub fn prune(dir: &Path, max_bytes: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut traces: Vec<_> = entries
        .flatten()
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("bots_") && name.ends_with(".jsonl")
        })
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            Some((meta.modified().ok()?, entry.path(), meta.len()))
        })
        .collect();
    traces.sort();
    let mut total: u64 = traces.iter().map(|(_, _, size)| size).sum();
    for (_, path, size) in traces {
        if total <= max_bytes {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            log::info!(
                "Bot traces: over {} MB together; {} deleted",
                max_bytes >> 20,
                path.display()
            );
            total -= size;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_oldest_traces_go_over_the_limit() {
        let dir = std::env::temp_dir().join(format!("srp-trace-prune-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let now = std::time::SystemTime::now();
        for (index, name) in ["bots_00000003.jsonl", "bots_00000001.jsonl", "bots_00000002.jsonl"]
            .iter()
            .enumerate()
        {
            let path = dir.join(name);
            std::fs::write(&path, [b'x'; 100]).unwrap();
            let at = now - std::time::Duration::from_secs(100 - index as u64 * 10);
            File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(at)
                .unwrap();
        }
        // Not a trace: kept whatever its size.
        std::fs::write(dir.join("other.txt"), [b'x'; 1000]).unwrap();
        prune(&dir, 250);
        let left = |name: &str| dir.join(name).exists();
        assert!(!left("bots_00000003.jsonl"));
        assert!(left("bots_00000001.jsonl") && left("bots_00000002.jsonl"));
        assert!(left("other.txt"));
        // A new match's trace is opened after the cut.
        let mut trace = Trace::open(&dir, 4, 0, 150);
        trace.line(serde_json::json!({"a": 1}));
        drop(trace);
        assert!(!left("bots_00000001.jsonl") && left("bots_00000002.jsonl"));
        assert!(left("bots_00000004.jsonl"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
