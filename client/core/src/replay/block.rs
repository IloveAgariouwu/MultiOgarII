use std::rc::Rc;

use miniz_oxide::deflate::compress_to_vec;
use miniz_oxide::deflate::core::{
    compress_to_output, create_comp_flags_from_zip_params, CompressorOxide, TDEFLFlush, TDEFLStatus,
};
use miniz_oxide::inflate::decompress_to_vec_with_limit;
use thiserror::Error;

use super::{Packet, Sight};
use crate::world::Tab;

pub(super) const BLOCK_MS: f64 = 10_000.0;
const LEVEL: u8 = 6;
const MAX_RAW_LEN: u32 = 256 << 20;

const FLAG_SECOND: u8 = 1;
const FLAG_PERMANENT: u8 = 2;

const META: usize = 0;
const OPAQUE: usize = 1;
const STREAMS: usize = 2;
const COLUMNS: usize = 14;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum BlockError {
    #[error("block does not inflate")]
    Inflate,
    #[error("block inflates to {found} bytes, expected {expected}")]
    Size { expected: u32, found: usize },
    #[error("block column sizes overflow")]
    SizeOverflow,
    #[error("block is truncated")]
    Truncated,
    #[error("block has {0} unread bytes")]
    Trailing(usize),
    #[error("unknown packet flags {0:#04x}")]
    Flags(u8),
    #[error("block holds {found} packets, expected {expected}")]
    Count { expected: u32, found: usize },
    #[error("block raw size {0} is over the limit")]
    TooLarge(u32),
    #[error("block holds no packets")]
    Empty,
    #[error("block times do not match its recorded span")]
    Span,
    #[error("sight splitting byte {0} is not 0 or 1")]
    Splitting(u8),
    #[error("sight aim player {0} is out of range")]
    AimPlayer(u64),
}

