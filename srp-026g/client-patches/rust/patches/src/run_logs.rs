//! Patch 147: every run of the game writes its logs to files of its own,
//! named by the run's start (docs/notes/run-logs.md).
//!
//! The run starts at the process's first hook (`memory.allocate_arenas`):
//! its stamp, the local time to the second. The client patches' log
//! (vostok::log) then writes `srp_patches.<stamp>.log` in `game\logs`, and
//! the engine's log, which logging_initialize named
//! `<Documents>\Survarium\Survarium_<user>.log` and made anew at every start,
//! is `Survarium_<user>.<stamp>.log` there too (the logs of its kind an
//! earlier exe left in Documents\Survarium deleted first). Before a file is made the oldest
//! of its kind are deleted past 100 files or 500 MiB; older files are never
//! written over.

#![forbid(unsafe_code)]

use crate::vostok::log::{self, Piece};
use crate::vostok::run_log::{self, EngineLogName, Kind};
use crate::vostok::{Context, Flow, NoExit};

/// The process's start: the run's stamp.
pub fn on_process_start(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if run_log::start(ctx.thread()).is_none() {
        log::line(&[Piece::Text("run_logs: no local time; this run's logs keep their old names")]);
    }
    Flow::Continue
}

/// logging_initialize with the engine's log named: the run's name in
/// `game\logs` instead, the logs of its kind an earlier exe left in the
/// engine's folder (Documents\Survarium) and the kind's oldest deleted first. A
/// name that does not fit (a stem near MAX_PATH) keeps the engine's.
pub fn on_engine_log(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(name) = EngineLogName::at(ctx) else { return Flow::Continue };
    let Some(path) = name.path() else { return Flow::Continue };
    let Some(engine) = Kind::of_log(path.bytes(), run_log::lead_byte()) else { return Flow::Continue };
    let Some(stamp) = run_log::start(ctx.thread()) else { return Flow::Continue };
    let kind = engine.in_logs();
    kind.make_folder();
    let cleared = engine.delete_all();
    let deleted = kind.prune(stamp);
    let named = kind.next_name(stamp).is_some_and(|run| name.set(&run));
    let said: &str = if named { "run_logs: the game's log is the run's, " } else { "run_logs: the game's log keeps its name (no room for the run's), " };
    log::line(&[Piece::Text(said), Piece::Dec(cleared), Piece::Text(" in its old folder and "), Piece::Dec(deleted),
                Piece::Text(" old ones deleted")]);
    Flow::Continue
}

/// The arena's C code's line (patch 100's ad_log): to the patches' log, as
/// a line of ours.
pub fn on_c_line(ctx: &mut Context<'_>) -> Flow<NoExit> {
    log::c_line(ctx);
    Flow::Continue
}
