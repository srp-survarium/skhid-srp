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
        // A285's lines are the log's, made from what the bot already holds: no decision (they leave the hash as it was).
        if note.text.starts_with(crate::v026g::bots::nav_log::PREFIX) {
            return;
        }
        // A292's trap notes are the log's too, made from what the habit already decided.
        if note.text.starts_with(crate::v026g::bots::traps::tally::PREFIX) {
            return;
        }
        // nav-team 2b's dead-link list is the log's too (written with `unstick.dead_links` off as well).
        if note.text.starts_with(crate::v026g::bots::dead_links::PREFIX) {
            return;
        }
        // A302's "; placed by <rule>" in a smoke plan's note is the log's too (the rule that already chose the landing).
        let text = without_placed_by(&note.text);
        self.text(&format!("{} {} {}", note.place, note.time, crate::v026g::bots::stuck::without_evidence(&text)));
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

/// A smoke plan's note without its A302 clause "; placed by <rule>" (the digest of a run before A302 and after it agree).
fn without_placed_by(text: &str) -> std::borrow::Cow<'_, str> {
    const CLAUSE: &str = "; placed by ";
    match text.find(CLAUSE) {
        Some(at) => {
            let rest = &text[at + CLAUSE.len()..];
            let end = rest.find(';').map_or(text.len(), |e| at + CLAUSE.len() + e);
            std::borrow::Cow::Owned(format!("{}{}", &text[..at], &text[end..]))
        }
        None => std::borrow::Cow::Borrowed(text),
    }
}

#[cfg(test)]
mod placed_by_tests {
    #[test]
    fn the_placed_by_clause_leaves_the_digest_text_as_before_a302() {
        let before = "decides a smoke for CrossLane: the lane of player 3; to land at (1.0, 2.0, 3.0), covering 9.0 m of the line";
        let after = "decides a smoke for CrossLane: the lane of player 3; placed by best_cover; to land at (1.0, 2.0, 3.0), covering 9.0 m of the line";
        assert_eq!(super::without_placed_by(after), before);
        assert_eq!(super::without_placed_by(before), before);
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
