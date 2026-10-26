use super::*;

/// Every input the bots made and every note they wrote, as one number:
/// two runs that decide the same give the same digest. The proof that a
/// change for speed or memory left the bots as they were
/// (`docs/notes/player_bots_mapdata.md` §6).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Decisions {
    pub(crate) digest: mapdata::digest::Digest,
    pub(crate) inputs: u64,
    pub(crate) notes: u64,
}

impl Decisions {
    pub(crate) fn made(&mut self, made: &Made) {
        self.text(&format!("{} {} {:?}", made.place, made.time, made.input));
        self.inputs += 1;
    }

    pub(crate) fn note(&mut self, note: &Note) {
        self.text(&format!("{} {} {}", note.place, note.time, note.text));
        self.notes += 1;
    }

    /// With `SRP_TRACE_DECISIONS=<file>`, each line is also appended
    /// there: two runs' files show where they part.
    pub(crate) fn text(&mut self, text: &str) {
        if let Some(path) = std::env::var_os("SRP_TRACE_DECISIONS") {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap();
            writeln!(file, "{text}").unwrap();
        }
        self.digest.len(text.len());
        for chunk in text.as_bytes().chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.digest.u64(u64::from_le_bytes(word));
        }
    }

    pub(crate) fn line(&self) -> String {
        format!(
            "decisions {:016x} ({} inputs, {} notes)",
            self.digest.finish(),
            self.inputs,
            self.notes
        )
    }
}

/// How a soak run went.
#[derive(Debug, Default)]
pub(crate) struct Outcome {
    pub(crate) decisions: Decisions,
    pub(crate) winner: Option<u8>,
    pub(crate) end_s: f32,
    pub(crate) kills: [u32; 2],
    pub(crate) delivered: [u32; 2],
    pub(crate) stolen: u32,
    pub(crate) taken: u32,
    pub(crate) taken_killed: u32,
    pub(crate) shots: u32,
    pub(crate) hits: u32,
    pub(crate) team_hits: u32,
    pub(crate) team_kills: u32,
    pub(crate) walled: u32,
    pub(crate) hidden: u32,
    /// Sight and aim rays the bots cast (`aimpoint`), per bot and second
    /// of play.
    pub(crate) rays_per_bot_s: f32,
    pub(crate) least_moved_m: f32,
    pub(crate) stuck_notes: usize,
    #[allow(dead_code)]
    pub(crate) notes: Vec<Note>,
    /// Every raid taken (play 24), as it went.
    pub(crate) raids: Vec<RaidRun>,
    pub(crate) carry: CarryTally,
}
