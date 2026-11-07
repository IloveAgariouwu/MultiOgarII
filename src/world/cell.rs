use crate::core::math::{Rect, Vec2};

pub const NO_OWNER: u32 = u32::MAX;
pub const NOT_LISTED: u32 = u32::MAX;
pub const EAT_RATIO: f64 = 1.15;

pub fn size_from_mass(mass: f64) -> f64 {
    (mass * 100.0).sqrt()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Player,
    Food,
    Virus,
    Ejected,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Clone, Debug)]
pub struct Cell {
    pub id: u32,
    pub kind: Kind,
    pub alive: bool,
    pub moving: bool,
    pub is_mother: bool,
    pub no_respawn: bool,
    pub can_remerge: bool,
    pub unit: u8,
    pub owner: u32,
    pub color: Rgb,
    pub pos: Vec2,
    pub radius: f64,
    pub radius2: f64,
    pub mass: f64,
    pub boost_distance: f64,
    pub boost_direction: Vec2,
    pub created_at: u64,
    pub killer: u32,
    pub list_index: u32,
    pub moving_index: u32,
}

impl Cell {
    pub fn set_size(&mut self, size: f64) {
        self.radius = size;
        self.radius2 = size * size;
        self.mass = self.radius2 / 100.0;
    }

    pub fn bound(&self) -> Rect {
        Rect::around(self.pos, self.radius, self.radius)
    }

    pub fn age(&self, ticks: u64) -> u64 {
        ticks - self.created_at
    }

    pub fn has_owner(&self) -> bool {
        self.owner != NO_OWNER
    }
}

#[derive(Default)]
pub struct CellStore {
    pub slots: Vec<Cell>,
    free: Vec<u32>,
    last_id: u32,
}

impl CellStore {
    pub fn alloc(&mut self, kind: Kind, pos: Vec2, radius: f64, created_at: u64) -> u32 {
        self.last_id = self.last_id.checked_add(1).unwrap_or(1);
        let mut cell = Cell {
            id: self.last_id,
            kind,
            alive: true,
            moving: false,
            is_mother: false,
            no_respawn: false,
            can_remerge: false,
            unit: 0,
            owner: NO_OWNER,
            color: Rgb::default(),
            pos,
            radius: 0.0,
            radius2: 0.0,
            mass: 0.0,
            boost_distance: 0.0,
            boost_direction: Vec2::ZERO,
            created_at,
            killer: 0,
            list_index: NOT_LISTED,
            moving_index: NOT_LISTED,
        };
        cell.set_size(radius);
        match self.free.pop() {
            Some(slot) => {
                self.slots[slot as usize] = cell;
                slot
            }
            None => {
                self.slots.push(cell);
                self.slots.len() as u32 - 1
            }
        }
    }

    pub fn reclaim(&mut self, slot: u32) {
        debug_assert!(!self.slots[slot as usize].alive);
        self.free.push(slot);
    }

    pub fn clear(&mut self) {
        self.slots.clear();
        self.free.clear();
        self.last_id = 0;
    }

}

impl std::ops::Index<u32> for CellStore {
    type Output = Cell;
    fn index(&self, slot: u32) -> &Cell {
        &self.slots[slot as usize]
    }
}

impl std::ops::IndexMut<u32> for CellStore {
    fn index_mut(&mut self, slot: u32) -> &mut Cell {
        &mut self.slots[slot as usize]
    }
}
