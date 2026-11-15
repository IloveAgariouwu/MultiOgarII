//! Decodes what one player of a MultiOgarII server received over protocol 6 and
//! over the compact protocol, from the same world, and checks that both build
//! the same scene after every tick. The fixture comes from the server's
//! `cargo run --release --example protocol_fixture`.

use std::collections::VecDeque;

use super::Ogar;
use crate::hud::HudUpdate;
use crate::proto::buf::Reader;
use crate::proto::{Cx, Identity};
use crate::world::scene::{Scene, Visibility};
use crate::world::{Circle, Kind, Rgb, Tab, World};

const FIXTURE: &[u8] = include_bytes!("../../../tests/fixtures/ogar6_and_compact.bin.z");
const TICK_MS: f64 = 40.0;

type Ticks = Vec<Vec<Vec<u8>>>;

fn streams() -> [Ticks; 2] {
    let raw = miniz_oxide::inflate::decompress_to_vec_zlib(FIXTURE).expect("fixture inflates");
    let mut r = Reader::new(&raw);
    let mut read = || -> Ticks {
        (0..r.u32().unwrap())
            .map(|_| {
                (0..r.u32().unwrap())
                    .map(|_| {
                        let len = r.u32().unwrap() as usize;
                        r.take(len).unwrap().to_vec()
                    })
                    .collect()
            })
            .collect()
    };
    let streams = [read(), read()];
    assert!(r.is_empty());
    streams
}

#[derive(Debug, PartialEq)]
struct CellState {
    id: u32,
    kind: Kind,
    color: Rgb,
    owner: u32,
    to: Circle,
    /// Player cells only: they are refreshed every tick in both protocols.
    from: Option<Circle>,
    velocity: Option<(f32, f32)>,
    name: String,
    skin: String,
}

#[derive(Debug, PartialEq)]
struct State {
    cells: Vec<CellState>,
    own: Vec<u32>,
    controlled: Option<u32>,
}

struct Client {
    world: World,
    vis: Visibility,
    codec: Ogar,
}

impl Client {
    fn new() -> Self {
        Self { world: World::new(), vis: Visibility::default(), codec: Ogar::default() }
    }

    fn receive(&mut self, frame: &[u8], now: f64) {
        let (mut out, mut hud, identity) = (VecDeque::new(), HudUpdate::default(), Identity::default());
        let mut cx = Cx {
            tab: Tab::Main,
            now,
            out: &mut out,
            hud: &mut hud,
            identity: &identity,
            publishes: false,
            latency: None,
            spectating: false,
        };
        let mut scene = Scene::new(&mut self.world, &mut self.vis, Tab::Main);
        self.codec
            .receive(frame, &mut cx, &mut scene)
            .unwrap_or_else(|e| panic!("frame {:#04x}: {e}", frame[0]));
    }

    fn state(&self) -> State {
        let w = &self.world;
        let mut cells: Vec<CellState> = w
            .cells()
            .iter()
            .map(|c| {
                let player = (c.kind == Kind::Player).then(|| w.player(c.owner)).flatten();
                CellState {
                    id: c.id,
                    kind: c.kind,
                    color: c.color,
                    owner: c.owner,
                    to: c.to,
                    from: (c.kind == Kind::Player).then_some(c.from),
                    velocity: (c.kind == Kind::Player).then_some(c.velocity),
                    name: player.map_or(String::new(), |p| w.string(p.name).to_owned()),
                    skin: player.map_or(String::new(), |p| w.string(p.skins[0]).to_owned()),
                }
            })
            .collect();
        cells.sort_by_key(|c| c.id);
        let mut own = w.own(Tab::Main).to_vec();
        own.sort_unstable();
        State { cells, own, controlled: self.vis.units[0] }
    }
}

#[test]
fn compact_protocol_builds_the_same_scene_as_protocol_6() {
    let [ogar6, compact] = streams();
    assert_eq!(ogar6.len(), compact.len());
    let (mut a, mut b) = (Client::new(), Client::new());
    let (mut bytes_a, mut bytes_b, mut players_seen, mut own_units) = (0, 0, 0, [false; 2]);
    for (tick, (frames_a, frames_b)) in ogar6.iter().zip(&compact).enumerate() {
        let now = tick as f64 * TICK_MS;
        for f in frames_a {
            bytes_a += f.len();
            a.receive(f, now);
        }
        for f in frames_b {
            bytes_b += f.len();
            b.receive(f, now);
        }
        let (sa, sb) = (a.state(), b.state());
        if sa != sb {
            let first = sa.cells.iter().zip(&sb.cells).find(|(x, y)| x != y);
            panic!(
                "tick {tick}: scenes differ ({} and {} cells, own {:?} and {:?}, controlled \
                 {:?} and {:?}); first differing cell: {first:#?}",
                sa.cells.len(),
                sb.cells.len(),
                sa.own,
                sb.own,
                sa.controlled,
                sb.controlled
            );
        }
        match (a.world.server_view(), b.world.server_view()) {
            (None, None) => {}
            (Some(va), Some(vb)) => {
                assert!((va.x - vb.x).abs() <= 1.0 && (va.y - vb.y).abs() <= 1.0, "tick {tick}");
            }
            other => panic!("tick {tick}: server views {other:?}"),
        }
        players_seen = players_seen.max(sa.cells.iter().filter(|c| c.kind == Kind::Player).count());
        for c in &sa.cells {
            if c.owner == 2 || c.owner == 3 {
                own_units[(c.owner - 2) as usize] = true;
            }
        }
    }
    assert!(!a.codec.compact && b.codec.compact);
    assert!(players_seen > 10, "only {players_seen} player cells in view");
    assert_eq!(own_units, [true, true], "both units played");
    assert!(bytes_b * 3 < bytes_a, "compact {bytes_b} bytes, protocol 6 {bytes_a}");
}
