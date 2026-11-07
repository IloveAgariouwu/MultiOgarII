use ogar_core::world::{Circle, World};

use super::camera::Camera;
use super::options::{with_alpha, Reader, Rgba};
use super::primitive::{Edge, FrameEdge, Outline, Prim, Rect};
use super::rainbow::{Rainbow, Stops};
use super::scene::{SECTORS, SECTOR_COUNT};
use super::text::{MapText, Slot, TextAtlas, TIERS};
use super::{ScreenClip, FRAME_MS};
use crate::world::{map_dots, parse_map_dots, MapDot};

const PERCENT: f32 = 100.0;
const COORD_SIZE: f32 = 0.09;
const NAME_FONT_PX: f32 = 13.0;
const NAME_LIFT: f32 = 0.8;
const ENEMY_ALPHA: f32 = 0.375;
const SELF_MIN_RADIUS: f32 = 4.0;
const SELF_RING_RADIUS: f32 = 1.32;
const SELF_RING_WIDTH: f32 = 0.16;
const GLIDE_KEEP: f32 = 0.99;
const GLOW_DEPTH: f32 = 0.08;
const GLOW_EDGE_ALPHA: f32 = 0x59 as f32 / 255.0;
const NAME_COLOR_ALPHA: u8 = 0xc7;
const GRID_ALPHA: f32 = 0.35;
const VIEW_ALPHA: f32 = 0.6;
const PLAYER_STRIDE: usize = 5;
const DOT_MIN_RADIUS: f32 = 2.5;
const DOT_SCALE: f32 = 0.75;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MinimapOptions {
    glowing_border: bool,
    ani_border: bool,
    show_coord: bool,
    border_size: f32,
    opaque: bool,
    name_zoom: f32,
    draw_cells: bool,
    show_name: bool,
    show_self_name: bool,
    name_team_color: bool,
    show_crosshair: bool,
    show_view: bool,
    anim_interval: f32,
    ui_font: String,
    theme_text: Rgba,
    border: Rgba,
    coords: Rgba,
    rainbow_stops: Stops,
    foreground: Rgba,
    background: Rgba,
}

