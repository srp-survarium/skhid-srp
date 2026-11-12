//! Patch 83's render scale: the 3D frame rendered at a share of the window's
//! size, the window, the back buffer and Scaleform at the window's
//! (docs/notes/dlss-option.md, "The render size"). DLSS's link asks for it
//! with [`ask`]; until that exists, the diagnostics switch
//! `SKHID_DLSS_RENDER_SCALE=0.667` does, at the process's start.
//!
//! One owner ([`OWNER`]) holds the scale wanted and the frame as it was
//! made: native, or scaled with its depth-stencil. The frame's targets are
//! made at the wanted scale wherever the engine makes them (set_size); a
//! scale asked later is made by a forced set_size at the next frame's
//! start. Without DLSS the smaller frame reaches the window through the
//! stock screen image quad, stretched.

#![forbid(unsafe_code)]

use crate::vostok::log::{Piece::*, line};
use crate::vostok::render_size::{self, DepthTarget, Size, TargetsSize};
use crate::vostok::renderer::get_environment;
use crate::vostok::{Context, Flow, Lock, NoExit, Thread, ThreadBound};

/// A render size's share of the output's, each side: within
/// [`Ratio::LEAST`]..=[`Ratio::MOST`] (DLSS's modes are 0.33..0.77).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ratio(f32);

impl Ratio {
    const LEAST: f32 = 0.25;
    const MOST: f32 = 0.95;
    /// The engine makes no output under 16 pixels a side (resize 0x3CFBAC).
    const LEAST_SIDE: u32 = 16;

    pub fn new(share: f32) -> Option<Ratio> {
        (Ratio::LEAST..=Ratio::MOST).contains(&share).then_some(Ratio(share))
    }

    /// A decimal fraction's text, `0.667` or `.5`: at most six digits after
    /// the point, nothing else.
    pub fn of_text(text: &[u8]) -> Option<Ratio> {
        let digits = text.strip_prefix(b"0.").or_else(|| text.strip_prefix(b"."))?;
        if digits.is_empty() || digits.len() > 6 {
            return None;
        }
        let (mut value, mut scale) = (0u32, 1u32);
        for &digit in digits {
            if !digit.is_ascii_digit() {
                return None;
            }
            value = value * 10 + u32::from(digit - b'0');
            scale *= 10;
        }
        Ratio::new(value as f32 / scale as f32)
    }

    /// The render size of an output: each side's share, rounded, never
    /// under the engine's least.
    pub fn of(self, output: Size) -> Size {
        let side = |pixels: u32| ((pixels as f32 * self.0 + 0.5) as u32).max(Ratio::LEAST_SIDE).min(pixels);
        Size { width: side(output.width), height: side(output.height) }
    }

    /// Thousandths, for a log line.
    fn thousandths(self) -> u32 {
        (self.0 * 1000.0 + 0.5) as u32
    }
}

/// What the 3D frame is rendered at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RenderScale {
    /// The window's size: the engine as it is.
    Native,
    /// A share of the window's size.
    Ratio(Ratio),
}

/// The frame as its targets were last made.
enum Frame {
    Native,
    /// At `ratio` of the window, `depth` its depth-stencil (and its size).
    Scaled { ratio: Ratio, depth: ThreadBound<DepthTarget> },
}

impl Frame {
    fn scale(&self) -> RenderScale {
        match self {
            Frame::Native => RenderScale::Native,
            Frame::Scaled { ratio, .. } => RenderScale::Ratio(*ratio),
        }
    }
}

struct Owner {
    wanted: RenderScale,
    frame: Frame,
}

static OWNER: Lock<Owner> = Lock::new(Owner { wanted: RenderScale::Native, frame: Frame::Native });

/// What happens to the render size.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Event {
    /// A scale is asked for ([`ask`]).
    Asked(RenderScale),
    /// A frame starts (`render.output_setup`).
    FrameStarts,
    /// The engine is about to make the frame's targets for an output of
    /// this size (`render_window.targets_size`).
    TargetsAsked(Size),
}

/// What an event asks of the engine.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Nothing,
    /// The frame is another scale's: its targets are made again.
    Remake,
    /// The targets are made at the output's size.
    MakeNative,
    /// The targets and a depth-stencil are made at this size.
    MakeScaled(Ratio, Size),
}

