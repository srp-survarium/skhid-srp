//! Python-compatible Mersenne Twister (`random.Random(seed)` for an int seed, CPython 3.12): `random()`,
//! `uniform`, `randint`, `choices` with weights. Used so the synthetic result statistics are the same byte for byte
//! for the same seed as those of the earlier reference implementation.

pub struct PyRandom {
    mt: [u32; 624],
    idx: usize,
}

impl PyRandom {
    /// `random.Random(seed)` for a non-negative int seed below 2^32 (CPython `init_by_array([seed])`).
    pub fn new(seed: u32) -> Self {
        let mut r = PyRandom { mt: [0; 624], idx: 624 };
        r.init_genrand(19_650_218);
        let key = [seed];
        let (mut i, mut j) = (1usize, 0usize);
        for _ in 0..624.max(key.len()) {
            let prev = r.mt[i - 1] ^ (r.mt[i - 1] >> 30);
            r.mt[i] = (r.mt[i] ^ prev.wrapping_mul(1_664_525)).wrapping_add(key[j]).wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= 624 {
                r.mt[0] = r.mt[623];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
        }
        for _ in 0..623 {
            let prev = r.mt[i - 1] ^ (r.mt[i - 1] >> 30);
            r.mt[i] = (r.mt[i] ^ prev.wrapping_mul(1_566_083_941)).wrapping_sub(i as u32);
            i += 1;
            if i >= 624 {
                r.mt[0] = r.mt[623];
                i = 1;
            }
        }
        r.mt[0] = 0x8000_0000;
        r.idx = 624;
        r
    }

    fn init_genrand(&mut self, s: u32) {
        self.mt[0] = s;
        for i in 1..624 {
            let p = self.mt[i - 1] ^ (self.mt[i - 1] >> 30);
            self.mt[i] = p.wrapping_mul(1_812_433_253).wrapping_add(i as u32);
        }
    }

    fn genrand(&mut self) -> u32 {
        if self.idx >= 624 {
            for k in 0..624 {
                let y = (self.mt[k] & 0x8000_0000) | (self.mt[(k + 1) % 624] & 0x7FFF_FFFF);
                let mut v = self.mt[(k + 397) % 624] ^ (y >> 1);
                if y & 1 != 0 {
                    v ^= 0x9908_B0DF;
                }
                self.mt[k] = v;
            }
            self.idx = 0;
        }
        let mut y = self.mt[self.idx];
        self.idx += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9D2C_5680;
        y ^= (y << 15) & 0xEFC6_0000;
        y ^= y >> 18;
        y
    }

    /// `random()`: 53-bit float in [0, 1).
    pub fn random(&mut self) -> f64 {
        let a = (self.genrand() >> 5) as f64;
        let b = (self.genrand() >> 6) as f64;
        (a * 67_108_864.0 + b) * (1.0 / 9_007_199_254_740_992.0)
    }

    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }

    fn randbelow(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let k = 32 - n.leading_zeros();
        loop {
            let r = self.genrand() >> (32 - k);
            if r < n {
                return r;
            }
        }
    }

    /// `randint(a, b)` (inclusive).
    pub fn randint(&mut self, a: u32, b: u32) -> u32 {
        a + self.randbelow(b - a + 1)
    }

    /// `choices(range(len(weights)), weights, k=k)`: indices.
    pub fn choices(&mut self, weights: &[f64], k: usize) -> Vec<usize> {
        let mut cum = Vec::with_capacity(weights.len());
        let mut s = 0.0;
        for w in weights {
            s += w;
            cum.push(s);
        }
        let total = cum[cum.len() - 1] + 0.0;
        let hi = weights.len() - 1;
        (0..k)
            .map(|_| {
                let x = self.random() * total;
                // bisect_right(cum, x, 0, hi)
                let (mut lo, mut h) = (0usize, hi);
                while lo < h {
                    let mid = (lo + h) / 2;
                    if x < cum[mid] {
                        h = mid;
                    } else {
                        lo = mid + 1;
                    }
                }
                lo
            })
            .collect()
    }
}
