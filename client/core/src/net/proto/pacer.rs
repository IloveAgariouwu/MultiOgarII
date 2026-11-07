pub(crate) const FEED_INTERVAL_MS: f64 = 1000.0 / 7.0;
const SPLIT_SPACING_MS: f64 = 70.0;
const STAT_INTERVAL_MS: f64 = 2000.0;
const UNANSWERED_STAT_MS: f64 = 5000.0;

#[derive(Debug, Clone, Default)]
pub(crate) struct Pacer {
    feeding: Option<f64>,
    splits: Vec<f64>,
    stat_sent: Option<f64>,
    stat_waiting: bool,
}

impl Pacer {
    pub(crate) fn feed(&mut self, down: bool, now: f64, interval: f64) -> bool {
        self.feeding = down.then_some(now + interval);
        down
    }

    pub(crate) fn stop_feed(&mut self) {
        self.feeding = None;
    }

    pub(crate) fn feed_due(&mut self, now: f64, interval: f64) -> bool {
        match self.feeding {
            Some(due) if due <= now => {
                self.feeding = Some(now + interval);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn queue_splits(&mut self, now: f64, count: u8) {
        self.splits
            .extend((1..=count).map(|k| now + SPLIT_SPACING_MS * f64::from(k)));
    }

    pub(crate) fn splits_due(&mut self, now: f64) -> usize {
        let queued = self.splits.len();
        self.splits.retain(|&at| at > now);
        queued - self.splits.len()
    }

    pub(crate) fn next_tick_at(&self) -> Option<f64> {
        self.splits
            .iter()
            .copied()
            .chain(self.feeding)
            .reduce(f64::min)
    }

    pub(crate) fn ping(&mut self, now: f64) -> bool {
        if let Some(at) = self.stat_sent {
            let wait = if self.stat_waiting {
                UNANSWERED_STAT_MS
            } else {
                STAT_INTERVAL_MS
            };
            if now - at < wait {
                return false;
            }
        }
        self.stat_sent = Some(now);
        self.stat_waiting = true;
        true
    }

    pub(crate) fn pong(&mut self, now: f64) -> Option<f64> {
        if !std::mem::take(&mut self.stat_waiting) {
            return None;
        }
        self.stat_sent.map(|at| now - at)
    }
}
