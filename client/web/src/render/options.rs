use std::collections::HashMap;

use ogar_core::colors::Colors;
use ogar_core::settings::Settings;
use serde_json::Value;

use super::minimap::MinimapOptions;
use super::rainbow::Stops;

const RENDER_SCALES: [f32; 4] = [0.5, 0.75, 1.0, 1.5];
const SKIN_SIDES: [u32; 3] = [128, 256, 512];
const CARD_SIZES: [f32; 3] = [200.0, 400.0, 800.0];

pub(crate) type Rgba = [u8; 4];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Interpolation {
    Fast,
    Linear,
    Preceding,
    Chase,
    Timed,
    Softened,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverlapMarker {
    Off,
    Auto,
    On,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CameraMode {
    Default,
    UnitCenter,
    CellCenter,
}

pub(crate) fn argb_to_rgba(argb: u32) -> Rgba {
    let [a, r, g, b] = argb.to_be_bytes();
    [r, g, b, a]
}

pub(crate) fn with_alpha(c: Rgba, alpha: f32) -> Rgba {
    [c[0], c[1], c[2], (f32::from(c[3]) * alpha).round() as u8]
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Options {
    pub render_scale: f32,
    pub skin_side: u32,
    pub smooth_skin: bool,
    pub card_size: f32,
    pub text_max_tier: usize,
    pub background: Rgba,
    pub border: Rgba,
    pub border_width: f32,
    pub glowing_border: bool,
    pub glowing_cells: bool,
    pub cell_shadow: bool,
    pub cell_shadow_alpha: f32,
    pub jelly_cells: bool,
    pub ani_border: bool,
    pub ani_virus: bool,
    pub anim_interval: f32,
    pub rainbow_stops: Stops,
    pub show_coord: bool,
    pub field_coords: Rgba,
    pub show_food: bool,
    pub show_pellet: bool,
    pub pellet_alpha: f32,
    pub single_pellet: Option<Rgba>,
    pub self_colors: Option<[Rgba; 4]>,
    pub invisible_self: bool,
    pub cell_alpha: f32,
    pub overlap_marker: OverlapMarker,
    pub overlap_marker_mask: bool,
    pub mass_overlap_size: f32,
    pub cell_ring: bool,
    pub marker_thickness: f32,
    pub marker_alpha: f32,
    pub simple_virus: bool,
    pub virus_range_hint: bool,
    pub show_back_image: bool,
    pub back_image_uri: String,
    pub cell_font: String,
    pub minimap: MinimapOptions,
    pub back_image_alpha: f32,
    pub virus_range_color: Rgba,
    pub virus_fill: Rgba,
    pub virus_stroke: Rgba,
    pub blue_virus_fill: Rgba,
    pub blue_virus_stroke: Rgba,
    pub mother_fill: Rgba,
    pub mother_stroke: Rgba,
    pub show_skin: bool,
    pub show_self_skin: bool,
    pub show_enemy_skin: bool,
    pub show_enemy_hint: bool,
    pub use_external_skin: bool,
    pub show_name: bool,
    pub show_self_name: bool,
    pub show_circular_name: bool,
    pub show_mass: bool,
    pub simplified_mass: bool,
    pub name_zoom: f32,
    pub mass_zoom: f32,
    pub min_text_px: f32,
    pub label_alpha: f32,
    pub zoom_speed: f32,
    pub camera_mode: CameraMode,
    pub camera_movement_speed: f32,
    pub unit_camera_speed: f32,
    pub cell_camera_speed: f32,
    pub interpolation: Interpolation,
    pub interpolation_speed: f32,
    pub timed_animation: f32,
    pub soften_animation: f32,
    pub rendering_time_offset: f32,
    pub virus_split_hint: bool,
    pub show_cursor_line: bool,
    pub cursor_line_thickness: f32,
    pub cursor_line: Rgba,
    pub use_spawn_signal: bool,
    pub show_spec_aim_cursor: bool,
    pub show_split_prediction: bool,
    pub show_half_split: bool,
    pub ani_half_split: bool,
    pub show_order_ring: bool,
    pub show_order_ring2: bool,
    pub show_order_ring3: bool,
    pub show_mass_marker: bool,
    pub marker_extend: bool,
    pub marker_light: bool,
    pub show_split_indicator: bool,
    pub show_auto_split_alert: bool,
    pub show_unit_arrow: bool,
    pub show_eat_limit_marker: bool,
    pub show_cell_direction_marker: bool,
    pub marker_rings: [Rgba; 4],
    pub half_marker: Rgba,
    pub marker_levels: [Rgba; 8],
}

pub(crate) type Overrides = HashMap<String, Value>;

pub(super) struct Reader<'a> {
    settings: &'a Settings,
    colors: &'a Colors,
    overrides: &'a Overrides,
}

impl Reader<'_> {
    fn value(&self, key: &str) -> &Value {
        self.overrides
            .get(key)
            .or_else(|| self.settings.get(key))
            .unwrap_or_else(|| panic!("setting {key} is not in the schema"))
    }

    pub(super) fn flag(&self, key: &str) -> bool {
        let v = self.value(key);
        v.as_bool()
            .unwrap_or_else(|| panic!("setting {key} = {v} is not a boolean"))
    }

    pub(super) fn num(&self, key: &str) -> f32 {
        let v = self.value(key);
        v.as_f64()
            .unwrap_or_else(|| panic!("setting {key} = {v} is not a number")) as f32
    }

    pub(super) fn text(&self, key: &str) -> String {
        let v = self.value(key);
        v.as_str()
            .unwrap_or_else(|| panic!("setting {key} = {v} is not a string"))
            .to_owned()
    }

    pub(super) fn index(&self, key: &str) -> usize {
        self.num(key).round().max(0.0) as usize
    }

    pub(super) fn color(&self, key: &str) -> Rgba {
        if let Some(argb) = self.overrides.get(key) {
            let argb = argb
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or_else(|| panic!("color override {key} = {argb} is not an ARGB color"));
            return argb_to_rgba(argb);
        }
        argb_to_rgba(
            self.colors
                .get(key)
                .unwrap_or_else(|| panic!("color {key} is not in the schema")),
        )
    }
}

impl Options {
    pub(crate) fn read(settings: &Settings, colors: &Colors, overrides: &Overrides) -> Self {
        let r = Reader {
            settings,
            colors,
            overrides,
        };
        Self {
            render_scale: RENDER_SCALES[r.index("RenderScale").min(RENDER_SCALES.len() - 1)],
            skin_side: SKIN_SIDES[r.index("RenderQuality").min(SKIN_SIDES.len() - 1)],
            smooth_skin: r.flag("SmoothSkin"),
            card_size: CARD_SIZES[r.index("RenderQuality").min(CARD_SIZES.len() - 1)],
            text_max_tier: r.index("RenderQuality").min(CARD_SIZES.len() - 1) + 1,
            background: r.color("clGameBackground"),
            border: r.color("clFieldBorder"),
            border_width: r.num("fieldBorderSize") / 100.0,
            glowing_border: r.flag("GlowingBorder"),
            glowing_cells: r.flag("GlowingCells"),
            cell_shadow: r.flag("CellShadow"),
            cell_shadow_alpha: r.num("CellShadowIntensity") / 100.0,
            jelly_cells: r.flag("JellyCells"),
            ani_border: r.flag("AniBorder"),
            ani_virus: r.flag("AniVirus"),
            anim_interval: r.num("animInterval"),
            rainbow_stops: [
                [
                    "clBorderColor1",
                    "clBorderColor2",
                    "clBorderColor3",
                    "clBorderColor4",
                ],
                [
                    "clBorderColor5",
                    "clBorderColor6",
                    "clBorderColor7",
                    "clBorderColor8",
                ],
            ]
            .map(|keys| keys.map(|k| r.color(k))),
            show_coord: r.flag("ShowCoord"),
            field_coords: r.color("clFieldCoords"),
            show_food: r.flag("ShowFood"),
            show_pellet: r.flag("ShowPellet"),
            pellet_alpha: r.num("PelletCellsAlpha"),
            single_pellet: r.flag("SinglePelletsColor").then(|| r.color("clPellet")),
            self_colors: r.flag("ShowClSelfCell").then(|| {
                [
                    "clSelfColor",
                    "clSelfColor2",
                    "clSelfColor3",
                    "clSelfColor4",
                ]
                .map(|k| r.color(k))
            }),
            invisible_self: r.flag("invisibleSelfCell"),
            cell_alpha: if r.flag("TogglePlayerTransparentCells") {
                r.num("PlayerCellsAlpha")
            } else {
                1.0
            },
            cell_ring: r.flag("ShowCellRing"),
            marker_thickness: r.num("MarkerThickness"),
            overlap_marker: match r.index("OverlapMarker") {
                0 => OverlapMarker::Off,
                1 => OverlapMarker::Auto,
                2 => OverlapMarker::On,
                other => unreachable!("OverlapMarker {other} is outside the schema range"),
            },
            overlap_marker_mask: r.flag("OverlapMarkerMask"),
            mass_overlap_size: r.num("massOverlapSize"),
            marker_alpha: r.num("MarkerAlpha"),
            simple_virus: r.flag("SimpleVirus"),
            virus_range_hint: r.flag("VirusRangeHint"),
            show_back_image: r.flag("ShowfieldBackImage"),
            back_image_uri: r.text("fieldBackImageUri"),
            cell_font: r.text("CellFont"),
            minimap: MinimapOptions::read(&r),
            back_image_alpha: r.num("fieldBackImageAlpha"),
            virus_range_color: r.color("clVirusRangeHint"),
            virus_fill: r.color("clVirusInnerFill"),
            virus_stroke: r.color("clVirusOuterStroke"),
            blue_virus_fill: r.color("clBlueVirusInnerFill"),
            blue_virus_stroke: r.color("clBlueVirusOuterStroke"),
            mother_fill: r.color("clMotherCellInnerFill"),
            mother_stroke: r.color("clMotherCellOuterStroke"),
            show_skin: r.flag("ShowSkin"),
            show_self_skin: r.flag("ShowSelfSkin"),
            show_enemy_skin: r.flag("ShowEnemySkin"),
            show_enemy_hint: r.flag("ShowEnemyHint"),
            use_external_skin: r.flag("UseExternalSkin"),
            show_name: r.flag("ShowName"),
            show_self_name: r.flag("ShowSelfName"),
            show_circular_name: r.flag("ShowCircularName"),
            show_mass: r.flag("ShowMass"),
            simplified_mass: r.flag("SimplifiedMass"),
            name_zoom: r.num("nameTextZoom") / 100.0,
            mass_zoom: r.num("massTextZoom") / 100.0,
            min_text_px: r.num("MinTextSize"),
            label_alpha: r.num("PlayerLabelsAlpha"),
            zoom_speed: r.num("CameraZoomSpeed") / 100.0,
            camera_mode: match r.index("CameraMode") {
                0 => CameraMode::Default,
                1 => CameraMode::UnitCenter,
                2 => CameraMode::CellCenter,
                other => unreachable!("CameraMode {other} is outside the schema range"),
            },
            camera_movement_speed: r.num("CameraMovementSpeed"),
            unit_camera_speed: r.num("UnitCameraSpeed"),
            cell_camera_speed: r.num("CellCameraSpeed"),
            interpolation: match r.index("InterpolationType") {
                0 => Interpolation::Fast,
                1 => Interpolation::Linear,
                2 => Interpolation::Preceding,
                3 => Interpolation::Chase,
                4 => Interpolation::Timed,
                5 => Interpolation::Softened,
                other => unreachable!("InterpolationType {other} is outside the schema range"),
            },
            interpolation_speed: r.num("InterpolationSpeed"),
            timed_animation: r.num("TimedAnimation"),
            soften_animation: r.num("SoftenAnimation"),
            rendering_time_offset: r.num("RenderingTimeOffset"),
            virus_split_hint: r.flag("VirusSplitHint"),
            show_cursor_line: r.flag("ShowCursorLine"),
            cursor_line_thickness: r.num("CursorLineThickness"),
            cursor_line: r.color("clCursorLine"),
            use_spawn_signal: true,
            show_spec_aim_cursor: true,
            show_split_prediction: r.flag("ShowSplitPrediction"),
            show_half_split: r.flag("ShowHalfSplit"),
            ani_half_split: r.flag("AniHalfSplit"),
            show_order_ring: r.flag("ShowOrderRing"),
            show_order_ring2: r.flag("ShowOrderRing2"),
            show_order_ring3: r.flag("ShowOrderRing3"),
            show_mass_marker: r.flag("ShowMassMarker"),
            marker_extend: r.flag("MarkerExtend"),
            marker_light: r.flag("MarkerLight"),
            show_split_indicator: r.flag("ShowSplitIndicator"),
            show_auto_split_alert: r.flag("ShowAutoSplitAlert"),
            show_unit_arrow: r.flag("ShowUnitArrow"),
            show_eat_limit_marker: r.flag("ShowEatLimitMarker"),
            show_cell_direction_marker: r.flag("ShowCellDirectionMarker"),
            marker_rings: [
                "clMarkerRing",
                "clMarkerRing2",
                "clMarkerRing3",
                "clMarkerRing4",
            ]
            .map(|k| r.color(k)),
            half_marker: r.color("clHalfMarker"),
            marker_levels: [
                "clMarkerA",
                "clMarkerB",
                "clMarkerC",
                "clMarkerD",
                "clMarkerE",
                "clMarkerF",
                "clMarkerG",
                "clMarkerH",
            ]
            .map(|k| r.color(k)),
        }
    }

    pub(crate) fn world_only(self) -> Self {
        Self {
            overlap_marker: OverlapMarker::Off,
            cell_ring: false,
            virus_range_hint: false,
            show_enemy_hint: false,
            use_external_skin: false,
            virus_split_hint: false,
            show_cursor_line: false,
            use_spawn_signal: false,
            show_spec_aim_cursor: false,
            show_split_prediction: false,
            show_half_split: false,
            show_order_ring: false,
            show_order_ring2: false,
            show_order_ring3: false,
            show_mass_marker: false,
            show_split_indicator: false,
            show_auto_split_alert: false,
            show_unit_arrow: false,
            show_eat_limit_marker: false,
            show_cell_direction_marker: false,
            ..self
        }
    }
}