fn put(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn zig32(v: i32) -> u64 {
    u64::from(((v << 1) ^ (v >> 31)) as u32)
}

fn zig64(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

fn unzig32(v: u64) -> i32 {
    let v = v as u32;
    (v >> 1) as i32 ^ -((v & 1) as i32)
}

fn unzig64(v: u64) -> i64 {
    (v >> 1) as i64 ^ -((v & 1) as i64)
}

struct Col<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Col<'a> {
    fn var(&mut self) -> Result<u64, BlockError> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.byte()?;
            v |= u64::from(b & 0x7f) << shift;
            if b < 0x80 {
                return Ok(v);
            }
        }
        Err(BlockError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, BlockError> {
        let b = *self.data.get(self.pos).ok_or(BlockError::Truncated)?;
        self.pos += 1;
        Ok(b)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], BlockError> {
        let slice = self
            .data
            .get(self.pos..self.pos + n)
            .ok_or(BlockError::Truncated)?;
        self.pos += n;
        Ok(slice)
    }

    fn len(&mut self) -> Result<usize, BlockError> {
        usize::try_from(self.var()?).map_err(|_| BlockError::Truncated)
    }
}

struct ColumnStream {
    compressor: Box<CompressorOxide>,
    packed: Vec<u8>,
    raw: usize,
}

impl Default for ColumnStream {
    fn default() -> Self {
        Self {
            compressor: Box::new(CompressorOxide::new(create_comp_flags_from_zip_params(
                i32::from(LEVEL),
                0,
                0,
            ))),
            packed: Vec::new(),
            raw: 0,
        }
    }
}

impl ColumnStream {
    fn feed(&mut self, mut input: &[u8], flush: TDEFLFlush) {
        self.raw += input.len();
        loop {
            let packed = &mut self.packed;
            let (status, used) = compress_to_output(&mut self.compressor, input, flush, |out| {
                packed.extend_from_slice(out);
                true
            });
            input = &input[used..];
            match status {
                TDEFLStatus::Done => return,
                TDEFLStatus::Okay if input.is_empty() && flush == TDEFLFlush::None => return,
                TDEFLStatus::Okay => {}
                other => unreachable!("deflate into a growing buffer cannot fail: {other:?}"),
            }
        }
    }

    fn finish(&mut self) -> (usize, Vec<u8>) {
        self.feed(&[], TDEFLFlush::Finish);
        self.compressor.reset();
        (
            std::mem::take(&mut self.raw),
            std::mem::take(&mut self.packed),
        )
    }
}

#[derive(Default)]
pub(super) struct BlockWriter {
    cols: [Vec<u8>; STREAMS],
    streams: [ColumnStream; STREAMS],
    times: Vec<f64>,
    last_bits: u64,
}

pub(super) struct Sealed {
    pub(super) block: Block,
    pub(super) times: Box<[f64]>,
}

impl BlockWriter {
    pub(super) fn len(&self) -> usize {
        self.times.len()
    }

    pub(super) fn first_at(&self) -> Option<f64> {
        self.times.first().copied()
    }

    pub(super) fn span(&self) -> f64 {
        match (self.times.first(), self.times.last()) {
            (Some(first), Some(last)) => last - first,
            _ => 0.0,
        }
    }

    pub(super) fn push(&mut self, packet: &Packet) {
        let bits = packet.at.to_bits();
        let mut flags = 0;
        if packet.tab == Tab::Second {
            flags |= FLAG_SECOND;
        }
        if packet.permanent {
            flags |= FLAG_PERMANENT;
        }
        let meta = &mut self.cols[META];
        meta.push(flags);
        put(meta, zig64(bits.wrapping_sub(self.last_bits) as i64));
        put(meta, packet.data.len() as u64);
        self.last_bits = bits;
        self.times.push(packet.at);
        self.cols[OPAQUE].extend_from_slice(&packet.data);
        for (col, stream) in self.cols.iter_mut().zip(&mut self.streams) {
            if !col.is_empty() {
                stream.feed(col, TDEFLFlush::None);
                col.clear();
            }
        }
    }

    pub(super) fn seal(&mut self) -> Option<Sealed> {
        if self.times.is_empty() {
            return None;
        }
        let columns: Vec<(usize, Vec<u8>)> =
            self.streams.iter_mut().map(ColumnStream::finish).collect();
        let mut bytes = Vec::with_capacity(columns.iter().map(|c| c.1.len()).sum::<usize>() + 64);
        for (raw, _) in &columns {
            put(&mut bytes, *raw as u64);
        }
        bytes.extend_from_slice(&[0; COLUMNS - STREAMS]);
        for (_, packed) in &columns {
            put(&mut bytes, packed.len() as u64);
        }
        bytes.extend_from_slice(&[0; COLUMNS - STREAMS]);
        for (_, packed) in &columns {
            bytes.extend_from_slice(packed);
        }
        self.last_bits = 0;
        let times: Box<[f64]> = std::mem::take(&mut self.times).into();
        Some(Sealed {
            block: Block {
                count: times.len() as u32,
                first_at: times[0],
                last_at: times[times.len() - 1],
                raw_len: columns.iter().map(|c| c.0).sum::<usize>() as u32,
                bytes: bytes.into(),
            },
            times,
        })
    }
}

struct Layout {
    raw: [usize; COLUMNS],
    packed: [usize; COLUMNS],
    start: usize,
}

#[derive(Debug)]
pub(super) struct Block {
    count: u32,
    first_at: f64,
    last_at: f64,
    raw_len: u32,
    bytes: Box<[u8]>,
}

impl Block {
    pub(super) fn len(&self) -> usize {
        self.count as usize
    }

    pub(super) fn first_at(&self) -> f64 {
        self.first_at
    }

    pub(super) fn last_at(&self) -> f64 {
        self.last_at
    }

    pub(super) fn stored_len(&self) -> usize {
        self.bytes.len()
    }

    pub(super) fn raw_len(&self) -> u32 {
        self.raw_len
    }

    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(super) fn from_file(
        raw_len: u32,
        count: u32,
        (first_at, last_at): (f64, f64),
        bytes: &[u8],
    ) -> Result<Block, BlockError> {
        if count == 0 {
            return Err(BlockError::Empty);
        }
        let block = Block {
            count,
            first_at,
            last_at,
            raw_len,
            bytes: bytes.into(),
        };
        block.layout()?;
        Ok(block)
    }

    fn layout(&self) -> Result<Layout, BlockError> {
        if self.raw_len > MAX_RAW_LEN {
            return Err(BlockError::TooLarge(self.raw_len));
        }
        let mut lens = Col {
            data: &self.bytes,
            pos: 0,
        };
        let mut raw = [0; COLUMNS];
        for len in &mut raw {
            *len = lens.len()?;
        }
        let mut packed = [0; COLUMNS];
        for len in &mut packed {
            *len = lens.len()?;
        }
        let total = raw
            .iter()
            .try_fold(0usize, |sum, &n| sum.checked_add(n))
            .ok_or(BlockError::SizeOverflow)?;
        if total != self.raw_len as usize {
            return Err(BlockError::Size {
                expected: self.raw_len,
                found: total,
            });
        }
        let end = packed
            .iter()
            .try_fold(lens.pos, |sum, &n| sum.checked_add(n))
            .ok_or(BlockError::Truncated)?;
        match end.cmp(&self.bytes.len()) {
            std::cmp::Ordering::Equal => {}
            std::cmp::Ordering::Less => return Err(BlockError::Trailing(self.bytes.len() - end)),
            std::cmp::Ordering::Greater => return Err(BlockError::Truncated),
        }
        Ok(Layout {
            raw,
            packed,
            start: lens.pos,
        })
    }

    pub(super) fn decode(&self) -> Result<Vec<Packet>, BlockError> {
        let layout = self.layout()?;
        let mut at = layout.start;
        let mut columns: [Vec<u8>; COLUMNS] = Default::default();
        for ((column, &raw_len), &size) in columns.iter_mut().zip(&layout.raw).zip(&layout.packed) {
            if raw_len > 0 {
                *column = decompress_to_vec_with_limit(&self.bytes[at..at + size], raw_len)
                    .map_err(|_| BlockError::Inflate)?;
                if column.len() != raw_len {
                    return Err(BlockError::Inflate);
                }
            }
            at += size;
        }
        let mut cols = columns.each_ref().map(|data| Col { data, pos: 0 });
        let mut packets = Vec::new();
        let mut bits = 0u64;
        while cols[META].pos < cols[META].data.len() {
            let flags = cols[META].byte()?;
            if flags & !(FLAG_SECOND | FLAG_PERMANENT) != 0 {
                return Err(BlockError::Flags(flags));
            }
            bits = bits.wrapping_add(unzig64(cols[META].var()?) as u64);
            let len = cols[META].len()?;
            let data = cols[OPAQUE].take(len)?.to_vec();
            packets.push(Packet {
                at: f64::from_bits(bits),
                tab: if flags & FLAG_SECOND != 0 {
                    Tab::Second
                } else {
                    Tab::Main
                },
                permanent: flags & FLAG_PERMANENT != 0,
                data: Rc::from(data),
            });
        }
        let unread: usize = cols.iter().map(|c| c.data.len() - c.pos).sum();
        if unread != 0 {
            return Err(BlockError::Trailing(unread));
        }
        if packets.len() != self.count as usize {
            return Err(BlockError::Count {
                expected: self.count,
                found: packets.len(),
            });
        }
        let span = (packets[0].at, packets[packets.len() - 1].at);
        if span.0.to_bits() != self.first_at.to_bits() || span.1.to_bits() != self.last_at.to_bits()
        {
            return Err(BlockError::Span);
        }
        Ok(packets)
    }
}

pub(super) fn pack_sights(sights: &[Sight]) -> (u32, Vec<u8>) {
    let mut raw = Vec::with_capacity(sights.len() * 12);
    let mut prev = Sight {
        at: 0.0,
        eye_x: 0.0,
        eye_y: 0.0,
        scale: 0.0,
        aim_x: 0.0,
        aim_y: 0.0,
        splitting: false,
        aim_player: 0,
    };
    for s in sights {
        put(
            &mut raw,
            zig64(s.at.to_bits().wrapping_sub(prev.at.to_bits()) as i64),
        );
        for (now, before) in [
            (s.eye_x, prev.eye_x),
            (s.eye_y, prev.eye_y),
            (s.scale, prev.scale),
            (s.aim_x, prev.aim_x),
            (s.aim_y, prev.aim_y),
        ] {
            put(
                &mut raw,
                zig32(now.to_bits().wrapping_sub(before.to_bits()) as i32),
            );
        }
        raw.push(u8::from(s.splitting));
        put(&mut raw, u64::from(s.aim_player));
        prev = *s;
    }
    (raw.len() as u32, compress_to_vec(&raw, LEVEL))
}

pub(super) fn unpack_sights(
    raw_len: u32,
    count: u32,
    bytes: &[u8],
) -> Result<Vec<Sight>, BlockError> {
    if raw_len > MAX_RAW_LEN {
        return Err(BlockError::TooLarge(raw_len));
    }
    let raw =
        decompress_to_vec_with_limit(bytes, raw_len as usize).map_err(|_| BlockError::Inflate)?;
    if raw.len() != raw_len as usize {
        return Err(BlockError::Size {
            expected: raw_len,
            found: raw.len(),
        });
    }
    let mut col = Col { data: &raw, pos: 0 };
    let mut at = 0u64;
    let mut fields = [0u32; 5];
    let mut sights = Vec::with_capacity(count as usize);
    for _ in 0..count {
        at = at.wrapping_add(unzig64(col.var()?) as u64);
        for field in &mut fields {
            *field = field.wrapping_add(unzig32(col.var()?) as u32);
        }
        let splitting = match col.byte()? {
            0 => false,
            1 => true,
            other => return Err(BlockError::Splitting(other)),
        };
        let code = col.var()?;
        let aim_player = u16::try_from(code).map_err(|_| BlockError::AimPlayer(code))?;
        sights.push(Sight {
            at: f64::from_bits(at),
            eye_x: f32::from_bits(fields[0]),
            eye_y: f32::from_bits(fields[1]),
            scale: f32::from_bits(fields[2]),
            aim_x: f32::from_bits(fields[3]),
            aim_y: f32::from_bits(fields[4]),
            splitting,
            aim_player,
        });
    }
    if col.pos != raw.len() {
        return Err(BlockError::Trailing(raw.len() - col.pos));
    }
    Ok(sights)
}
