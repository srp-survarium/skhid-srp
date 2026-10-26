use crate::{RATE, sounds::STATIC_BED};
#[derive(Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}
#[derive(Clone, Copy)]
enum Kind {
    Low,
    High,
    Peak,
}
impl Biquad {
    fn new(kind: Kind, f: f32, q: f32, gain: f32) -> Self {
        let w = 2. * std::f32::consts::PI * f / RATE as f32;
        let c = w.cos();
        let alpha = w.sin() / (2. * q);
        let (b0, b1, b2, a0, a1, a2) = match kind {
            Kind::Peak => {
                let a = 10f32.powf(gain / 40.);
                (
                    1. + alpha * a,
                    -2. * c,
                    1. - alpha * a,
                    1. + alpha / a,
                    -2. * c,
                    1. - alpha / a,
                )
            }
            Kind::High => (
                (1. + c) / 2.,
                -(1. + c),
                (1. + c) / 2.,
                1. + alpha,
                -2. * c,
                1. - alpha,
            ),
            Kind::Low => (
                (1. - c) / 2.,
                1. - c,
                (1. - c) / 2.,
                1. + alpha,
                -2. * c,
                1. - alpha,
            ),
        };
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.,
            z2: 0.,
        }
    }
    fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}
/// Fitted to retail's plain/_radio quick-command pairs. The clipping's
/// high-pass removes difference tones; the makeup preserves voice loudness.
pub struct Radio {
    stages: [Biquad; 10],
    post: Biquad,
    bed_at: usize,
}
impl Radio {
    pub fn new(seed: u32) -> Self {
        use Kind::*;
        let specs = [
            (High, 405., 0.5098, 0.),
            (High, 405., 0.6013, 0.),
            (High, 405., 0.9, 0.),
            (High, 405., 2.5629, 0.),
            (Peak, 575., 3., 12.),
            (Peak, 790., 0.5, -12.),
            (Peak, 1990., 3., 6.6),
            (Peak, 3120., 1., -12.),
            (Peak, 4490., 3., 12.),
            (Low, 6300., std::f32::consts::FRAC_1_SQRT_2, 0.),
        ];
        Self {
            stages: specs.map(|(k, f, q, g)| Biquad::new(k, f, q, g)),
            post: Biquad::new(High, 400., std::f32::consts::FRAC_1_SQRT_2, 0.),
            bed_at: seed as usize % STATIC_BED.len(),
        }
    }
    pub fn process(&mut self, pcm: &mut [f32], bed: f32) {
        for sample in pcm {
            let mut v = *sample;
            for stage in &mut self.stages {
                v = stage.run(v);
            }
            let drive = v * 5.;
            let sat = if drive >= 3. {
                1.
            } else if drive <= -3. {
                -1.
            } else {
                let v2 = drive * drive;
                drive * (27. + v2) / (27. + 9. * v2)
            };
            v = self.post.run(sat / 5. * 2.5);
            if bed > 0. {
                v += bed * f32::from(STATIC_BED[self.bed_at]) / 32768.;
                self.bed_at = (self.bed_at + 1) % STATIC_BED.len();
            }
            *sample = v;
        }
    }
}
pub fn unit(value: f32) -> f32 {
    if value.is_nan() {
        0.
    } else {
        value.clamp(0., 1.)
    }
}
pub fn mic_gain(sensitivity: f32) -> f32 {
    2f32.powf((unit(sensitivity) - 0.5) * 4.)
}
