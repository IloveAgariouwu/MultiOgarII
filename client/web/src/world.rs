use ogar_core::world::preview::{Focus, PreviewScene, SelfIdentity};
use ogar_core::world::sample::identities;
use ogar_core::world::{player_key, Kind, Tab, World};
use wasm_bindgen::prelude::*;

const MINIMAP_X: usize = 0;
const MINIMAP_Y: usize = 1;
const MINIMAP_RADIUS: usize = 2;
const MINIMAP_RGB: usize = 3;
const MINIMAP_FLAGS: usize = 4;
const MINIMAP_OWNER: usize = 5;
const MINIMAP_STRIDE: usize = 6;
const MINIMAP_NO_OWNER: f64 = -1.0;
const MINIMAP_OWN: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MapDot {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub rgb: [u8; 3],
    pub own: bool,
    pub owner: f64,
}

impl MapDot {
    fn flags(&self) -> u32 {
        if self.own {
            MINIMAP_OWN
        } else {
            0
        }
    }
}

fn packed_rgb([r, g, b]: [u8; 3]) -> u32 {
    (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
}

pub(crate) fn map_dots(world: &World, out: &mut Vec<MapDot>) {
    out.clear();
    let b = world.border();
    if !b.is_valid() {
        return;
    }
    let (w, h) = (b.max_x - b.min_x, b.max_y - b.min_y);
    let own_owners: Vec<u32> = [Tab::Main, Tab::Second]
        .into_iter()
        .flat_map(|tab| world.own(tab))
        .filter_map(|&id| world.cell(id))
        .map(|c| c.owner)
        .collect();
    for c in world.cells().iter().filter(|c| c.kind == Kind::Player) {
        let own = own_owners.contains(&c.owner);
        out.push(MapDot {
            x: (c.to.x - b.min_x) / w,
            y: (c.to.y - b.min_y) / h,
            r: c.to.r / w,
            rgb: [c.color.0, c.color.1, c.color.2],
            own,
            owner: if c.owner == 0 {
                MINIMAP_NO_OWNER
            } else {
                f64::from(player_key(c.owner))
            },
        });
    }
}

pub(crate) fn parse_map_dots(cells: &[f64], out: &mut Vec<MapDot>) {
    out.clear();
    out.extend(cells.chunks_exact(MINIMAP_STRIDE).map(|e| {
        let flags = e[MINIMAP_FLAGS] as u32;
        let [_, r, g, b] = (e[MINIMAP_RGB] as u32).to_be_bytes();
        MapDot {
            x: e[MINIMAP_X] as f32,
            y: e[MINIMAP_Y] as f32,
            r: e[MINIMAP_RADIUS] as f32,
            rgb: [r, g, b],
            own: flags & MINIMAP_OWN != 0,
            owner: e[MINIMAP_OWNER],
        }
    }));
}

#[wasm_bindgen(js_name = World)]
pub struct WorldHandle {
    pub(crate) inner: World,
}

#[wasm_bindgen(js_class = World)]
impl WorldHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WorldHandle {
        WorldHandle {
            inner: World::new(),
        }
    }

    pub fn border(&self) -> Vec<f32> {
        let b = self.inner.border();
        if !b.is_valid() {
            return Vec::new();
        }
        vec![b.min_x, b.min_y, b.max_x, b.max_y]
    }

    #[wasm_bindgen(js_name = minimapLayout)]
    pub fn minimap_layout() -> String {
        serde_json::json!({
            "stride": MINIMAP_STRIDE,
            "x": MINIMAP_X,
            "y": MINIMAP_Y,
            "radius": MINIMAP_RADIUS,
            "rgb": MINIMAP_RGB,
            "flags": MINIMAP_FLAGS,
            "owner": MINIMAP_OWNER,
            "own": MINIMAP_OWN,
        })
        .to_string()
    }

    #[wasm_bindgen(js_name = minimapCells)]
    pub fn minimap_cells(&self) -> Vec<f64> {
        let mut dots = Vec::new();
        map_dots(&self.inner, &mut dots);
        let mut out = Vec::with_capacity(dots.len() * MINIMAP_STRIDE);
        for d in &dots {
            let mut entry = [0.0; MINIMAP_STRIDE];
            entry[MINIMAP_X] = f64::from(d.x);
            entry[MINIMAP_Y] = f64::from(d.y);
            entry[MINIMAP_RADIUS] = f64::from(d.r);
            entry[MINIMAP_RGB] = f64::from(packed_rgb(d.rgb));
            entry[MINIMAP_FLAGS] = f64::from(d.flags());
            entry[MINIMAP_OWNER] = d.owner;
            out.extend(entry);
        }
        out
    }
}

impl Default for WorldHandle {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
pub struct PreviewWorld {
    scene: PreviewScene,
}

#[wasm_bindgen]
impl PreviewWorld {
    #[wasm_bindgen(constructor)]
    pub fn new(world: &mut WorldHandle, seed: f64) -> PreviewWorld {
        PreviewWorld {
            scene: PreviewScene::create(&mut world.inner, seed as u64),
        }
    }

    pub fn identities(seed: f64, count: usize) -> String {
        serde_json::to_string(&identities(seed as u64, count)).expect("identities serialize")
    }

    #[wasm_bindgen(js_name = setSelfIdentity)]
    pub fn set_self_identity(
        &self,
        world: &mut WorldHandle,
        name: &str,
        skin: &str,
        skin2: &str,
        external: &str,
        external2: &str,
    ) {
        self.scene.set_self_identity(
            &mut world.inner,
            &SelfIdentity {
                name,
                skins: [skin, skin2],
                external: [external, external2],
            },
        );
    }

    #[wasm_bindgen(js_name = updateFrame)]
    pub fn update_frame(&mut self, world: &mut WorldHandle, now: f64) {
        self.scene.update_frame(&mut world.inner, now);
    }

    #[wasm_bindgen(js_name = focusCamera)]
    pub fn focus_camera(&mut self, focus: &str, now: f64) -> Result<Vec<f32>, JsValue> {
        let focus = match focus {
            "scene" => Focus::Scene,
            "virus" => Focus::Virus,
            "autosplit" => Focus::AutoSplit,
            "border" => Focus::Border,
            "jelly" => Focus::Jelly,
            "marker" => Focus::Marker,
            "grid" => Focus::Grid,
            "field" => Focus::Field,
            "second" => Focus::Second,
            other => return Err(JsValue::from_str(&format!("unknown preview focus {other}"))),
        };
        let shot = self.scene.focus_camera(focus, now);
        let (ax, ay) = self.scene.aim_point_for(shot);
        Ok(vec![shot.x, shot.y, shot.span, ax, ay])
    }
}