impl MinimapOptions {
    pub(super) fn read(r: &Reader) -> Self {
        Self {
            glowing_border: r.flag("MiniMapGlowingBorder"),
            ani_border: r.flag("MiniMapAniBorder"),
            show_coord: r.flag("MiniMapShowCoord"),
            border_size: r.num("MiniMapBorderSize"),
            opaque: r.flag("MiniMapOpaque"),
            name_zoom: r.num("MiniMapNameZoom") / PERCENT,
            draw_cells: r.index("MiniMapDrawMode") == 1,
            show_name: r.flag("ShowMapName"),
            show_self_name: r.flag("MiniMapShowSelfName"),
            name_team_color: r.flag("MiniMapNameTeamColor"),
            show_crosshair: r.flag("MiniMapShowCrosshair"),
            show_view: r.flag("MiniMapShowView"),
            anim_interval: r.num("MiniMapAnimInterval"),
            ui_font: r.text("UiFont"),
            theme_text: r.color("clThemeText"),
            border: r.color("clMiniMapBorder"),
            coords: r.color("clMiniMapCoords"),
            rainbow_stops: [
                [
                    "clMiniMapBorderColor1",
                    "clMiniMapBorderColor2",
                    "clMiniMapBorderColor3",
                    "clMiniMapBorderColor4",
                ],
                [
                    "clMiniMapBorderColor5",
                    "clMiniMapBorderColor6",
                    "clMiniMapBorderColor7",
                    "clMiniMapBorderColor8",
                ],
            ]
            .map(|keys| keys.map(|k| r.color(k))),
            foreground: r.color("clGameForeground"),
            background: r.color("clMapBackground"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Place {
    pub x: f32,
    pub y: f32,
    pub side: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Style {
    pub text: f32,
    pub shrink: f32,
    pub watching: bool,
    pub live_view: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct View {
    ex: f32,
    ey: f32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

impl View {
    pub(crate) fn from_slice(v: &[f32]) -> Option<Self> {
        let &[ex, ey, x0, y0, x1, y1] = v else {
            return None;
        };
        Some(Self {
            ex,
            ey,
            x0,
            y0,
            x1,
            y1,
        })
    }

    fn from_camera(world: &World, cam: &Camera) -> Option<Self> {
        let b = world.border();
        if !b.is_valid() {
            return None;
        }
        let (w, h) = (b.max_x - b.min_x, b.max_y - b.min_y);
        let (hw, hh) = cam.half_view;
        Some(Self {
            ex: (cam.x - b.min_x) / w,
            ey: (cam.y - b.min_y) / h,
            x0: (cam.x - hw - b.min_x) / w,
            y0: (cam.y - hh - b.min_y) / h,
            x1: (cam.x + hw - b.min_x) / w,
            y1: (cam.y + hh - b.min_y) / h,
        })
    }
}

#[derive(Debug, Clone)]
struct Player {
    id: f64,
    x: f32,
    y: f32,
    r: f32,
    color: Rgba,
    /// The name as drawn, made once when the player data arrives.
    label: String,
}

#[derive(Debug, Clone, Copy)]
struct Track {
    x: f32,
    y: f32,
    r: f32,
}

struct Label {
    player: usize,
    x: f32,
    y: f32,
    color: Rgba,
}

fn rgb(packed: f64) -> Rgba {
    let [_, r, g, b] = (packed as u32).to_be_bytes();
    [r, g, b, 255]
}

fn disc(x: f32, y: f32, r: f32, fill: Rgba, edge: Option<Edge>) -> Prim {
    Prim::Disc {
        at: Circle { x, y, r },
        outline: Outline::Round,
        fill,
        edge,
    }
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32, color: Rgba) -> Prim {
    Prim::Rect {
        rect: Rect::spanning(x0, y0, x1, y1),
        color,
    }
}

fn glyph(x: f32, y: f32, slot: Slot, px: f32, tint: Rgba) -> Prim {
    let k = px / slot.px;
    Prim::Glyph {
        rect: Rect {
            x,
            y,
            half_w: slot.w * k * 0.5,
            half_h: slot.h * k * 0.5,
        },
        uv: slot.uv,
        page: slot.page,
        tint,
    }
}

#[derive(Default)]
pub(crate) struct Minimap {
    place: Option<Place>,
    style: Option<Style>,
    family: String,
    view: Option<View>,
    players: Vec<Player>,
    tracks: Vec<Track>,
    cells: Option<Vec<f64>>,
    dots: Vec<MapDot>,
    labels: Vec<Label>,
    rainbow: Rainbow,
    last: Option<f64>,
    pub base: Vec<Prim>,
    pub clipped: Vec<Prim>,
    pub clip: Option<ScreenClip>,
}

impl Minimap {
    pub(crate) fn set_place(&mut self, place: Option<Place>) {
        self.place = place;
    }

    pub(crate) fn set_style(&mut self, style: Style, family: String) {
        self.style = Some(style);
        self.family = family;
    }

    pub(crate) fn font_family<'a>(&'a self, o: &'a MinimapOptions) -> Option<&'a str> {
        self.style?;
        self.place?;
        Some(if self.family.is_empty() {
            &o.ui_font
        } else {
            &self.family
        })
    }

    pub(crate) fn set_view(&mut self, view: Option<View>) {
        self.view = view;
    }

    pub(crate) fn set_cells(&mut self, cells: Option<Vec<f64>>) {
        self.cells = cells;
    }

    pub(crate) fn set_players(&mut self, nums: &[f64], names: Vec<String>) {
        debug_assert_eq!(
            names.len(),
            nums.len() / PLAYER_STRIDE,
            "one name per player"
        );
        let players: Vec<Player> = nums
            .chunks_exact(PLAYER_STRIDE)
            .zip(names)
            .map(|(p, name)| Player {
                id: p[0],
                x: p[1] as f32,
                y: p[2] as f32,
                r: p[3] as f32,
                color: rgb(p[4]),
                label: name.trim().to_owned(),
            })
            .collect();
        let tracks = players
            .iter()
            .map(|p| {
                self.players
                    .iter()
                    .zip(&self.tracks)
                    .find(|(old, _)| old.id == p.id)
                    .map_or(
                        Track {
                            x: p.x,
                            y: p.y,
                            r: p.r,
                        },
                        |(_, t)| *t,
                    )
            })
            .collect();
        self.players = players;
        self.tracks = tracks;
    }

    fn glide_players(&mut self, now: f64) {
        let dt = self.last.map_or(0.0, |last| now - last);
        self.last = Some(now);
        let keep = GLIDE_KEEP.powf((dt / FRAME_MS) as f32);
        for (t, p) in self.tracks.iter_mut().zip(&self.players) {
            t.x = p.x + (t.x - p.x) * keep;
            t.y = p.y + (t.y - p.y) * keep;
            t.r = p.r + (t.r - p.r) * keep;
        }
    }

    pub(crate) fn build(
        &mut self,
        world: &World,
        o: &MinimapOptions,
        cam: &Camera,
        css_ratio: f32,
        now: f64,
        atlas: &mut TextAtlas,
    ) {
        self.base.clear();
        self.clipped.clear();
        self.clip = None;
        self.glide_players(now);
        self.rainbow.advance(now, o.anim_interval, &o.rainbow_stops);
        let (Some(place), Some(style)) = (self.place, self.style) else {
            return;
        };
        let at = Place {
            x: (place.x * css_ratio).round(),
            y: (place.y * css_ratio).round(),
            side: (place.side * css_ratio).round(),
        };
        let Place { x: ox, y: oy, side } = at;
        if side <= 0.0 {
            return;
        }
        let k = side / place.side;
        // Only drawn cells need the dots; skip scanning the world otherwise.
        match &self.cells {
            _ if !o.draw_cells => self.dots.clear(),
            Some(cells) => parse_map_dots(cells, &mut self.dots),
            None => map_dots(world, &mut self.dots),
        }
        let bw = (2.0 * o.border_size / PERCENT).max(1.0) * k;
        self.build_base(o, &style, at, k, bw, atlas);
        let (cx0, cy0) = ((ox + bw).round(), (oy + bw).round());
        let (cx1, cy1) = ((ox + side - bw).round(), (oy + side - bw).round());
        if cx1 <= cx0 || cy1 <= cy0 {
            return;
        }
        self.clip = Some(ScreenClip {
            x: cx0.max(0.0) as u32,
            y: cy0.max(0.0) as u32,
            w: (cx1 - cx0.max(0.0)) as u32,
            h: (cy1 - cy0.max(0.0)) as u32,
        });
        let view = if style.live_view {
            self.view.or_else(|| View::from_camera(world, cam))
        } else {
            None
        };
        self.build_dots(o, &style, at, k, atlas);
        if let Some(v) = view.filter(|_| o.show_crosshair || o.show_view) {
            self.build_view(o, v, ox, oy, side, k);
        }
    }

    fn build_base(
        &mut self,
        o: &MinimapOptions,
        style: &Style,
        at: Place,
        k: f32,
        bw: f32,
        atlas: &mut TextAtlas,
    ) {
        let Place { x: ox, y: oy, side } = at;
        let step = side / SECTOR_COUNT as f32;
        let mut base = std::mem::take(&mut self.base);
        base.push(rect(ox, oy, ox + side, oy + side, o.background));
        if o.glowing_border && !o.ani_border {
            let depth = side * GLOW_DEPTH;
            let [r, g, b, _] = o.border;
            let mut i = 0.0;
            while i < depth {
                let a = GLOW_EDGE_ALPHA * (1.0 - (i + 0.5) / depth);
                let c = [r, g, b, (a * 255.0).round() as u8];
                let w = 1.0_f32.min(depth - i);
                base.extend([
                    rect(ox + i, oy, ox + i + w, oy + side, c),
                    rect(ox + side - i - w, oy, ox + side - i, oy + side, c),
                    rect(ox, oy + i, ox + side, oy + i + w, c),
                    rect(ox, oy + side - i - w, ox + side, oy + side - i, c),
                ]);
                i += 1.0;
            }
        }
        if o.show_coord {
            let line = k.round().max(1.0);
            let color = with_alpha(o.coords, GRID_ALPHA);
            let at = |i: usize| (i as f32 * step).round() + (line % 2.0) / 2.0 - line / 2.0;
            for i in 1..SECTOR_COUNT {
                let p = at(i);
                base.push(rect(ox + p, oy, ox + p + line, oy + side, color));
                let mut from = 0.0;
                for j in 1..=SECTOR_COUNT {
                    let to = if j < SECTOR_COUNT { at(j) } else { side };
                    base.push(rect(ox + from, oy + p, ox + to, oy + p + line, color));
                    from = to + line;
                }
            }
            let px = side * COORD_SIZE * style.text;
            let tier = TextAtlas::tier_for(px, TIERS.len() - 1);
            for (i, text) in SECTORS.iter().enumerate() {
                let (col, row) = ((i % SECTOR_COUNT) as f32, (i / SECTOR_COUNT) as f32);
                if let Some(slot) = atlas.map_text(text, MapText::Coord, tier) {
                    base.push(glyph(
                        ox + (col + 0.5) * step,
                        oy + (row + 0.5) * step,
                        slot,
                        px,
                        o.coords,
                    ));
                }
            }
        }
        let frame = Rect::spanning(ox + bw, oy + bw, ox + side - bw, oy + side - bw);
        if o.ani_border {
            let alpha = o.border[3];
            let [from, to] = self.rainbow.colors().map(|c| [c[0], c[1], c[2], alpha]);
            base.push(Prim::Frame {
                rect: frame,
                band: bw,
                color: from,
                edge: FrameEdge::Rainbow {
                    to,
                    sigma: 0.0,
                    reach: 0.0,
                },
            });
        } else {
            base.push(Prim::Frame {
                rect: frame,
                band: bw,
                color: o.border,
                edge: FrameEdge::Solid,
            });
        }
        self.base = base;
    }

    fn build_dots(
        &mut self,
        o: &MinimapOptions,
        style: &Style,
        at: Place,
        k: f32,
        atlas: &mut TextAtlas,
    ) {
        let Place { x: ox, y: oy, side } = at;
        let solid = o.opaque || style.watching;
        let dot_alpha = |mine: bool| if solid || mine { 1.0 } else { ENEMY_ALPHA };
        let mut out = std::mem::take(&mut self.clipped);
        if o.draw_cells {
            for d in &self.dots {
                let [r, g, b] = d.rgb;
                out.push(disc(
                    ox + d.x * side,
                    oy + d.y * side,
                    (DOT_MIN_RADIUS * k).max(d.r * side),
                    with_alpha([r, g, b, 255], dot_alpha(d.own)),
                    None,
                ));
            }
        }
        let mut labels = std::mem::take(&mut self.labels);
        labels.clear();
        for (i, (p, t)) in self.players.iter().zip(&self.tracks).enumerate() {
            let (x, y) = (ox + t.x * side, oy + t.y * side);
            let r = (SELF_MIN_RADIUS * k).max(t.r * side * DOT_SCALE);
            let seen = o.draw_cells && self.dots.iter().any(|d| d.owner == p.id);
            if !seen {
                out.push(disc(x, y, r, p.color, None));
                let outer = r * (SELF_RING_RADIUS + SELF_RING_WIDTH / 2.0);
                out.push(Prim::Ring {
                    at: Circle { x, y, r: outer },
                    outline: Outline::Round,
                    edge: Edge {
                        width: r * SELF_RING_WIDTH / outer,
                        color: p.color,
                    },
                });
            }
            if o.show_name && o.show_self_name {
                labels.push(Label {
                    player: i,
                    x,
                    y: y - r * NAME_LIFT,
                    color: name_color(o, p.color),
                });
            }
        }
        let name_px = NAME_FONT_PX * style.text * style.shrink * o.name_zoom * k;
        let name_tier = TextAtlas::tier_for(name_px, TIERS.len() - 1);
        for l in &labels {
            let text = &self.players[l.player].label;
            if let Some(slot) = atlas.map_text(text, MapText::Name, name_tier) {
                out.push(glyph(l.x, l.y - name_px / 2.0, slot, name_px, l.color));
            }
        }
        self.labels = labels;
        self.clipped = out;
    }

    fn build_view(&mut self, o: &MinimapOptions, v: View, ox: f32, oy: f32, side: f32, k: f32) {
        let line = k.round().max(1.0);
        let half = (line % 2.0) / 2.0;
        let at = |f: f32| (f * side - half).round() + half - line / 2.0;
        let color = with_alpha(o.foreground, VIEW_ALPHA);
        let out = &mut self.clipped;
        if o.show_crosshair {
            let (x, y) = (at(v.ex), at(v.ey));
            out.push(rect(ox + x, oy, ox + x + line, oy + side, color));
            out.push(rect(ox, oy + y, ox + x, oy + y + line, color));
            out.push(rect(ox + x + line, oy + y, ox + side, oy + y + line, color));
        }
        if o.show_view {
            let (x0, y0, x1, y1) = (at(v.x0), at(v.y0), at(v.x1), at(v.y1));
            out.extend([
                rect(ox + x0, oy + y0, ox + x1 + line, oy + y0 + line, color),
                rect(ox + x0, oy + y1, ox + x1 + line, oy + y1 + line, color),
                rect(ox + x0, oy + y0 + line, ox + x0 + line, oy + y1, color),
                rect(ox + x1, oy + y0 + line, ox + x1 + line, oy + y1, color),
            ]);
        }
    }
}

fn name_color(o: &MinimapOptions, team: Rgba) -> Rgba {
    if o.name_team_color {
        team
    } else {
        [
            o.theme_text[0],
            o.theme_text[1],
            o.theme_text[2],
            NAME_COLOR_ALPHA,
        ]
    }
}
