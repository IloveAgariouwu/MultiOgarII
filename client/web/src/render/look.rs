use ogar_core::world::{Cell, Player, Sym, Tab, World, AUTO_SPLIT_ALERT_MASS};

use super::options::{Options, Rgba};
use super::scene::{rgba, server_mass, team_rgba, Scene, BIG_MASS};

const SPLIT_HUE_READY: f32 = 0.9;
const MARKER_LEVELS: [Rgba; 6] = [
    [0xFF, 0x00, 0x00, 255],
    [0xFF, 0x66, 0x00, 255],
    [0xFF, 0xFF, 0x00, 255],
    [0x00, 0xDD, 0xFF, 255],
    [0x00, 0xAA, 0x00, 255],
    [0x00, 0x00, 0xCC, 255],
];
const MARKER_EXTEND_LOWEST: Rgba = [0x00, 0x00, 0xCC, 255];

pub(super) fn edge_complement(c: Rgba) -> Rgba {
    let (hi, lo) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    if hi - lo < 24 {
        let v = f32::from(c[0]);
        let target = if v < 128.0 { 255.0 } else { 0.0 };
        let g = (v + (target - v) * 0.6).round() as u8;
        return [g, g, g, c[3]];
    }
    let sum = u16::from(hi) + u16::from(lo);
    let flip = |v: u8| (sum - u16::from(v)) as u8;
    [flip(c[0]), flip(c[1]), flip(c[2]), c[3]]
}

#[derive(Debug, Default)]
pub(super) struct Rings {
    pub body: Option<Rgba>,
    pub edge: Option<Rgba>,
    pub slash: bool,
    pub second: Option<Rgba>,
    pub outer: Option<Rgba>,
    pub arrow: bool,
    pub order3: Option<Rgba>,
    pub rainbow: bool,
}

pub(super) fn opaque(c: Rgba) -> Rgba {
    [c[0], c[1], c[2], 255]
}

fn hsv(h: f32) -> Rgba {
    let h = h.rem_euclid(1.0) * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    let c = |v: f32| (v * 255.0).round() as u8;
    [c(r), c(g), c(b), 255]
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SkinLook {
    pub key: Sym,
    pub alpha: f32,
    pub enemy_overlay: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Label {
    pub name: Sym,
    pub color: Rgba,
}

impl Label {
    pub(super) fn text(&self, world: &World) -> String {
        world.string(self.name).to_owned()
    }
}

pub(super) fn name_for(o: &Options, player: Option<&Player>, own: bool) -> Option<Label> {
    let p = player?;
    if !o.show_name || (own && !o.show_self_name) || p.name.is_none() {
        return None;
    }
    Some(Label {
        name: p.name,
        color: team_rgba(p),
    })
}

impl Scene {
    pub(super) fn skin_look(
        &self,
        world: &World,
        o: &Options,
        cell: &Cell,
        p: &Player,
    ) -> SkinLook {
        let unit = (cell.owner & 1) as usize;
        let own_slot = self.own_owner_slot(cell.owner);
        let own = own_slot.is_some();
        let mut key = p.skins[unit];
        if let (true, Some(slot)) = (o.use_external_skin, own_slot) {
            let tab = if slot < 2 { Tab::Main } else { Tab::Second };
            let external = world.external_skin(tab, unit);
            if external != Sym::NONE {
                key = external;
            }
        }
        let playing = !world.own(Tab::Main).is_empty() || !world.own(Tab::Second).is_empty();
        let hint = !own && playing && o.show_enemy_hint;
        let wanted = if own {
            o.show_self_skin
        } else {
            o.show_enemy_skin || hint
        };
        let visible = o.show_skin && wanted && key != Sym::NONE && world.string(p.name) != "dead";
        let alpha = match (visible, hint) {
            (false, _) => 0.0,
            (true, true) => 0.6,
            (true, false) => 1.0,
        };
        SkinLook {
            key,
            alpha,
            enemy_overlay: hint && visible,
        }
    }

    pub(super) fn rings_for(&self, world: &World, o: &Options, cell: &Cell, own: bool) -> Rings {
        let mut rings = Rings::default();
        let active = world.active_owner() == Some(cell.owner);
        let mark = self.marks.mark_of(cell.id);
        let half = o.show_half_split && active && self.marks.is_half_biggest(cell.id);
        let half_color = opaque(o.half_marker);
        let [ring, ring2, ring3, ring4] = o.marker_rings;
        if o.cell_ring {
            rings.edge = Some(if half {
                half_color
            } else {
                edge_complement(rgba(cell))
            });
        } else if own {
            if o.show_split_prediction && active {
                rings.edge = Some(if half {
                    half_color
                } else {
                    hsv(SPLIT_HUE_READY + (0.58 - SPLIT_HUE_READY) * mark.split_weight)
                });
            } else if o.show_order_ring2 && !o.show_order_ring {
                rings.edge = Some(match (o.show_half_split && active, half, active) {
                    (true, true, _) => half_color,
                    (_, _, true) => opaque(ring),
                    _ => opaque(ring2),
                });
            } else if o.show_order_ring && active && !o.show_order_ring2 {
                rings.edge = Some(if half { half_color } else { opaque(ring) });
            } else if half {
                rings.edge = Some(half_color);
            }
            rings.arrow = o.show_unit_arrow && active;
        }
        if half && o.ani_half_split {
            rings.edge = None;
            rings.rainbow = true;
        }
        if o.show_order_ring3 && own {
            rings.order3 = Some(if active { ring3 } else { ring4 });
        }
        if let (true, false, Some(level)) = (o.show_mass_marker, active, mark.size_level) {
            let level = usize::from(level);
            let color = if !o.marker_extend {
                MARKER_LEVELS[level]
            } else if level == o.marker_levels.len() {
                MARKER_EXTEND_LOWEST
            } else {
                opaque(o.marker_levels[level])
            };
            let skinned = o.show_skin
                && if own {
                    o.show_self_skin
                } else {
                    o.show_enemy_skin
                };
            if skinned {
                rings.edge = Some(color);
                rings.slash = o.show_enemy_hint;
            } else {
                rings.body = Some(color);
            }
        }
        let playing = !world.own(Tab::Main).is_empty() || !world.own(Tab::Second).is_empty();
        if o.show_split_indicator && playing && !own && mark.show_mark {
            rings.outer = Some(if mark.can_eat {
                [0x00, 0xFF, 0x00, 255]
            } else {
                [0xAA, 0xAA, 0xAA, 255]
            });
        }
        let mass = server_mass(cell) as f32;
        if o.show_auto_split_alert && mass >= AUTO_SPLIT_ALERT_MASS {
            let mut u = ((self.now / (16.0 * 25.0 / 0.5)) % 1.0) as f32 * 2.0;
            if u > 1.0 {
                u = 2.0 - u;
            }
            let (from, to) = if mass > BIG_MASS {
                (0.0, 0.33)
            } else {
                (0.2875, 0.725)
            };
            rings.second = Some(hsv(from + (to - from) * u));
        }
        rings
    }
}
