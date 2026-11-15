//! Plays a human connection against a world of bots and decodes every frame it
//! would receive, keeping a model of the cells the client knows. The model must
//! match what the server believes the client has, and the positions in it must
//! match the world, whether the client reads every tick or falls behind. Both
//! protocol 6 and the compact protocol are played.

use multiogarii::core::config::Config;
use multiogarii::net::compact;
use multiogarii::net::decode::ClientMsg;
use multiogarii::net::outbox::Outbox;
use multiogarii::world::cell::Kind;
use multiogarii::world::data::Data;
use multiogarii::world::{ConnInfo, World};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

const TICK_MS: f64 = 40.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Codec {
    Ogar6,
    Compact,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Seen {
    x: i32,
    y: i32,
    radius: u32,
    /// Compact only: the kind code and the owner's pid.
    kind: u8,
    owner: Option<u32>,
}

struct Client {
    codec: Codec,
    hello: bool,
    cells: HashMap<u32, Seen>,
    players: HashMap<u32, String>,
    camera: (i32, i32),
    update_frames: usize,
    update_bytes: usize,
    other_bytes: usize,
}

struct Reader<'a> {
    buf: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> &[u8] {
        let out = &self.buf[self.at..self.at + n];
        self.at += n;
        out
    }
    fn u8(&mut self) -> u8 {
        self.take(1)[0]
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.take(2).try_into().unwrap())
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    fn utf8z(&mut self) -> String {
        let start = self.at;
        while self.u8() != 0 {}
        String::from_utf8(self.buf[start..self.at - 1].to_vec()).unwrap()
    }
    fn varint(&mut self) -> u64 {
        let (mut v, mut shift) = (0u64, 0);
        loop {
            let b = self.u8();
            v |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return v;
            }
            shift += 7;
        }
    }
    fn signed(&mut self) -> i64 {
        let v = self.varint();
        (v >> 1) as i64 ^ -((v & 1) as i64)
    }
    fn done(&self) -> bool {
        self.at == self.buf.len()
    }
}

impl Client {
    fn new(codec: Codec) -> Self {
        Self {
            codec,
            hello: false,
            cells: HashMap::new(),
            players: HashMap::new(),
            camera: (0, 0),
            update_frames: 0,
            update_bytes: 0,
            other_bytes: 0,
        }
    }

    fn apply(&mut self, frame: &[u8]) {
        if !self.hello {
            assert_eq!(frame[0] == compact::OP_HELLO, self.codec == Codec::Compact, "first frame");
            self.hello = true;
        }
        match (frame[0], self.codec) {
            (0x10, Codec::Ogar6) => self.apply_update_nodes(frame),
            (compact::OP_WORLD, Codec::Compact) => self.apply_world(frame),
            (compact::OP_PLAYER, Codec::Compact) => {
                self.other_bytes += frame.len();
                let mut r = Reader { buf: frame, at: 1 };
                let pid = r.varint() as u32;
                let name = r.utf8z();
                let _skin = r.utf8z();
                assert!(r.done());
                self.players.insert(pid, name);
            }
            (0x10 | 0x11 | 0x20 | 0x21, Codec::Compact) => panic!("legacy frame {:#x}", frame[0]),
            (op, _) => {
                self.other_bytes += frame.len();
                if op == 0x12 {
                    self.cells.clear();
                }
            }
        }
    }

