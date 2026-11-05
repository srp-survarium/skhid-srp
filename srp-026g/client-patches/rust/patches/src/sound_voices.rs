//! Patch 96: a new sound takes the voice of a much quieter one when the
//! world's 64 voices are all taken (docs/notes/sound_voices.md).
//!
//! The engine gives voices first come, first served and never takes one
//! back from a sound still above its gain floor. A gun's shot leaves tails
//! of up to ten seconds (the ported guns' `rudnya_outdoor*` layers, 9.2 to
//! 9.9 s, the AKs' forest echo 4.2 s), so a few seconds of automatic fire
//! hold every voice with tails and the next shots, impacts and steps get
//! none: silent until the tails end. At `sound_scene.voice_made`, a sound
//! refused a voice takes the one of the quietest sound playing now, when
//! that one is less than half as loud.

#![forbid(unsafe_code)]

use crate::vostok::voices::{Loudness, VoiceRequest};
use crate::vostok::{Context, Flow, NoExit};

/// A voice changes hands only to a sound more than twice as loud (6 dB):
/// two sounds of about one loudness never take it back and forth.
const MARGIN: f32 = 2.0;

/// Whether a sound of loudness `asking` takes the voice of one of `held`.
pub fn takes(asking: Loudness, held: Loudness) -> bool {
    held.0 * MARGIN < asking.0
}

/// `sound_scene.voice_made`: the world's answer to a propagator asking for
/// a voice.
pub fn on_voice_made(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(mut request) = VoiceRequest::at(ctx) else { return Flow::Continue };
    if request.granted() {
        return Flow::Continue;
    }
    let asking = request.asking().loudness();
    let quietest = request
        .playing()
        .filter(|sound| sound.voiced())
        .min_by(|a, b| a.loudness().0.total_cmp(&b.loudness().0))
        .filter(|victim| takes(asking, victim.loudness()))
        .map(|victim| victim.claim());
    if let Some(victim) = quietest {
        request.take_from(victim);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_voice_goes_only_to_a_sound_more_than_twice_as_loud() {
        assert!(takes(Loudness(0.9), Loudness(0.1)));
        assert!(takes(Loudness(0.5), Loudness(0.2)));
        assert!(!takes(Loudness(0.4), Loudness(0.2)));
        assert!(!takes(Loudness(0.3), Loudness(0.3)));
        assert!(!takes(Loudness(0.0), Loudness(0.0)));
    }
}
