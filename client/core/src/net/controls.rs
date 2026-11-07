use super::hotkey::Hotkey;
use super::proto::{Act, Cx, Ogar};

const DOUBLE_SPLIT_FREEZE_MS: f64 = 600.0;
const OVERLAP_FEED_MS: f64 = 240.0;

type Point = (f32, f32);

#[derive(Debug, Default, Clone)]
pub(crate) struct Controls {
    stop_mouse: bool,
    stop_until: Option<f64>,
    holding_split: bool,
    pending: Vec<(f64, Act)>,
}

impl Controls {
    pub(crate) fn frozen(&self) -> bool {
        self.stop_mouse
    }

    pub(crate) fn next_tick_at(&self) -> Option<f64> {
        self.pending
            .iter()
            .map(|p| p.0)
            .chain(self.stop_until)
            .reduce(f64::min)
    }

    pub(crate) fn tick(&mut self, p: &mut Ogar, cx: &mut Cx) {
        if self.stop_until.is_some_and(|until| until <= cx.now) {
            self.stop_until = None;
            self.stop_mouse = false;
        }
        let now = cx.now;
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].0 <= now {
                let (_, act) = self.pending.remove(i);
                p.send_action(act, cx);
            } else {
                i += 1;
            }
        }
    }

    fn later(&mut self, cx: &Cx, delay_ms: f64, act: Act) {
        self.pending.push((cx.now + delay_ms, act));
    }

    fn hold_split(&mut self, n: u8, down: bool, aim: Point, p: &mut Ogar, cx: &mut Cx) {
        let was_holding = self.holding_split;
        p.aim(aim.0, aim.1, cx);
        self.stop_mouse = down;
        self.holding_split = down;
        if down && !was_holding {
            p.send_action(Act::Split(n), cx);
        }
    }

    pub(crate) fn overlap_feed(&mut self, p: &mut Ogar, cx: &mut Cx) {
        p.send_action(Act::Feed(true), cx);
        self.later(cx, OVERLAP_FEED_MS, Act::Feed(false));
    }

    fn split(n: u8, down: bool, aim: Point, p: &mut Ogar, cx: &mut Cx) {
        if down {
            if n == 1 {
                p.aim(aim.0, aim.1, cx);
            }
            p.send_action(Act::Split(n), cx);
        }
    }

    fn on_down(down: bool, act: Act, p: &mut Ogar, cx: &mut Cx) -> bool {
        if down {
            p.send_action(act, cx);
        }
        down
    }

    pub(crate) fn press(
        &mut self,
        hotkey: Hotkey,
        down: bool,
        aim: Point,
        p: &mut Ogar,
        cx: &mut Cx,
    ) -> bool {
        if hotkey.releases_mouse() {
            self.stop_mouse = false;
        }
        match hotkey {
            Hotkey::Tab
            | Hotkey::StartNewGame
            | Hotkey::ToggleSpectateTarget
            | Hotkey::Parallel => false,
            Hotkey::DoubleSplit => {
                p.aim(aim.0, aim.1, cx);
                self.stop_mouse = down;
                if down {
                    self.stop_until = Some(cx.now + DOUBLE_SPLIT_FREEZE_MS);
                    p.send_action(Act::Split(2), cx);
                }
                true
            }
            Hotkey::Split => {
                Self::split(1, down, aim, p, cx);
                true
            }
            Hotkey::QuadSplit => {
                Self::split(4, down, aim, p, cx);
                true
            }
            Hotkey::QuintupleSplit => {
                Self::split(5, down, aim, p, cx);
                true
            }
            Hotkey::SextupleSplit => {
                Self::split(6, down, aim, p, cx);
                true
            }
            Hotkey::SeptupleSplit => {
                Self::split(7, down, aim, p, cx);
                true
            }
            Hotkey::OctupleSplit => {
                Self::split(8, down, aim, p, cx);
                true
            }
            Hotkey::TripleSplit => {
                self.hold_split(3, down, aim, p, cx);
                true
            }
            Hotkey::SuperQuadSplit => {
                self.hold_split(4, down, aim, p, cx);
                true
            }
            Hotkey::Feed => {
                p.send_action(Act::Feed(down), cx);
                true
            }
            Hotkey::FeedOne => Self::on_down(down, Act::FeedOne, p, cx),
            Hotkey::ChangeUnit => Self::on_down(down, Act::ChangeUnit, p, cx),
            Hotkey::InfernoSplit => {
                if down {
                    p.send_action(Act::Split(2), cx);
                    p.send_action(Act::ChangeUnit, cx);
                    p.aim(aim.0, aim.1, cx);
                    p.send_action(Act::Split(4), cx);
                }
                down
            }
            Hotkey::TestSplit => {
                if down {
                    p.send_action(Act::Split(4), cx);
                    p.send_action(Act::ChangeUnit, cx);
                    p.aim(aim.0, aim.1, cx);
                    p.send_action(Act::Split(4), cx);
                }
                down
            }
            Hotkey::OverlapSplit => {
                if down {
                    p.send_action(Act::Split(4), cx);
                    p.send_action(Act::ChangeUnit, cx);
                    p.send_action(Act::Split(4), cx);
                    self.overlap_feed(p, cx);
                }
                down
            }
            Hotkey::OverlapSplitOnce => {
                if down {
                    p.send_action(Act::Split(1), cx);
                    p.send_action(Act::ChangeUnit, cx);
                    p.send_action(Act::Split(1), cx);
                    p.send_action(Act::ChangeUnit, cx);
                }
                down
            }
        }
    }
}
