use bytemuck::{Pod, Zeroable};
use ogar_core::world::Circle;

use super::options::Rgba;

const COVERS: usize = 8;
pub(crate) const COVER_VEC4S: usize = COVERS * 3 / 4;
const UNMASKED: f32 = -1.0;

#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub(crate) struct OverlapMark {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub color: Rgba,
    pub covers: [[f32; 3]; COVERS],
}

pub(crate) const OVERLAP_BYTES: u32 = std::mem::size_of::<OverlapMark>() as u32;

pub(crate) fn cover_offset(vec4: usize) -> usize {
    std::mem::offset_of!(OverlapMark, covers) + vec4 * std::mem::size_of::<[f32; 4]>()
}

impl OverlapMark {
    pub(crate) fn is_masked(&self) -> bool {
        self.covers[0][2] != UNMASKED
    }

    pub(crate) fn clip_circles(&self) -> impl Iterator<Item = [f32; 3]> + '_ {
        let contains =
            |[ox, oy, or]: [f32; 3], [x, y, r]: [f32; 3]| (x - ox).hypot(y - oy) + r <= or;
        let whole = [0.0, 0.0, self.r];
        let drawn = self.covers.iter().take_while(|c| c[2] > 0.0).copied();
        let unclipped = !self.is_masked() || drawn.clone().any(|c| contains(c, whole));
        drawn
            .enumerate()
            .filter(move |&(k, c)| {
                !unclipped && !self.covers[..k].iter().any(|&larger| contains(larger, c))
            })
            .map(|(_, c)| c)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Candidate {
    pub id: u32,
    pub owner: u32,
    pub at: Circle,
    pub color: Rgba,
}

#[derive(Default)]
pub(crate) struct Overlaps {
    candidates: Vec<Candidate>,
    order: Vec<usize>,
    hits: Vec<(usize, usize)>,
    marks: Vec<OverlapMark>,
    marked: Vec<u32>,
}

impl Overlaps {
    pub(crate) fn clear(&mut self) {
        self.candidates.clear();
        self.marks.clear();
        self.marked.clear();
    }

    pub(crate) fn push_candidate(&mut self, candidate: Candidate) {
        self.candidates.push(candidate);
    }

    pub(crate) fn marks(&self) -> &[OverlapMark] {
        &self.marks
    }

    pub(crate) fn marked_cells(&self) -> &[u32] {
        &self.marked
    }

    pub(crate) fn resolve_marks(&mut self, mask: bool) {
        let c = &self.candidates;
        self.order.clear();
        self.order.extend(0..c.len());
        let left = |k: usize| c[k].at.x - c[k].at.r;
        self.order
            .sort_unstable_by(|&p, &q| left(p).total_cmp(&left(q)));
        self.hits.clear();
        for (s, &p) in self.order.iter().enumerate() {
            let a = c[p];
            let right = a.at.x + a.at.r;
            for &q in &self.order[s + 1..] {
                if left(q) >= right {
                    break;
                }
                let b = c[q];
                if a.owner == b.owner {
                    continue;
                }
                let sum = a.at.r + b.at.r;
                let (dx, dy) = (a.at.x - b.at.x, a.at.y - b.at.y);
                if dx * dx + dy * dy >= sum * sum {
                    continue;
                }
                let (i, j) = (p.min(q), p.max(q));
                let (small, big) = if c[i].at.r < c[j].at.r {
                    (i, j)
                } else {
                    (j, i)
                };
                self.hits.push((small, big));
            }
        }
        self.hits.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| c[b.1].at.r.total_cmp(&c[a.1].at.r))
        });
        for group in self.hits.chunk_by(|a, b| a.0 == b.0) {
            let small = c[group[0].0];
            let mut covers = [[0.0; 3]; COVERS];
            for (slot, &(_, big)) in covers.iter_mut().zip(group) {
                let b = c[big].at;
                *slot = [b.x - small.at.x, b.y - small.at.y, b.r];
            }
            if !mask {
                covers[0][2] = UNMASKED;
            }
            self.marks.push(OverlapMark {
                x: small.at.x,
                y: small.at.y,
                r: small.at.r,
                color: small.color,
                covers,
            });
            self.marked.push(small.id);
        }
    }
}
