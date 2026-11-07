use super::options::Rgba;
use super::FRAME_MS;

const DURATION: f32 = 1000.0;
const MAX_DT_MS: f64 = 100.0;

pub(crate) type Stops = [[Rgba; 4]; 2];

#[derive(Debug, Default)]
pub(crate) struct Rainbow {
    stop: usize,
    progress: f32,
    last: Option<f64>,
    colors: [Rgba; 2],
}

fn lerp_channel(from: u8, to: u8, t: f32) -> u8 {
    ((1.0 - t) * f32::from(from) + t * f32::from(to)).floor() as u8
}

impl Rainbow {
    pub(crate) fn colors(&self) -> [Rgba; 2] {
        self.colors
    }

    pub(crate) fn advance(&mut self, now: f64, interval: f32, stops: &Stops) {
        let dt = self
            .last
            .map_or(FRAME_MS, |last| (now - last).min(MAX_DT_MS));
        self.last = Some(now);
        let step = (dt / FRAME_MS) as f32 * interval / DURATION;
        let next = (self.stop + 1) % stops[0].len();
        let t = self.progress;
        self.colors = stops.map(|colors| {
            let (from, to) = (colors[self.stop], colors[next]);
            [
                lerp_channel(from[0], to[0], t),
                lerp_channel(from[1], to[1], t),
                lerp_channel(from[2], to[2], t),
                255,
            ]
        });
        if self.progress >= 1.0 {
            self.progress = 0.0;
            self.stop = next;
        }
        self.progress += step;
    }
}