/// The scale wanted after `event`, and what it asks; `made` is the frame's
/// scale now.
fn step(wanted: RenderScale, made: RenderScale, event: Event) -> (RenderScale, Action) {
    match event {
        Event::Asked(scale) => (scale, Action::Nothing),
        Event::FrameStarts if wanted == made => (wanted, Action::Nothing),
        Event::FrameStarts => (wanted, Action::Remake),
        Event::TargetsAsked(output) => match wanted {
            RenderScale::Native => (wanted, Action::MakeNative),
            RenderScale::Ratio(ratio) => (wanted, Action::MakeScaled(ratio, ratio.of(output))),
        },
    }
}

impl Owner {
    fn step(&mut self, event: Event) -> Action {
        let (wanted, action) = step(self.wanted, self.frame.scale(), event);
        self.wanted = wanted;
        action
    }

    /// The frame is `frame` from now on; the one before lets its
    /// depth-stencil go, on the thread that made it.
    fn made(&mut self, thread: &Thread, frame: Frame) {
        if let Frame::Scaled { depth, .. } = core::mem::replace(&mut self.frame, frame) {
            drop(depth.into_inner(thread));
        }
    }

    /// The scaled frame's depth-stencil, on its thread.
    fn depth(&self, thread: &Thread) -> Option<&DepthTarget> {
        match &self.frame {
            Frame::Native => None,
            Frame::Scaled { depth, .. } => depth.get(thread),
        }
    }
}

/// The link's one call: the 3D frame is rendered at `scale` from the next
/// frame on. A ratio is taken only at the process's start (before the
/// render device: the frame's sites are written then, and only then);
/// native is taken any time (the fallback). Answers whether it was taken.
pub fn ask(thread: &Thread, scale: RenderScale) -> bool {
    if let RenderScale::Ratio(ratio) = scale
        && let Err(refused) = render_size::install(thread)
    {
        line(&[Text("render size: "), Dec(ratio.thousandths()), Text("/1000 not taken: "), Text(match refused {
            render_size::InstallRefused::NotTheStart => "asked after the render device was made",
            render_size::InstallRefused::Site(_) => "a site of the frame is not the game's code",
        })]);
        return false;
    }
    OWNER.with(thread, |owner| owner.step(Event::Asked(scale)));
    true
}

/// The process's start: the diagnostics switch `SKHID_DLSS_RENDER_SCALE`,
/// a ratio as `0.667`. Unset, nothing is asked and nothing written.
pub fn start(thread: &Thread) {
    let mut room = [0u8; 16];
    let len = get_environment(thread, c"SKHID_DLSS_RENDER_SCALE", &mut room);
    if len == 0 {
        return;
    }
    match Ratio::of_text(&room[..len]) {
        Some(ratio) => {
            if ask(thread, RenderScale::Ratio(ratio)) {
                line(&[Text("render size: SKHID_DLSS_RENDER_SCALE, the 3D frame at "), Dec(ratio.thousandths()),
                       Text("/1000 of the window")]);
            }
        }
        None => line(&[Text("render size: SKHID_DLSS_RENDER_SCALE is not a ratio as 0.667 (0.25 to 0.95): native")]),
    }
}

/// `render_window.targets_size`: the engine makes the frame's targets; at
/// the wanted scale they and a depth-stencil of ours are the render size.
/// No depth-stencil: native, and nothing is asked again.
pub fn on_targets_size(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some(targets) = TargetsSize::at(ctx) else { return Flow::Continue };
    OWNER.with(thread, |owner| match owner.step(Event::TargetsAsked(targets.asked())) {
        Action::MakeScaled(ratio, size) => match DepthTarget::make(thread, size) {
            Some(depth) => {
                line(&[Text("render size: the 3D frame "), Dec(size.width), Text("x"), Dec(size.height), Text(" of "),
                       Dec(targets.asked().width), Text("x"), Dec(targets.asked().height)]);
                owner.made(thread, Frame::Scaled { ratio, depth: ThreadBound::new(thread, depth) });
                targets.make_at(size);
            }
            None => {
                line(&[Text("render size: no depth-stencil of "), Dec(size.width), Text("x"), Dec(size.height),
                       Text(": native")]);
                owner.step(Event::Asked(RenderScale::Native));
                owner.made(thread, Frame::Native);
            }
        },
        Action::MakeNative => owner.made(thread, Frame::Native),
        Action::Nothing | Action::Remake => {}
    });
    Flow::Continue
}