    /// Protocol 6 UpdateNodes: eats, nodes up to id 0, then removals.
    fn apply_update_nodes(&mut self, frame: &[u8]) {
        self.update_frames += 1;
        self.update_bytes += frame.len();
        let mut r = Reader { buf: frame, at: 1 };
        let mut eaten = Vec::new();
        for _ in 0..r.u16() {
            let _hunter = r.u32();
            let prey = r.u32();
            assert!(self.cells.remove(&prey).is_some(), "eat of unknown cell {prey}");
            eaten.push(prey);
        }
        loop {
            let id = r.u32();
            if id == 0 {
                break;
            }
            let (x, y, radius) = (r.u32() as i32, r.u32() as i32, r.u16());
            let flags = r.u8();
            if flags & 0x02 != 0 {
                r.take(3);
            }
            if flags & 0x04 != 0 {
                r.utf8z();
            }
            if flags & 0x08 != 0 {
                r.utf8z();
            }
            let seen = Seen { x, y, radius: radius.into(), kind: 0, owner: None };
            self.cells.insert(id, seen);
        }
        for _ in 0..r.u16() {
            // Eaten cells are listed again among the removals.
            let id = r.u32();
            assert!(
                self.cells.remove(&id).is_some() || eaten.contains(&id),
                "removal of unknown cell {id}"
            );
        }
        assert!(r.done(), "trailing bytes in an update");
    }

    fn apply_world(&mut self, frame: &[u8]) {
        self.update_frames += 1;
        self.update_bytes += frame.len();
        let mut r = Reader { buf: frame, at: 1 };
        self.camera = (r.signed() as i32, r.signed() as i32);
        let next = |prev: &mut u32, delta: u64| {
            *prev = prev.wrapping_add(delta as u32);
            *prev
        };
        let mut prev = 0;
        for _ in 0..r.varint() {
            let prey = next(&mut prev, r.varint());
            let _hunter = r.varint();
            assert!(self.cells.remove(&prey).is_some(), "eat of unknown cell {prey}");
        }
        let mut prev = 0;
        for _ in 0..r.varint() {
            let head = r.varint();
            let id = next(&mut prev, head >> 4);
            let changed = head as u8 & 0x0f;
            assert_ne!(changed, 0, "update of {id} that changes nothing");
            let cell = self.cells.get_mut(&id).unwrap_or_else(|| panic!("update of unknown {id}"));
            if changed & compact::CHANGED_X != 0 {
                cell.x += r.signed() as i32;
            }
            if changed & compact::CHANGED_Y != 0 {
                cell.y += r.signed() as i32;
            }
            if changed & compact::CHANGED_RADIUS != 0 {
                cell.radius = (i64::from(cell.radius) + r.signed()) as u32;
            }
            if changed & compact::CHANGED_COLOR != 0 {
                r.take(3);
            }
        }
        let mut prev = 0;
        for _ in 0..r.varint() {
            let head = r.varint();
            let id = next(&mut prev, head >> 3);
            let kind = head as u8 & 0x07;
            let x = self.camera.0 + r.signed() as i32;
            let y = self.camera.1 + r.signed() as i32;
            let radius = r.varint() as u32;
            r.take(3);
            let owner = (kind >= compact::KIND_PLAYER).then(|| r.varint() as u32);
            if let Some(pid) = owner {
                assert!(self.players.contains_key(&pid), "cell {id} of untold player {pid}");
            }
            let seen = Seen { x, y, radius, kind, owner };
            assert!(self.cells.insert(id, seen).is_none(), "add of known cell {id}");
        }
        let mut prev = 0;
        for _ in 0..r.varint() {
            let id = next(&mut prev, r.varint());
            assert!(self.cells.remove(&id).is_some(), "removal of unknown cell {id}");
        }
        assert!(r.done(), "trailing bytes in a world frame");
    }
}

struct Game {
    world: World,
    outbox: Arc<Outbox>,
    slot: u32,
    client: Client,
    rng: u64,
    ejected_seen: usize,
    own_seen: [usize; 2],
}

