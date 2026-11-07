use super::ogar::SELF_IDS;
use super::{Cx, RADIUS_SQ_PER_MASS};
pub(super) use crate::net::hud::MapSpot;
use crate::net::hud::{hex_color, UnitRow};
use crate::world::{player_key, Tab, World};

fn mass_of_size(size: f32) -> f64 {
    f64::from(size * size) / RADIUS_SQ_PER_MASS
}

const SECTOR_ROWS: [&str; 5] = ["A", "B", "C", "D", "E"];

impl MapSpot {
    fn sector(self) -> String {
        let last = (SECTOR_ROWS.len() - 1) as f32;
        let at = |v: f32| (v * SECTOR_ROWS.len() as f32).floor().clamp(0.0, last) as usize;
        format!("{}{}", SECTOR_ROWS[at(self.y)], at(self.x) + 1)
    }
}

pub(super) fn publish_map(cx: &mut Cx, world: &World) {
    if !world.border().is_valid() {
        return;
    }
    let width = world.border().max_x - world.border().min_x;
    let mut rows = Vec::with_capacity(SELF_IDS.len());
    for (tab, owner) in [Tab::Main, Tab::Second].into_iter().zip(SELF_IDS) {
        let Some(dot) = world.tab_dot(tab) else {
            continue;
        };
        let spot = MapSpot {
            x: dot.x,
            y: dot.y,
            r: dot.r,
        };
        let row = UnitRow {
            id: player_key(owner),
            name: cx.identity.name.clone(),
            mass: mass_of_size(dot.r * width),
            color: hex_color(dot.color.0, dot.color.1, dot.color.2),
            cell: spot.sector(),
            spot,
        };
        let info = cx.hud.self_info.get_or_insert_with(Default::default);
        let sector = Some(row.cell.clone());
        match tab {
            Tab::Main => info.pos1 = sector,
            Tab::Second => info.pos2 = sector,
        }
        rows.push(row);
    }
    cx.hud.units = Some(rows);
}
