mod block;

use std::collections::VecDeque;
use std::num::NonZeroU32;
use std::rc::Rc;

use serde::Serialize;
use thiserror::Error;

pub use self::block::BlockError;
use self::block::{pack_sights, unpack_sights, Block, BlockWriter, Sealed, BLOCK_MS};
use super::buf::{ReadError, Reader, Writer};
use super::hud::HudUpdate;
use super::proto::{Identity, RecordPolicy};
use super::session::Session;
use crate::world::{Aim, Tab, World};

const FILE_MAGIC: &[u8; 4] = b"OGRR";
const FILE_VERSION: u16 = 4;
const PROTOCOL_CODE: u8 = 0;
const FLAG_PERMANENT: u8 = 1;
const CHECKPOINTS: usize = 64;

#[derive(Debug, Error, PartialEq)]
pub enum ReelFileError {
    #[error("bad magic")]
    BadMagic,
    #[error("unsupported version {0}")]
    Version(u16),
    #[error("unknown protocol code {0}")]
    UnknownProtocol(u8),
    #[error("bad tab {0}")]
    Tab(u8),
    #[error("reel has no packets")]
    Empty,
    #[error("unknown packet flags {0:#04x}")]
    Flags(u8),
    #[error("text is not UTF-8")]
    Text,
    #[error("persona change before packet {0} is past the reel end")]
    PersonaPosition(u32),
    #[error("reel skips {0} packets of its first block")]
    Skip(u32),
    #[error("{0} bytes after the last section")]
    Trailing(usize),
    #[error("bad block: {0}")]
    Block(#[from] BlockError),
    #[error("truncated: {0}")]
    Truncated(#[from] ReadError),
}

fn tab_code(tab: Tab) -> u8 {
    u8::from(tab == Tab::Second)
}

fn tab_of(code: u8) -> Result<Tab, ReelFileError> {
    match code {
        0 => Ok(Tab::Main),
        1 => Ok(Tab::Second),
        t => Err(ReelFileError::Tab(t)),
    }
}

fn write_text(w: &mut Writer, text: &str) {
    w.u32(text.len() as u32).bytes(text.as_bytes());
}

fn read_text(r: &mut Reader) -> Result<String, ReelFileError> {
    let len = r.u32()? as usize;
    String::from_utf8(r.take(len)?.to_vec()).map_err(|_| ReelFileError::Text)
}

#[derive(Debug, Clone, Default, PartialEq)]
struct Persona {
    name: String,
    skin2: String,
    external_skins: [String; 2],
}

impl Persona {
    fn from_identity(identity: &Identity) -> Self {
        Self {
            name: identity.name.clone(),
            skin2: identity.skin2.clone(),
            external_skins: [
                identity.external_skin.clone(),
                identity.external_skin2.clone(),
            ],
        }
    }

    fn matches_identity(&self, identity: &Identity) -> bool {
        self.name == identity.name
            && self.skin2 == identity.skin2
            && self.external_skins[0] == identity.external_skin
            && self.external_skins[1] == identity.external_skin2
    }

    fn identity(&self) -> Identity {
        Identity {
            name: self.name.clone(),
            skin2: self.skin2.clone(),
            external_skin: self.external_skins[0].clone(),
            external_skin2: self.external_skins[1].clone(),
            ..Identity::default()
        }
    }

    fn write(&self, w: &mut Writer) {
        write_text(w, &self.name);
        write_text(w, &self.skin2);
        for skin in &self.external_skins {
            write_text(w, skin);
        }
    }

    fn read(r: &mut Reader) -> Result<Self, ReelFileError> {
        Ok(Self {
            name: read_text(r)?,
            skin2: read_text(r)?,
            external_skins: [read_text(r)?, read_text(r)?],
        })
    }
}

#[derive(Debug, Clone)]
struct PersonaChange {
    before: usize,
    tab: Tab,
    persona: Persona,
}

struct PersonaMark {
    seq: u64,
    tab: Tab,
    persona: Persona,
}

#[derive(Debug, Clone)]
struct Packet {
    at: f64,
    tab: Tab,
    permanent: bool,
    data: Rc<[u8]>,
}

impl Packet {
    fn write(&self, w: &mut Writer, offset: f64) {
        w.f64(offset)
            .u8(tab_code(self.tab))
            .u8(if self.permanent { FLAG_PERMANENT } else { 0 })
            .u32(self.data.len() as u32)
            .bytes(&self.data);
    }

    fn read(r: &mut Reader) -> Result<Packet, ReelFileError> {
        let at = r.f64()?;
        let tab = tab_of(r.u8()?)?;
        let flags = r.u8()?;
        if flags & !FLAG_PERMANENT != 0 {
            return Err(ReelFileError::Flags(flags));
        }
        let permanent = flags & FLAG_PERMANENT != 0;
        let len = r.u32()? as usize;
        Ok(Packet {
            at,
            tab,
            permanent,
            data: Rc::from(r.take(len)?),
        })
    }
}

struct Sequenced {
    seq: u64,
    packet: Packet,
}

struct LiveBlock {
    block: Rc<Block>,
    times: Box<[f64]>,
    seqs: Box<[u64]>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sight {
    pub at: f64,
    pub eye_x: f32,
    pub eye_y: f32,
    pub scale: f32,
    pub aim_x: f32,
    pub aim_y: f32,
    pub splitting: bool,
    pub aim_player: u16,
}

impl Sight {
    fn aim(self) -> Option<Aim> {
        (self.aim_x.is_finite() && self.aim_y.is_finite()).then_some(Aim {
            owner: u32::from(self.aim_player),
            x: self.aim_x,
            y: self.aim_y,
            splitting: self.splitting,
        })
    }

    fn lerp(self, next: Sight, at: f64) -> Sight {
        let t = ((at - self.at) / (next.at - self.at)) as f32;
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Sight {
            at,
            eye_x: mix(self.eye_x, next.eye_x),
            eye_y: mix(self.eye_y, next.eye_y),
            scale: mix(self.scale, next.scale),
            aim_x: mix(self.aim_x, next.aim_x),
            aim_y: mix(self.aim_y, next.aim_y),
            ..self
        }
    }
}

#[derive(Debug)]
struct Track {
    blocks: Vec<Rc<Block>>,
    skip: usize,
    start: f64,
    end: f64,
}

impl Track {
    fn new(blocks: Vec<Rc<Block>>, skip: usize, start: f64) -> Option<Track> {
        if skip >= blocks.first()?.len() {
            return None;
        }
        let end = blocks[blocks.len() - 1].last_at();
        Some(Track {
            blocks,
            skip,
            start,
            end,
        })
    }

    fn len(&self) -> usize {
        self.blocks.iter().map(|b| b.len()).sum::<usize>() - self.skip
    }

    fn bounds(&self) -> (f64, f64) {
        (self.start, self.end)
    }
}

#[derive(Debug)]
struct Reel {
    preface: Vec<Packet>,
    body: Track,
    sights: Vec<Sight>,
    openings: [Persona; 2],
    persona_changes: Vec<PersonaChange>,
}

impl Reel {
    fn bounds(&self) -> (f64, f64) {
        self.body.bounds()
    }

    fn duration(&self) -> f64 {
        let (start, end) = self.bounds();
        end - start
    }

    fn to_file_bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.bytes(FILE_MAGIC)
            .u16(FILE_VERSION)
            .u8(PROTOCOL_CODE)
            .u32(self.preface.len() as u32);
        for p in &self.preface {
            p.write(&mut w, 0.0);
        }
        w.u32(self.body.skip as u32)
            .f64(self.body.start)
            .u32(self.body.blocks.len() as u32);
        for b in &self.body.blocks {
            w.u32(b.raw_len())
                .u32(b.len() as u32)
                .f64(b.first_at())
                .f64(b.last_at())
                .u32(b.stored_len() as u32)
                .bytes(b.bytes());
        }
        let (raw_len, sights) = pack_sights(&self.sights);
        w.u32(self.sights.len() as u32)
            .u32(raw_len)
            .u32(sights.len() as u32)
            .bytes(&sights);
        self.write_personas(&mut w);
        w.finish()
    }

    fn trimmed(&self, from: f64, to: f64) -> Result<Self, TrimError> {
        let empty = || TrimError::EmptyRange { from, to };
        let (origin, _) = self.bounds();
        let (start, cut) = (origin + from, origin + to);
        let source = &self.body.blocks;
        let keep = source.partition_point(|b| b.first_at() <= cut);
        if from < 0.0 || from >= to || keep == 0 {
            return Err(empty());
        }
        let mut blocks = source[..keep].to_vec();
        if blocks[keep - 1].last_at() > cut {
            let mut writer = BlockWriter::default();
            for p in blocks[keep - 1].decode()?.iter().filter(|p| p.at <= cut) {
                writer.push(p);
            }
            let sealed = writer.seal().ok_or_else(empty)?;
            blocks[keep - 1] = Rc::new(sealed.block);
        }
        let body = Track::new(blocks, self.body.skip, start)
            .filter(|t| t.end > start)
            .ok_or_else(empty)?;
        let packets = body.len();
        Ok(Self {
            preface: self.preface.clone(),
            body,
            sights: self
                .sights
                .iter()
                .filter(|s| (start..=cut).contains(&s.at))
                .copied()
                .collect(),
            openings: self.openings.clone(),
            persona_changes: self
                .persona_changes
                .iter()
                .filter(|c| c.before <= packets)
                .cloned()
                .collect(),
        })
    }

    fn write_personas(&self, w: &mut Writer) {
        for persona in &self.openings {
            persona.write(w);
        }
        w.u32(self.persona_changes.len() as u32);
        for c in &self.persona_changes {
            w.u32(c.before as u32).u8(tab_code(c.tab));
            c.persona.write(w);
        }
    }

    fn camera_at(&self, position: f64) -> Option<Sight> {
        let (start, _) = self.bounds();
        let at = start + position;
        let next = self.sights.partition_point(|s| s.at <= at);
        let sight = match (
            next.checked_sub(1).map(|i| self.sights[i]),
            self.sights.get(next),
        ) {
            (Some(prev), Some(&next)) => prev.lerp(next, at),
            (Some(only), None) | (None, Some(&only)) => only,
            (None, None) => return None,
        };
        Some(Sight {
            at: sight.at - start,
            ..sight
        })
    }

    fn from_file_bytes(bytes: &[u8]) -> Result<Self, ReelFileError> {
        let mut r = Reader::new(bytes);
        if r.take(FILE_MAGIC.len())? != FILE_MAGIC {
            return Err(ReelFileError::BadMagic);
        }
        match r.u16()? {
            FILE_VERSION => {}
            v => return Err(ReelFileError::Version(v)),
        }
        match r.u8()? {
            PROTOCOL_CODE => {}
            c => return Err(ReelFileError::UnknownProtocol(c)),
        }
        let mut reel = Self::parse_blocks(&mut r)?;
        reel.openings = [Persona::read(&mut r)?, Persona::read(&mut r)?];
        let body_len = reel.body.len();
        reel.persona_changes = (0..r.u32()?)
            .map(|_| Self::persona_change(&mut r, body_len))
            .collect::<Result<Vec<_>, _>>()?;
        if !r.is_empty() {
            return Err(ReelFileError::Trailing(r.remaining()));
        }
        Ok(reel)
    }

    fn parse_blocks(r: &mut Reader) -> Result<Self, ReelFileError> {
        let preface = (0..r.u32()?)
            .map(|_| Packet::read(r))
            .collect::<Result<Vec<_>, _>>()?;
        let skip = r.u32()?;
        let start = r.f64()?;
        let count = r.u32()?;
        if count == 0 {
            return Err(ReelFileError::Empty);
        }
        let mut blocks = Vec::new();
        for _ in 0..count {
            let raw_len = r.u32()?;
            let count = r.u32()?;
            let span = (r.f64()?, r.f64()?);
            let len = r.u32()? as usize;
            blocks.push(Rc::new(Block::from_file(
                raw_len,
                count,
                span,
                r.take(len)?,
            )?));
        }
        let body = Track::new(blocks, skip as usize, start)
            .filter(|t| skip > 0 || start >= t.blocks[0].first_at())
            .ok_or(ReelFileError::Skip(skip))?;
        let count = r.u32()?;
        let raw_len = r.u32()?;
        let len = r.u32()? as usize;
        let sights = unpack_sights(raw_len, count, r.take(len)?)?;
        Ok(Self {
            preface,
            body,
            sights,
            openings: Default::default(),
            persona_changes: Vec::new(),
        })
    }

    fn persona_change(r: &mut Reader, body_len: usize) -> Result<PersonaChange, ReelFileError> {
        let before = r.u32()?;
        if before as usize > body_len {
            return Err(ReelFileError::PersonaPosition(before));
        }
        Ok(PersonaChange {
            before: before as usize,
            tab: tab_of(r.u8()?)?,
            persona: Persona::read(r)?,
        })
    }
}

struct Recording {
    preface: Vec<Packet>,
    sights: Vec<Sight>,
    openings: [Persona; 2],
    persona_changes: Vec<PersonaChange>,
    blocks: Vec<Rc<Block>>,
    skip: usize,
    packets: usize,
    start: Option<f64>,
}

#[derive(Default)]
struct Recorder {
    open: BlockWriter,
    open_seqs: Vec<u64>,
    sealed: VecDeque<LiveBlock>,
    sights: VecDeque<Sight>,
    permanent: Vec<Sequenced>,
    personas: Vec<PersonaMark>,
    recording: Option<Recording>,
    next_seq: u64,
    started: Option<f64>,
    latest: f64,
    window_ms: Option<f64>,
}

impl Recorder {
    fn record(&mut self, tab: Tab, data: &[u8], policy: RecordPolicy, now: f64) {
        self.started.get_or_insert(now);
        self.latest = now;
        let permanent = policy == RecordPolicy::Keep;
        if policy == RecordPolicy::Skip || !(self.window_ms.is_some() || permanent) {
            return;
        }
        let packet = Packet {
            at: now,
            tab,
            permanent,
            data: Rc::from(data),
        };
        let seq = self.next_seq;
        self.next_seq += 1;
        if permanent {
            self.permanent.push(Sequenced {
                seq,
                packet: packet.clone(),
            });
        }
        if self.window_ms.is_none() && self.recording.is_none() {
            return;
        }
        if let Some(recording) = self.recording.as_mut() {
            recording.start.get_or_insert(now);
            recording.packets += 1;
        }
        self.open.push(&packet);
        self.open_seqs.push(seq);
        let block_ms = self.window_ms.map_or(BLOCK_MS, |ms| BLOCK_MS.min(ms));
        if self.open.span() >= block_ms {
            self.seal_open_block();
        }
        if let Some(ms) = self.window_ms {
            self.drop_blocks_before(now - ms);
        }
    }

    fn seal_open_block(&mut self) {
        let Some(Sealed { block, times }) = self.open.seal() else {
            return;
        };
        let block = Rc::new(block);
        let seqs = std::mem::take(&mut self.open_seqs).into_boxed_slice();
        if let Some(recording) = self.recording.as_mut() {
            recording.blocks.push(Rc::clone(&block));
        }
        if self.window_ms.is_some() {
            self.sealed.push_back(LiveBlock { block, times, seqs });
        }
    }

    fn drop_blocks_before(&mut self, limit: f64) {
        while self.sealed.front().is_some() {
            let next_first = self
                .sealed
                .get(1)
                .map(|b| b.block.first_at())
                .or_else(|| self.open.first_at());
            if !next_first.is_some_and(|at| at < limit) {
                break;
            }
            self.sealed.pop_front();
        }
    }

    fn clear_window(&mut self) {
        self.seal_open_block();
        self.sealed.clear();
        self.sights.clear();
    }

    fn record_sight(&mut self, sight: Sight) {
        let Some(window_ms) = self.window_ms else {
            return;
        };
        if let Some(recording) = self.recording.as_mut() {
            recording.sights.push(sight);
        }
        self.sights.push_back(sight);
        let limit = sight.at - window_ms;
        while self.sights.front().is_some_and(|s| s.at < limit) {
            self.sights.pop_front();
        }
    }

    fn buffered_packets(&self) -> usize {
        self.sealed.iter().map(|b| b.block.len()).sum::<usize>() + self.open.len()
    }

    fn capture_window(&mut self, now: f64) -> Option<Reel> {
        self.seal_open_block();
        let window_ms = self.window_ms?;
        let positions: Vec<(usize, usize)> = self
            .sealed
            .iter()
            .enumerate()
            .flat_map(|(b, block)| (0..block.times.len()).map(move |i| (b, i)))
            .filter(|&(b, i)| self.sealed[b].times[i] >= self.latest - window_ms)
            .collect();
        let at = |(b, i): (usize, usize)| self.sealed[b].times[i];
        let tail = positions.len().checked_sub(1)?;
        let started = self.started.expect("a non-empty window has a start");
        let span = window_ms.min(now - started);
        let limit = now - span;
        let mut head = tail;
        while head > 0 && at(positions[head - 1]) >= limit {
            head -= 1;
        }
        let (first_block, skip) = positions[head.saturating_sub(1)];
        let live = &self.sealed[first_block];
        let (first, start) = (live.seqs[skip], live.times[skip]);
        let window = self.sealed.range(first_block..);
        let body = Track::new(
            window.clone().map(|b| Rc::clone(&b.block)).collect(),
            skip,
            start,
        )?;
        let seqs: Vec<u64> = window
            .flat_map(|b| b.seqs.iter().copied())
            .skip(skip)
            .collect();
        let persona_changes = self
            .personas
            .iter()
            .filter(|m| m.seq >= first)
            .map(|m| PersonaChange {
                before: seqs.partition_point(|&s| s < m.seq),
                tab: m.tab,
                persona: m.persona.clone(),
            })
            .collect();
        Some(Reel {
            preface: self
                .permanent
                .iter()
                .take_while(|p| p.seq < first)
                .map(|p| p.packet.clone())
                .collect(),
            body,
            sights: self
                .sights
                .iter()
                .filter(|s| s.at >= start)
                .copied()
                .collect(),
            openings: self.personas_before(first),
            persona_changes,
        })
    }

    fn personas_before(&self, seq: u64) -> [Persona; 2] {
        [Tab::Main, Tab::Second].map(|tab| {
            self.personas
                .iter()
                .rev()
                .find(|m| m.tab == tab && m.seq < seq)
                .map_or_else(Persona::default, |m| m.persona.clone())
        })
    }

    fn note_identity(&mut self, tab: Tab, identity: &Identity) {
        let current = self.personas.iter().rev().find(|m| m.tab == tab);
        if current.is_some_and(|m| m.persona.matches_identity(identity)) {
            return;
        }
        let persona = Persona::from_identity(identity);
        if let Some(recording) = self.recording.as_mut() {
            recording.persona_changes.push(PersonaChange {
                before: recording.packets,
                tab,
                persona: persona.clone(),
            });
        }
        self.personas.push(PersonaMark {
            seq: self.next_seq,
            tab,
            persona,
        });
        self.next_seq += 1;
    }

    fn start_recording(&mut self, preface: Vec<Packet>, openings: [Persona; 2]) {
        self.recording = Some(Recording {
            preface,
            sights: Vec::new(),
            openings,
            persona_changes: Vec::new(),
            blocks: Vec::new(),
            skip: self.open.len(),
            packets: 0,
            start: None,
        });
    }

    fn take_recording(&mut self) -> Option<Reel> {
        self.seal_open_block();
        let Recording {
            preface,
            sights,
            openings,
            persona_changes,
            mut blocks,
            mut skip,
            start,
            ..
        } = self.recording.take()?;
        let start = start?;
        let whole = blocks.iter().take_while(|b| skip >= b.len()).count();
        for b in blocks.drain(..whole) {
            skip -= b.len();
        }
        Some(Reel {
            preface,
            body: Track::new(blocks, skip, start)?,
            sights,
            openings,
            persona_changes,
        })
    }

    fn on_new_connection(&mut self) {
        self.clear_window();
        self.permanent.clear();
        self.personas.clear();
        self.started = None;
    }
}

pub type ReelId = u32;

#[derive(Debug, Error)]
#[error("unknown reel {0}")]
pub struct UnknownReel(pub ReelId);

#[derive(Debug, Error)]
pub enum TrimError {
    #[error(transparent)]
    Unknown(#[from] UnknownReel),
    #[error("the range {from}..{to} keeps nothing")]
    EmptyRange { from: f64, to: f64 },
    #[error(transparent)]
    Block(#[from] BlockError),
}

#[derive(Debug, Clone, Error, PartialEq)]
pub enum PlaybackError {
    #[error(transparent)]
    Packet(#[from] ReadError),
    #[error("block {index} is damaged: {source}")]
    Block { index: usize, source: BlockError },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReelOrigin {
    Live,
    Imported,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReelInfo {
    pub id: ReelId,
    pub origin: ReelOrigin,
    pub duration: f64,
}

#[derive(Debug)]
struct StoredReel {
    id: ReelId,
    origin: ReelOrigin,
    reel: Reel,
}

fn find_reel(reels: &[StoredReel], id: ReelId) -> Result<&StoredReel, UnknownReel> {
    reels.iter().find(|r| r.id == id).ok_or(UnknownReel(id))
}

#[derive(Default)]
pub struct ReplayLibrary {
    recorder: Recorder,
    connected: bool,
    reels: Vec<StoredReel>,
    next_id: ReelId,
    replay: Replay,
    export_replay: Replay,
}

impl ReplayLibrary {
    pub fn set_window(&mut self, window_ms: Option<NonZeroU32>) {
        if window_ms.is_none() {
            self.recorder.clear_window();
        }
        self.recorder.window_ms = window_ms.map(|ms| f64::from(ms.get()));
    }

    pub fn on_connect(&mut self) {
        if self.recorder.recording.is_some() {
            self.store_recording();
            self.recorder
                .start_recording(Vec::new(), Default::default());
        }
        self.recorder.on_new_connection();
        self.connected = true;
    }

    pub fn record(&mut self, tab: Tab, data: &[u8], policy: RecordPolicy, now: f64) {
        self.recorder.record(tab, data, policy, now);
    }

    pub fn record_sight(&mut self, sight: Sight) {
        self.recorder.record_sight(sight);
    }

    pub fn note_identity(&mut self, tab: Tab, identity: &Identity) {
        self.recorder.note_identity(tab, identity);
    }

    pub fn camera_at(&self, id: ReelId, position: f64) -> Option<Sight> {
        find_reel(&self.reels, id).ok()?.reel.camera_at(position)
    }

    fn store_reel(&mut self, origin: ReelOrigin, reel: Reel) -> ReelId {
        self.next_id += 1;
        self.reels.push(StoredReel {
            id: self.next_id,
            origin,
            reel,
        });
        self.next_id
    }

    fn store_recording(&mut self) -> Option<ReelId> {
        if !self.connected {
            return None;
        }
        let reel = self.recorder.take_recording()?;
        Some(self.store_reel(ReelOrigin::Live, reel))
    }

    pub fn capture_window(&mut self, now: f64) -> Option<ReelId> {
        if !self.connected {
            return None;
        }
        let reel = self.recorder.capture_window(now)?;
        Some(self.store_reel(ReelOrigin::Live, reel))
    }

    pub fn toggle_recording(&mut self) -> Option<ReelId> {
        if self.recorder.recording.is_some() {
            return self.store_recording();
        }
        let preface = self
            .recorder
            .permanent
            .iter()
            .map(|p| p.packet.clone())
            .collect();
        let openings = self.recorder.personas_before(u64::MAX);
        self.recorder.start_recording(preface, openings);
        None
    }

    pub fn is_recording(&self) -> bool {
        self.recorder.recording.is_some()
    }

    pub fn buffered_packets(&self) -> usize {
        self.recorder.buffered_packets()
    }

    pub fn reel_infos(&self) -> Vec<ReelInfo> {
        self.reels
            .iter()
            .map(|r| ReelInfo {
                id: r.id,
                origin: r.origin,
                duration: r.reel.duration(),
            })
            .collect()
    }

    pub fn import_reel(&mut self, bytes: &[u8]) -> Result<ReelId, ReelFileError> {
        let reel = Reel::from_file_bytes(bytes)?;
        Ok(self.store_reel(ReelOrigin::Imported, reel))
    }

    pub fn export_reel(&self, id: ReelId) -> Option<Vec<u8>> {
        let r = find_reel(&self.reels, id).ok()?;
        Some(r.reel.to_file_bytes())
    }

    pub fn export_trimmed_reel(
        &self,
        id: ReelId,
        from: f64,
        to: f64,
    ) -> Result<Vec<u8>, TrimError> {
        let r = find_reel(&self.reels, id)?;
        Ok(r.reel.trimmed(from, to)?.to_file_bytes())
    }

    pub fn delete_reel(&mut self, id: ReelId) {
        self.reels.retain(|r| r.id != id);
        for replay in [&mut self.replay, &mut self.export_replay] {
            if replay.active.as_ref().is_some_and(|a| a.reel == id) {
                replay.reset();
            }
        }
    }

    pub fn clear_library(&mut self) {
        self.reels.clear();
        self.recorder.clear_window();
        self.recorder.recording = None;
        self.replay.reset();
        self.export_replay.reset();
    }

    pub fn seek(
        &mut self,
        id: ReelId,
        position: f64,
        world: &mut World,
        now: f64,
    ) -> Result<Vec<PlaybackError>, UnknownReel> {
        let r = find_reel(&self.reels, id)?;
        Ok(self.replay.seek(r, position, world, now))
    }

    pub fn stop_playback(&mut self, world: &mut World) {
        self.replay.stop_playback(world);
    }

    pub fn scout_ahead(&mut self, id: ReelId, step_ms: f64, now: f64) -> Result<bool, UnknownReel> {
        let r = find_reel(&self.reels, id)?;
        Ok(self.replay.scout_ahead(r, step_ms, now))
    }

    pub fn take_hud(&mut self) -> Option<HudUpdate> {
        self.replay.take_hud()
    }

    pub fn seek_for_export(
        &mut self,
        id: ReelId,
        position: f64,
        world: &mut World,
        now: f64,
    ) -> Result<Vec<PlaybackError>, UnknownReel> {
        let r = find_reel(&self.reels, id)?;
        let errors = self.export_replay.seek(r, position, world, now);
        self.export_replay.take_hud();
        Ok(errors)
    }

    pub fn end_export(&mut self) {
        self.export_replay.reset();
    }
}

struct Checkpoint {
    block: usize,
    consumed: usize,
    next_change: usize,
    reached: f64,
    session: Session,
    world: World,
    errors: Vec<PlaybackError>,
}

#[derive(Default)]
struct Checkpoints {
    list: Vec<Checkpoint>,
}

impl Checkpoints {
    fn add_at(&mut self, player: &Player, world: &World) {
        if let Err(at) = self.list.binary_search_by_key(&player.block, |c| c.block) {
            self.list.insert(
                at,
                Checkpoint {
                    block: player.block,
                    consumed: player.consumed,
                    next_change: player.next_change,
                    reached: player.reached,
                    session: player.session.clone(),
                    world: world.clone(),
                    errors: player.errors.clone(),
                },
            );
        }
    }

    fn latest_before(&self, target: f64) -> Option<&Checkpoint> {
        self.list.iter().rev().find(|c| c.reached <= target)
    }
}

struct Player {
    session: Session,
    block: usize,
    index: usize,
    packets: Vec<Packet>,
    consumed: usize,
    next_change: usize,
    reached: f64,
    position: f64,
    errors: Vec<PlaybackError>,
    reported: usize,
}

impl Player {
    fn new(source: &StoredReel, world: &mut World, now: f64) -> Self {
        world.clear();
        let mut player = Self {
            session: Session::default(),
            block: 0,
            index: source.reel.body.skip,
            packets: Vec::new(),
            consumed: 0,
            next_change: 0,
            reached: f64::NEG_INFINITY,
            position: 0.0,
            errors: Vec::new(),
            reported: 0,
        };
        for p in &source.reel.preface {
            player.receive(source, p.tab, &p.data, world, now);
        }
        player.discard_outgoing();
        player
    }

    fn receive(&mut self, source: &StoredReel, tab: Tab, data: &[u8], world: &mut World, now: f64) {
        if !self.session.is_connected(tab) {
            let persona = source.reel.persona_changes[..self.next_change]
                .iter()
                .rev()
                .find(|c| c.tab == tab)
                .map_or(&source.reel.openings[tab.index()], |c| &c.persona);
            self.session.on_open(tab, persona.identity(), now);
        }
        if let Err(e) = self.session.receive(tab, data, world, now) {
            self.errors.push(e.into());
        }
    }

    fn from_checkpoint(from: &Checkpoint, reported: usize, world: &mut World) -> Self {
        world.restore(&from.world);
        Self {
            session: from.session.clone(),
            block: from.block,
            index: 0,
            packets: Vec::new(),
            consumed: from.consumed,
            next_change: from.next_change,
            reached: from.reached,
            position: f64::NEG_INFINITY,
            errors: from.errors.clone(),
            reported,
        }
    }

    fn discard_outgoing(&mut self) {
        for tab in [Tab::Main, Tab::Second] {
            while self.session.take_outgoing(tab).is_some() {}
        }
    }

    fn is_finished(&self, source: &StoredReel) -> bool {
        self.block >= source.reel.body.blocks.len()
    }

    fn advance_to(
        &mut self,
        source: &StoredReel,
        position: f64,
        world: &mut World,
        now: f64,
        checkpoints: &mut Checkpoints,
    ) {
        let (start, _) = source.reel.bounds();
        let target = start + position;
        let blocks = &source.reel.body.blocks;
        let stride = blocks.len().div_ceil(CHECKPOINTS);
        loop {
            if self.packets.is_empty() {
                let Some(block) = blocks.get(self.block) else {
                    break;
                };
                match block.decode() {
                    Ok(packets) => self.packets = packets,
                    Err(source) => {
                        self.errors.push(PlaybackError::Block {
                            index: self.block,
                            source,
                        });
                        self.block = blocks.len();
                        break;
                    }
                }
            }
            let Some(p) = self.packets.get(self.index) else {
                self.block += 1;
                self.index = 0;
                self.packets.clear();
                if self.block.is_multiple_of(stride) && self.block < blocks.len() {
                    checkpoints.add_at(self, world);
                }
                continue;
            };
            if p.at > target {
                break;
            }
            while let Some(c) = source
                .reel
                .persona_changes
                .get(self.next_change)
                .filter(|c| c.before <= self.consumed)
            {
                self.session.set_identity(c.tab, c.persona.identity());
                self.next_change += 1;
            }
            let (at, tab, data) = (p.at, p.tab, Rc::clone(&p.data));
            self.receive(source, tab, &data, world, now);
            self.reached = at;
            self.index += 1;
            self.consumed += 1;
        }
        self.position = position;
        self.discard_outgoing();
    }
}

struct Scout {
    player: Player,
    world: World,
}

struct Active {
    reel: ReelId,
    player: Option<Player>,
    scout: Option<Scout>,
    checkpoints: Checkpoints,
}

#[derive(Default)]
struct Replay {
    active: Option<Active>,
}

impl Replay {
    fn select_reel(&mut self, reel: ReelId) -> &mut Active {
        let active = self
            .active
            .take()
            .filter(|a| a.reel == reel)
            .unwrap_or_else(|| Active {
                reel,
                player: None,
                scout: None,
                checkpoints: Checkpoints::default(),
            });
        self.active.insert(active)
    }

    fn seek(
        &mut self,
        source: &StoredReel,
        position: f64,
        world: &mut World,
        now: f64,
    ) -> Vec<PlaybackError> {
        let target = source.reel.bounds().0 + position;
        let active = self.select_reel(source.id);
        let best = active.checkpoints.latest_before(target);
        let (mut player, jumped) = match (active.player.take(), best) {
            (Some(p), Some(c)) if position < p.position => {
                (Player::from_checkpoint(c, 0, world), true)
            }
            (Some(p), Some(c)) if c.consumed > p.consumed => {
                (Player::from_checkpoint(c, p.reported, world), true)
            }
            (Some(p), _) if position >= p.position => (p, false),
            (_, Some(c)) => (Player::from_checkpoint(c, 0, world), true),
            _ => (Player::new(source, world, now), true),
        };
        player.advance_to(source, position, world, now, &mut active.checkpoints);
        if jumped {
            world.settle();
        }
        if let Some(sight) = source.reel.camera_at(position) {
            world.set_spectate_aim(sight.aim());
        }
        let errors = player.errors[player.reported..].to_vec();
        player.reported = player.errors.len();
        active.player = Some(player);
        errors
    }

    fn scout_ahead(&mut self, source: &StoredReel, step_ms: f64, now: f64) -> bool {
        let active = self.select_reel(source.id);
        let scout = active.scout.get_or_insert_with(|| {
            let mut world = World::new();
            let player = Player::new(source, &mut world, now);
            Scout { player, world }
        });
        if !scout.player.is_finished(source) {
            let position = scout.player.position + step_ms;
            scout.player.advance_to(
                source,
                position,
                &mut scout.world,
                now,
                &mut active.checkpoints,
            );
        }
        scout.player.is_finished(source)
    }

    fn reset(&mut self) {
        self.active = None;
    }

    fn stop_playback(&mut self, world: &mut World) {
        self.reset();
        world.clear();
    }

    fn take_hud(&mut self) -> Option<HudUpdate> {
        self.active.as_mut()?.player.as_mut()?.session.take_hud()
    }
}
