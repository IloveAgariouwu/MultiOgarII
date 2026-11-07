use crate::core::math::Rect;

const NOWHERE: u32 = u32::MAX;
const OVERSIZED: u8 = u8::MAX;
const MAX_BUCKETS_PER_LEVEL: u64 = 1 << 20;

#[derive(Clone, Copy)]
struct Entry {
    key: u32,
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

impl Entry {
    fn new(key: u32, x: f64, y: f64, r: f64) -> Self {
        Self {
            key,
            min_x: (x - r) as f32,
            min_y: (y - r) as f32,
            max_x: (x + r) as f32,
            max_y: (y + r) as f32,
        }
    }
}

#[derive(Clone, Copy)]
struct QueryBox {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

impl QueryBox {
    fn hits(&self, e: &Entry) -> bool {
        self.min_x < e.max_x && self.max_x > e.min_x && self.min_y < e.max_y && self.max_y > e.min_y
    }
}

#[derive(Clone, Copy)]
struct Location {
    level: u8,
    bucket: u32,
    index: u32,
}

const UNPLACED: Location = Location { level: 0, bucket: NOWHERE, index: NOWHERE };

struct Level {
    bucket_size: f64,
    inv_bucket_size: f64,
    cols: u32,
    rows: u32,
    words_per_row: usize,
    buckets: Vec<Vec<Entry>>,
    occupied: Vec<u64>,
}

impl Level {
    fn new(bucket_size: f64, width: f64, height: f64) -> Self {
        let cols = (width / bucket_size).ceil() as u32;
        let rows = (height / bucket_size).ceil() as u32;
        let words_per_row = (cols as usize).div_ceil(64);
        Self {
            bucket_size,
            inv_bucket_size: 1.0 / bucket_size,
            cols,
            rows,
            words_per_row,
            buckets: vec![Vec::new(); cols as usize * rows as usize],
            occupied: vec![0; words_per_row * rows as usize],
        }
    }

    fn bucket_index(&self, v: f64) -> u32 {
        (v * self.inv_bucket_size) as u32
    }

    fn bucket_of(&self, dx: f64, dy: f64) -> u32 {
        let col = self.bucket_index(dx).min(self.cols - 1);
        let row = self.bucket_index(dy).min(self.rows - 1);
        row * self.cols + col
    }

