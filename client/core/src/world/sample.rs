use serde::Serialize;

use super::Rgb;

const COLORS: [Rgb; 10] = [
    Rgb(0xff, 0x5c, 0x8a),
    Rgb(0x4f, 0xa3, 0xff),
    Rgb(0xff, 0xb3, 0x47),
    Rgb(0x7b, 0xe0, 0xa6),
    Rgb(0xff, 0x7a, 0x1a),
    Rgb(0xc7, 0x92, 0xea),
    Rgb(0xf6, 0xd3, 0x65),
    Rgb(0x6b, 0xd3, 0xff),
    Rgb(0x8f, 0xe3, 0x88),
    Rgb(0xff, 0x9f, 0x5a),
];
const MINSTD_MULTIPLIER: u64 = 16807;
const MINSTD_MODULUS: u64 = 2_147_483_647;

struct SampleRng(u64);

impl SampleRng {
    fn new(seed: u64) -> Self {
        Self(seed % (MINSTD_MODULUS - 1) + 1)
    }

    fn unit(&mut self) -> f32 {
        self.0 = (self.0 * MINSTD_MULTIPLIER) % MINSTD_MODULUS;
        self.0 as f32 / MINSTD_MODULUS as f32
    }

    fn below(&mut self, n: usize) -> usize {
        ((self.unit() * n as f32) as usize).min(n - 1)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Identity {
    pub name: String,
    #[serde(skip)]
    pub rgb: Rgb,
}

pub fn identities(seed: u64, count: usize) -> Vec<Identity> {
    let mut rng = SampleRng::new(seed);
    let mut numbers: Vec<usize> = (0..count).collect();
    (0..count)
        .map(|_| {
            let number = numbers.remove(rng.below(numbers.len()));
            Identity {
                name: format!("Bot | {number}"),
                rgb: COLORS[rng.below(COLORS.len())],
            }
        })
        .collect()
}