impl Game {
    fn new(seed: u64, codec: Codec, scramble: i32) -> Self {
        let cfg = Config {
            server_bots: 60,
            border_width: 5000.0,
            border_height: 5000.0,
            server_minion_threshold: 1000,
            server_scramble_level: scramble,
            ..Config::default()
        };
        let data = Data::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("data"));
        let mut world = World::new(Arc::new(cfg), data, seed, 0.0);
        let outbox = Arc::new(Outbox::new());
        let slot = world
            .connect(ConnInfo {
                ip: "10.0.0.1".into(),
                origin: None,
                user_agent: Some("x".repeat(60)),
                cache_control: Some("no-cache".into()),
                outbox: outbox.clone(),
            })
            .expect("connection accepted");
        let compact = codec == Codec::Compact;
        world.client_message(slot, ClientMsg::Ready { protocol: 6, compact });
        // Big enough to eject for a long time and to outgrow the bots.
        world.player_mut(slot).spawn_size = 600.0;
        Self {
            world,
            outbox,
            slot,
            client: Client::new(codec),
            rng: seed | 1,
            ejected_seen: 0,
            own_seen: [0; 2],
        }
    }

    fn random(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Plays one tick: steer and eject at random, rejoin after dying, and now
    /// and then switch to the other unit.
    fn tick(&mut self, t: u64) {
        let player = self.world.player(self.slot);
        if !player.is_alive() && t.is_multiple_of(30) {
            self.world.client_message(self.slot, ClientMsg::Join("tester".into()));
        }
        if t % 150 == 75 {
            self.world.client_message(self.slot, ClientMsg::SwitchUnit { tagged: true });
        }
        // Mostly wanders, but every 100 ticks ejects at one spot for 20 ticks,
        // so ejected cells pile up and push each other apart.
        let piling = t % 100 < 20;
        let (x, y) = if piling {
            let c = self.world.player(self.slot).center;
            (c.x + 1.0, c.y)
        } else {
            (self.random() * 5000.0 - 2500.0, self.random() * 5000.0 - 2500.0)
        };
        let sc = self.world.player(self.slot).scramble;
        let (x, y) = (x + sc.x, y + sc.y);
        self.world.client_message(self.slot, ClientMsg::Mouse { x, y });
        if piling || self.random() < 0.1 {
            self.world.client_message(self.slot, ClientMsg::Eject);
        }
        self.world.tick(t as f64 * TICK_MS);
    }

    fn read(&mut self) {
        for frame in self.outbox.drain().frames {
            self.client.apply(&frame);
        }
    }

    /// The model holds exactly the cells the server thinks the client has, at
    /// the positions and sizes they have now.
    fn check(&mut self, t: u64) {
        let player = self.world.player(self.slot);
        let sc = player.scramble;
        let codec = self.client.codec;
        let wire_id = |id: u32| match codec {
            Codec::Ogar6 => id ^ sc.id,
            Codec::Compact => compact::scramble_id(id, &sc),
        };
        let view = &self.world.views[self.slot as usize];
        let believed: HashMap<u32, u32> =
            view.client_nodes.iter().map(|k| (wire_id(k.id), k.slot)).collect();
        assert_eq!(believed.len(), self.client.cells.len(), "tick {t}: cell count");
        let seen_cells: Vec<(u32, Seen)> = self.client.cells.iter().map(|(&i, &s)| (i, s)).collect();
        for (id, seen) in &seen_cells {
            let slot = *believed.get(id).unwrap_or_else(|| panic!("tick {t}: {id} not believed"));
            let cell = self.world.cells[slot].clone();
            assert_eq!(wire_id(cell.id), *id);
            let (x, y) = ((cell.pos.x + sc.x) as i32, (cell.pos.y + sc.y) as i32);
            assert!(
                (seen.x - x).abs() <= 1 && (seen.y - y).abs() <= 1,
                "tick {t}: cell {id} {:?} shown at {},{} but is at {x},{y}",
                cell.kind,
                seen.x,
                seen.y
            );
            match self.client.codec {
                Codec::Ogar6 => assert_eq!(seen.radius, cell.radius as u32 & 0xffff),
                Codec::Compact => assert_eq!(seen.radius, cell.radius as u32),
            }
            if cell.kind == Kind::Ejected {
                self.ejected_seen += 1;
            }
            if self.client.codec == Codec::Compact {
                self.check_compact(t, cell.kind, cell.is_mother, cell.owner, cell.unit, seen);
            }
        }
    }

    fn check_compact(&mut self, t: u64, kind: Kind, mother: bool, owner: u32, unit: u8, seen: &Seen) {
        let want = match kind {
            Kind::Food => compact::KIND_FOOD,
            Kind::Ejected => compact::KIND_EJECTED,
            Kind::Virus if mother => compact::KIND_MOTHER,
            Kind::Virus => compact::KIND_VIRUS,
            Kind::Player if owner == self.slot => compact::KIND_OWN + unit,
            Kind::Player => compact::KIND_PLAYER,
        };
        assert_eq!(seen.kind, want, "tick {t}: kind");
        if kind == Kind::Player {
            let owner = self.world.player(owner);
            assert_eq!(seen.owner, Some(owner.pid), "tick {t}: owner");
            assert_eq!(self.client.players[&owner.pid], owner.name, "tick {t}: owner's name");
        }
        if want >= compact::KIND_OWN {
            self.own_seen[usize::from(want - compact::KIND_OWN)] += 1;
        }
    }
}