    fn set_occupied(&mut self, bucket: u32, on: bool) {
        let row = (bucket / self.cols) as usize;
        let col = (bucket % self.cols) as usize;
        let word = row * self.words_per_row + col / 64;
        let bit = 1u64 << (col % 64);
        if on {
            self.occupied[word] |= bit;
        } else {
            self.occupied[word] &= !bit;
        }
    }
}

pub struct DynamicGrid {
    origin_x: f64,
    origin_y: f64,
    width: f64,
    height: f64,
    levels: Vec<Level>,
    oversized: Vec<Entry>,
    locations: Vec<Location>,
    len: usize,
}

impl DynamicGrid {
    pub fn new(world: Rect, min_radius: f64, max_radius: f64) -> Self {
        let (width, height) = (world.width().max(1.0), world.height().max(1.0));
        let mut size = ((min_radius * 2.0).max(1.0).ceil() as u64).next_power_of_two() as f64;
        while (width / size).ceil() as u64 * (height / size).ceil() as u64 > MAX_BUCKETS_PER_LEVEL {
            size *= 2.0;
        }
        let largest = max_radius * 2.0;
        let mut levels = vec![Level::new(size, width, height)];
        while size < largest && size < width.max(height) {
            size *= 4.0;
            levels.push(Level::new(size, width, height));
        }
        Self {
            origin_x: world.min_x,
            origin_y: world.min_y,
            width,
            height,
            levels,
            oversized: Vec::new(),
            locations: Vec::new(),
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn level_for(&self, x: f64, y: f64, radius: f64) -> u8 {
        let (dx, dy) = (x - self.origin_x, y - self.origin_y);
        if dx < 0.0 || dy < 0.0 || dx >= self.width || dy >= self.height {
            return OVERSIZED;
        }
        let diameter = radius * 2.0;
        for (i, level) in self.levels.iter().enumerate() {
            if level.bucket_size >= diameter {
                return i as u8;
            }
        }
        OVERSIZED
    }

    pub fn insert(&mut self, key: u32, x: f64, y: f64, radius: f64) {
        let idx = key as usize;
        if idx >= self.locations.len() {
            self.locations.resize(idx + 1, UNPLACED);
        }
        debug_assert!(self.locations[idx].index == NOWHERE, "double insert of {key}");
        self.place(key, x, y, radius);
        self.len += 1;
    }

    fn place(&mut self, key: u32, x: f64, y: f64, radius: f64) {
        let entry = Entry::new(key, x, y, radius);
        let level = self.level_for(x, y, radius);
        let location = if level == OVERSIZED {
            self.oversized.push(entry);
            Location { level, bucket: NOWHERE, index: self.oversized.len() as u32 - 1 }
        } else {
            let (dx, dy) = (x - self.origin_x, y - self.origin_y);
            let lvl = &mut self.levels[level as usize];
            let bucket = lvl.bucket_of(dx, dy);
            let items = &mut lvl.buckets[bucket as usize];
            items.push(entry);
            let index = items.len() as u32 - 1;
            if index == 0 {
                lvl.set_occupied(bucket, true);
            }
            Location { level, bucket, index }
        };
        self.locations[key as usize] = location;
    }

    fn unplace(&mut self, location: Location) {
        let moved = if location.level == OVERSIZED {
            self.oversized.swap_remove(location.index as usize);
            self.oversized.get(location.index as usize).map(|e| e.key)
        } else {
            let lvl = &mut self.levels[location.level as usize];
            let items = &mut lvl.buckets[location.bucket as usize];
            items.swap_remove(location.index as usize);
            let moved = items.get(location.index as usize).map(|e| e.key);
            if items.is_empty() {
                lvl.set_occupied(location.bucket, false);
            }
            moved
        };
        if let Some(key) = moved {
            self.locations[key as usize].index = location.index;
        }
    }

    pub fn update(&mut self, key: u32, x: f64, y: f64, radius: f64) {
        let location = self.locations[key as usize];
        debug_assert!(location.index != NOWHERE, "update of absent key {key}");
        let level = self.level_for(x, y, radius);
        if level == location.level {
            if level == OVERSIZED {
                self.oversized[location.index as usize] = Entry::new(key, x, y, radius);
                return;
            }
            let (dx, dy) = (x - self.origin_x, y - self.origin_y);
            let lvl = &mut self.levels[level as usize];
            if lvl.bucket_of(dx, dy) == location.bucket {
                lvl.buckets[location.bucket as usize][location.index as usize] =
                    Entry::new(key, x, y, radius);
                return;
            }
        }
        self.unplace(location);
        self.place(key, x, y, radius);
    }

    pub fn remove(&mut self, key: u32) {
        let location = self.locations[key as usize];
        debug_assert!(location.index != NOWHERE, "remove of absent key {key}");
        self.unplace(location);
        self.locations[key as usize] = UNPLACED;
        self.len -= 1;
    }

    pub fn find(&self, area: Rect, mut visit: impl FnMut(u32) -> bool) -> bool {
        let query = QueryBox {
            min_x: area.min_x as f32,
            min_y: area.min_y as f32,
            max_x: area.max_x as f32,
            max_y: area.max_y as f32,
        };
        for e in &self.oversized {
            if query.hits(e) && visit(e.key) {
                return true;
            }
        }
        for level in &self.levels {
            let reach = level.bucket_size * 0.5;
            let right = area.max_x + reach - self.origin_x;
            let bottom = area.max_y + reach - self.origin_y;
            if right < 0.0 || bottom < 0.0 {
                continue;
            }
            let first_col = level.bucket_index(area.min_x - reach - self.origin_x);
            let first_row = level.bucket_index(area.min_y - reach - self.origin_y);
            if first_col >= level.cols || first_row >= level.rows {
                continue;
            }
            let last_col = level.bucket_index(right).min(level.cols - 1);
            let last_row = level.bucket_index(bottom).min(level.rows - 1);

            let (first_word, last_word) = ((first_col / 64) as usize, (last_col / 64) as usize);
            let first_mask = !0u64 << (first_col % 64);
            let last_mask = !0u64 >> (63 - last_col % 64);
            for row in first_row..=last_row {
                let row_base = row as usize * level.words_per_row;
                for w in first_word..=last_word {
                    let mut bits = level.occupied[row_base + w];
                    if w == first_word {
                        bits &= first_mask;
                    }
                    if w == last_word {
                        bits &= last_mask;
                    }
                    while bits != 0 {
                        let col = w as u32 * 64 + bits.trailing_zeros();
                        bits &= bits - 1;
                        let bucket = (row * level.cols + col) as usize;
                        for e in &level.buckets[bucket] {
                            if query.hits(e) && visit(e.key) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    pub fn query_into(&self, area: Rect, out: &mut Vec<u32>) {
        self.find(area, |key| {
            out.push(key);
            false
        });
    }
}