/// `render.output_setup`: a frame starts; a frame of another scale than the
/// wanted one has its targets made again (the engine's forced set_size,
/// [`on_targets_size`] inside it).
pub fn on_output_setup(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    // the lock is let go before set_size: on_targets_size takes it
    if OWNER.with(thread, |owner| owner.step(Event::FrameStarts)) == Action::Remake {
        render_size::remake_targets(ctx);
    }
    Flow::Continue
}

/// `render.output_set`: the stages of a scaled frame draw at its size, into
/// its depth-stencil.
pub fn on_output_set(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    OWNER.with(thread, |owner| {
        if let Some(depth) = owner.depth(thread) {
            render_size::enter(thread, depth);
        }
    });
    Flow::Continue
}

/// `gbuffer.depth_texture`: a scaled frame's linear depth is copied from
/// its own depth-stencil.
pub fn on_depth_texture(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let texture = OWNER.with(thread, |owner| owner.depth(thread).and_then(DepthTarget::texture));
    if let Some(texture) = texture {
        render_size::copy_depth_from(ctx, texture);
    }
    Flow::Continue
}

/// `render.present_stage`: a scaled frame is done; the screen image and
/// Scaleform draw on the back buffer at the window's size.
pub fn on_present_stage(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    OWNER.with(thread, |owner| {
        if owner.depth(thread).is_some() {
            render_size::leave(thread);
        }
    });
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ratio(share: f32) -> Ratio {
        Ratio::new(share).unwrap()
    }

    #[test]
    fn a_ratio_is_a_fraction_within_its_range() {
        assert_eq!(Ratio::of_text(b"0.667"), Some(ratio(0.667)));
        assert_eq!(Ratio::of_text(b".5"), Some(ratio(0.5)));
        for refused in [&b""[..], b"0.", b"1", b"1.0", b"0.2", b"0.96", b"0.6x", b"0,667", b"-0.5", b"0.6670000"] {
            assert_eq!(Ratio::of_text(refused), None, "{:?}", core::str::from_utf8(refused));
        }
        assert_eq!(Ratio::new(f32::NAN), None);
    }

    #[test]
    fn a_render_size_is_each_side_rounded() {
        let quality = ratio(0.667);
        assert_eq!(quality.of(Size { width: 1920, height: 1080 }), Size { width: 1281, height: 720 });
        assert_eq!(quality.of(Size { width: 2560, height: 1440 }), Size { width: 1708, height: 960 });
        // never under the engine's least side, never over the output
        assert_eq!(ratio(0.25).of(Size { width: 40, height: 20 }), Size { width: 16, height: 16 });
        assert_eq!(ratio(0.25).of(Size { width: 8, height: 8 }), Size { width: 8, height: 8 });
    }

    #[test]
    fn the_frame_follows_the_scale_asked() {
        let scaled = RenderScale::Ratio(ratio(0.5));
        let output = Size { width: 1920, height: 1080 };
        // asked: wanted, nothing done until a frame starts
        assert_eq!(step(RenderScale::Native, RenderScale::Native, Event::Asked(scaled)), (scaled, Action::Nothing));
        assert_eq!(step(scaled, RenderScale::Native, Event::FrameStarts), (scaled, Action::Remake));
        // the engine's set_size makes the targets at the wanted scale, whoever called it
        assert_eq!(step(scaled, RenderScale::Native, Event::TargetsAsked(output)),
                   (scaled, Action::MakeScaled(ratio(0.5), Size { width: 960, height: 540 })));
        assert_eq!(step(scaled, scaled, Event::FrameStarts), (scaled, Action::Nothing));
        // the fallback: native asked, the next frame remakes, set_size makes native
        assert_eq!(step(scaled, scaled, Event::Asked(RenderScale::Native)), (RenderScale::Native, Action::Nothing));
        assert_eq!(step(RenderScale::Native, scaled, Event::FrameStarts), (RenderScale::Native, Action::Remake));
        assert_eq!(step(RenderScale::Native, scaled, Event::TargetsAsked(output)), (RenderScale::Native, Action::MakeNative));
        assert_eq!(step(RenderScale::Native, RenderScale::Native, Event::FrameStarts), (RenderScale::Native, Action::Nothing));
    }
}