fn play_reading_every_tick(codec: Codec, scramble: i32) -> Game {
    let mut game = Game::new(3, codec, scramble);
    for t in 1..=1500 {
        game.tick(t);
        game.read();
        game.check(t);
    }
    assert!(game.client.update_frames >= 1400);
    assert!(game.ejected_seen > 2_000, "only {} ejected cells checked", game.ejected_seen);
    game
}

fn play_falling_behind(codec: Codec) -> Game {
    let mut game = Game::new(5, codec, 0);
    let (mut skipped, mut checked, mut read_last_tick) = (0, 0, true);
    for t in 1..=3000 {
        game.tick(t);
        // Reads in bursts: nothing for 8 ticks, then everything queued.
        let stalled = (t / 9) % 2 == 1 && t % 9 != 0;
        if stalled {
            skipped += 1;
            read_last_tick = false;
            continue;
        }
        let fresh = read_last_tick && game.outbox.update_pending();
        game.read();
        // Only an update built this tick reflects the world as it is now.
        if fresh {
            game.check(t);
            checked += 1;
        }
        read_last_tick = true;
    }
    assert!(checked > 1000);
    assert!(game.ejected_seen > 2_000, "only {} ejected cells checked", game.ejected_seen);
    assert!(skipped > 1000);
    // While stalled the server holds updates back rather than queue them.
    assert!(game.client.update_frames < 3000 - skipped / 2);
    game
}

#[test]
fn protocol_6_client_that_reads_every_tick_stays_in_sync() {
    play_reading_every_tick(Codec::Ogar6, 0);
}

#[test]
fn protocol_6_client_that_falls_behind_catches_up() {
    play_falling_behind(Codec::Ogar6);
}

#[test]
fn protocol_6_client_stays_in_sync_when_scrambled() {
    play_reading_every_tick(Codec::Ogar6, 1);
}

#[test]
fn compact_client_that_reads_every_tick_stays_in_sync() {
    let game = play_reading_every_tick(Codec::Compact, 0);
    assert!(game.own_seen.iter().all(|&n| n > 1_000), "own cells seen {:?}", game.own_seen);
}

#[test]
fn compact_client_that_falls_behind_catches_up() {
    play_falling_behind(Codec::Compact);
}

#[test]
fn compact_client_stays_in_sync_when_scrambled() {
    play_reading_every_tick(Codec::Compact, 1);
}

/// The same world, seen through both protocols.
#[test]
fn compact_world_frames_are_much_smaller() {
    let ogar = play_reading_every_tick(Codec::Ogar6, 0).client;
    let compact = play_reading_every_tick(Codec::Compact, 0).client;
    let total = |c: &Client| c.update_bytes + c.other_bytes;
    eprintln!(
        "protocol 6: {} bytes in updates, {} in other frames; compact: {} and {}",
        ogar.update_bytes, ogar.other_bytes, compact.update_bytes, compact.other_bytes
    );
    assert!(total(&compact) * 2 < total(&ogar));
}
