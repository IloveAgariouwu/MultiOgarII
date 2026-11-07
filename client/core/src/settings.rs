use std::collections::{BTreeMap, HashMap};

use serde::Deserialize;
use serde_json::{Map, Value};
use thiserror::Error;

use crate::themes::parse_builtin;

pub const EXTRACT_JSON: &str = include_str!("../../data/schema-extract.json");
pub const UI_JSON: &str = include_str!("../../data/settings-ui.json");
pub const THEMES_JSON: &str = include_str!("../../data/themes.json");
const QUALITY_PRESET: &str = "QualityPreset";
const NO_HOTKEY: i64 = -1;

pub type HotkeyMap = BTreeMap<String, i64>;

#[derive(Debug, Error, PartialEq)]
pub enum SettingsError {
    #[error("schema is not valid JSON: {0}")]
    Schema(String),
    #[error("unknown setting `{0}`")]
    UnknownKey(String),
    #[error("unknown hotkey `{0}`")]
    UnknownHotkey(String),
    #[error("unknown color `{0}`")]
    UnknownColor(String),
    #[error("rejected value: {0}")]
    InvalidValue(StoredError),
}

#[derive(Debug, Error, PartialEq)]
pub enum StoredError {
    #[error("stored data is not valid JSON: {0}")]
    Malformed(String),
    #[error("stored data is not a JSON object")]
    NotObject,
    #[error("`{key}` expects {expected}, found {found}; kept the default")]
    WrongType {
        key: String,
        expected: &'static str,
        found: Value,
    },
    #[error("`{key}` = {value} is outside {min}..={max}; clamped")]
    OutOfRange {
        key: String,
        value: f64,
        min: f64,
        max: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Range {
    min: f64,
    max: f64,
    integer: bool,
}

#[derive(Debug, Deserialize)]
struct Category {
    items: Vec<UiItem>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum UiItem {
    Key(String),
    Widget(Widget),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Widget {
    Range {
        key: String,
        min: f64,
        max: f64,
        step: f64,
    },
    Group {
        key: Option<String>,
        #[serde(default)]
        keys: Vec<String>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Extract {
    defaults: Map<String, Value>,
    hotkey_defaults: HotkeyMap,
    color_defaults: Map<String, Value>,
    groups: Map<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UiSchema {
    categories: Vec<Category>,
    ui_actions: Vec<String>,
    quality_presets: Vec<Map<String, Value>>,
    reload_required: Vec<String>,
    added_defaults: Map<String, Value>,
}

#[derive(Debug, Clone)]
pub struct Schema {
    defaults: Map<String, Value>,
    hotkey_defaults: HotkeyMap,
    option_keys: Vec<String>,
    groups: Map<String, Value>,
    quality_presets: Vec<Map<String, Value>>,
    reload_required: Vec<String>,
    ranges: HashMap<String, Range>,
    ui_actions: Vec<String>,
    builtin_colors: Map<String, Value>,
}

impl Schema {
    pub fn parse(
        extract_json: &str,
        ui_json: &str,
        themes_json: &str,
    ) -> Result<Self, SettingsError> {
        let extract: Extract =
            serde_json::from_str(extract_json).map_err(|e| SettingsError::Schema(e.to_string()))?;
        let ui: UiSchema =
            serde_json::from_str(ui_json).map_err(|e| SettingsError::Schema(e.to_string()))?;
        let mut defaults = extract.defaults;
        defaults.extend(ui.added_defaults);
        let items = || ui.categories.iter().flat_map(|c| &c.items);
        let option_keys: Vec<String> = items()
            .flat_map(|item| match item {
                UiItem::Key(key) => vec![key],
                UiItem::Widget(Widget::Group { key, keys }) => key.iter().chain(keys).collect(),
                UiItem::Widget(Widget::Range { .. } | Widget::Other) => Vec::new(),
            })
            .cloned()
            .collect();
        if let Some(unknown) = option_keys
            .iter()
            .find(|k| !defaults.contains_key(k.as_str()))
        {
            return Err(SettingsError::UnknownKey(unknown.clone()));
        }
        let ranges: HashMap<String, Range> = items()
            .filter_map(|item| match item {
                UiItem::Widget(Widget::Range {
                    key,
                    min,
                    max,
                    step,
                }) => Some((
                    key.clone(),
                    Range {
                        min: *min,
                        max: *max,
                        integer: step.fract() == 0.0 && min.fract() == 0.0,
                    },
                )),
                _ => None,
            })
            .collect();
        let control = extract
            .groups
            .get("control")
            .and_then(Value::as_array)
            .ok_or_else(|| SettingsError::Schema("groups.control is missing".to_owned()))?;
        if let Some(unknown) = ui
            .ui_actions
            .iter()
            .find(|a| !control.iter().any(|c| c.as_str() == Some(a.as_str())))
        {
            return Err(SettingsError::UnknownHotkey(unknown.clone()));
        }
        if let Some(unknown) = ui
            .quality_presets
            .iter()
            .flat_map(Map::keys)
            .find(|k| !defaults.contains_key(k.as_str()))
        {
            return Err(SettingsError::Schema(format!(
                "quality preset sets unknown key `{unknown}`"
            )));
        }
        let builtin_colors = parse_builtin(themes_json, &extract.color_defaults)?;
        Ok(Self {
            defaults,
            hotkey_defaults: extract.hotkey_defaults,
            option_keys,
            groups: extract.groups,
            quality_presets: ui.quality_presets,
            reload_required: ui.reload_required,
            ranges,
            ui_actions: ui.ui_actions,
            builtin_colors,
        })
    }

    pub fn bundled() -> Result<Self, SettingsError> {
        Self::parse(EXTRACT_JSON, UI_JSON, THEMES_JSON)
    }

    pub fn groups_json(&self) -> String {
        Value::Object(self.groups.clone()).to_string()
    }

    pub fn defaults_json(&self) -> String {
        Value::Object(self.defaults.clone()).to_string()
    }

    pub(crate) fn group(&self, name: &str) -> Vec<&str> {
        self.groups
            .get(name)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect()
    }

    pub(crate) fn builtin_colors(&self) -> &Map<String, Value> {
        &self.builtin_colors
    }

    pub(crate) fn ui_actions(&self) -> &[String] {
        &self.ui_actions
    }

    pub fn needs_reload(&self, key: &str) -> bool {
        self.reload_required.iter().any(|k| k == key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyKind {
    Control,
    Toggle,
    Hold,
    Mouse,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    values: Map<String, Value>,
    control_hotkeys: HotkeyMap,
    toggle_hotkeys: HotkeyMap,
    hold_hotkeys: HotkeyMap,
    mouse_hotkeys: HotkeyMap,
}

impl Settings {
    pub fn new(schema: &Schema) -> Self {
        let unbound = |k: &String| (k.clone(), NO_HOTKEY);
        let mut s = Self {
            values: schema.defaults.clone(),
            control_hotkeys: schema.hotkey_defaults.clone(),
            toggle_hotkeys: schema.option_keys.iter().map(unbound).collect(),
            hold_hotkeys: schema.option_keys.iter().map(unbound).collect(),
            mouse_hotkeys: schema.hotkey_defaults.keys().map(unbound).collect(),
        };
        s.sync_quality_preset(schema);
        s
    }

    pub fn load(&mut self, schema: &Schema, stored: &str) -> Vec<StoredError> {
        let saved = match parse_object(stored) {
            Ok(saved) => saved,
            Err(e) => return vec![e],
        };
        let mut issues = Vec::new();
        let mut stored = Vec::new();
        for (k, v) in saved {
            match k.as_str() {
                "controlHotKeys" => stored.extend(
                    copy_hotkeys(&mut self.control_hotkeys, &k, v, &mut issues)
                        .into_iter()
                        .map(|key| (HotkeyKind::Control, key)),
                ),
                "toggleHotKeys" => stored.extend(
                    copy_hotkeys(&mut self.toggle_hotkeys, &k, v, &mut issues)
                        .into_iter()
                        .map(|key| (HotkeyKind::Toggle, key)),
                ),
                "holdHotKeys" => stored.extend(
                    copy_hotkeys(&mut self.hold_hotkeys, &k, v, &mut issues)
                        .into_iter()
                        .map(|key| (HotkeyKind::Hold, key)),
                ),
                "mouseHotKeys" => stored.extend(
                    copy_hotkeys(&mut self.mouse_hotkeys, &k, v, &mut issues)
                        .into_iter()
                        .map(|key| (HotkeyKind::Mouse, key)),
                ),
                _ => {
                    let Some(slot) = self.values.get_mut(&k) else {
                        continue;
                    };
                    if let Some(value) = normalize(&k, slot, v, schema.ranges.get(&k), &mut issues)
                    {
                        *slot = value;
                    }
                }
            }
        }
        self.release_default_collisions(&stored);
        self.sync_quality_preset(schema);
        issues
    }

    pub fn to_stored(&self, schema: &Schema) -> String {
        let mut out: Map<String, Value> = self
            .values
            .iter()
            .filter(|(k, v)| schema.defaults.get(*k) != Some(v))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (group, map) in [
            ("controlHotKeys", &self.control_hotkeys),
            ("toggleHotKeys", &self.toggle_hotkeys),
            ("holdHotKeys", &self.hold_hotkeys),
            ("mouseHotKeys", &self.mouse_hotkeys),
        ] {
            let codes = map.iter().map(|(k, &code)| (k.clone(), Value::from(code)));
            out.insert(group.into(), Value::Object(codes.collect()));
        }
        Value::Object(out).to_string()
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    pub fn values(&self) -> &Map<String, Value> {
        &self.values
    }

    pub fn set(
        &mut self,
        schema: &Schema,
        key: &str,
        value: Value,
    ) -> Result<Vec<String>, SettingsError> {
        let Some(current) = self.values.get(key) else {
            return Err(SettingsError::UnknownKey(key.to_owned()));
        };
        let mut issues = Vec::new();
        let Some(value) = normalize(key, current, value, schema.ranges.get(key), &mut issues)
        else {
            let reason = issues
                .pop()
                .expect("normalize reports why it rejects a value");
            return Err(SettingsError::InvalidValue(reason));
        };
        let mut changed = Vec::new();
        if key == QUALITY_PRESET {
            let level = value.as_u64();
            if let Some(preset) = level.and_then(|l| schema.quality_presets.get(l as usize)) {
                for (k, v) in preset {
                    self.assign(k, v.clone(), &mut changed);
                }
            }
            self.assign(key, value, &mut changed);
            return Ok(changed);
        }
        self.assign(key, value, &mut changed);
        if schema.quality_presets.iter().any(|p| p.contains_key(key)) {
            let before = self.values.get(QUALITY_PRESET).cloned();
            self.sync_quality_preset(schema);
            if self.values.get(QUALITY_PRESET) != before.as_ref() {
                changed.push(QUALITY_PRESET.to_owned());
            }
        }
        Ok(changed)
    }

    pub fn reset_keys(&mut self, schema: &Schema, keys: &[String]) -> Result<(), SettingsError> {
        for key in keys {
            let default = schema
                .defaults
                .get(key)
                .ok_or_else(|| SettingsError::UnknownKey(key.clone()))?;
            self.values.insert(key.clone(), default.clone());
        }
        self.sync_quality_preset(schema);
        Ok(())
    }

    pub fn reset_hotkeys(&mut self, schema: &Schema) {
        let fresh = Self::new(schema);
        self.control_hotkeys = fresh.control_hotkeys;
        self.toggle_hotkeys = fresh.toggle_hotkeys;
        self.hold_hotkeys = fresh.hold_hotkeys;
        self.mouse_hotkeys = fresh.mouse_hotkeys;
    }

    pub fn hotkey(&self, kind: HotkeyKind, key: &str) -> Option<i64> {
        self.hotkeys(kind).get(key).copied()
    }

    pub fn hotkeys(&self, kind: HotkeyKind) -> &HotkeyMap {
        match kind {
            HotkeyKind::Control => &self.control_hotkeys,
            HotkeyKind::Toggle => &self.toggle_hotkeys,
            HotkeyKind::Hold => &self.hold_hotkeys,
            HotkeyKind::Mouse => &self.mouse_hotkeys,
        }
    }

    fn hotkeys_mut(&mut self, kind: HotkeyKind) -> &mut HotkeyMap {
        match kind {
            HotkeyKind::Control => &mut self.control_hotkeys,
            HotkeyKind::Toggle => &mut self.toggle_hotkeys,
            HotkeyKind::Hold => &mut self.hold_hotkeys,
            HotkeyKind::Mouse => &mut self.mouse_hotkeys,
        }
    }

    fn conflicting(kind: HotkeyKind) -> &'static [HotkeyKind] {
        match kind {
            HotkeyKind::Control => &[HotkeyKind::Control, HotkeyKind::Toggle, HotkeyKind::Hold],
            HotkeyKind::Toggle => &[HotkeyKind::Control, HotkeyKind::Toggle],
            HotkeyKind::Hold => &[HotkeyKind::Control, HotkeyKind::Hold],
            HotkeyKind::Mouse => &[HotkeyKind::Mouse],
        }
    }

    pub(crate) fn hotkey_available(&self, kind: HotkeyKind, code: i64) -> bool {
        !Self::conflicting(kind)
            .iter()
            .any(|&k| self.hotkeys(k).values().any(|&v| v == code))
    }

    pub fn release_code(&mut self, kind: HotkeyKind, code: i64) {
        for &k in Self::conflicting(kind) {
            for v in self.hotkeys_mut(k).values_mut().filter(|v| **v == code) {
                *v = NO_HOTKEY;
            }
        }
    }

    pub fn set_hotkey(
        &mut self,
        kind: HotkeyKind,
        key: &str,
        code: i64,
    ) -> Result<(), SettingsError> {
        if !self.hotkeys(kind).contains_key(key) {
            return Err(SettingsError::UnknownHotkey(key.to_owned()));
        }
        if code >= 0 {
            self.release_code(kind, code);
            let twin = match kind {
                HotkeyKind::Toggle => Some(HotkeyKind::Hold),
                HotkeyKind::Hold => Some(HotkeyKind::Toggle),
                HotkeyKind::Control | HotkeyKind::Mouse => None,
            };
            if let Some(v) = twin
                .and_then(|k| self.hotkeys_mut(k).get_mut(key))
                .filter(|v| **v == code)
            {
                *v = NO_HOTKEY;
            }
        }
        self.hotkeys_mut(kind).insert(key.to_owned(), code);
        Ok(())
    }

    fn release_default_collisions(&mut self, stored: &[(HotkeyKind, String)]) {
        let kinds = [
            HotkeyKind::Control,
            HotkeyKind::Toggle,
            HotkeyKind::Hold,
            HotkeyKind::Mouse,
        ];
        for kind in kinds {
            let taken: Vec<i64> = stored
                .iter()
                .filter(|(k, _)| Self::conflicting(kind).contains(k))
                .filter_map(|(k, key)| self.hotkey(*k, key))
                .filter(|&code| code >= 0)
                .collect();
            let defaulted: Vec<String> = self
                .hotkeys(kind)
                .iter()
                .filter(|(key, v)| {
                    !stored.iter().any(|(k, s)| *k == kind && s == *key) && taken.contains(v)
                })
                .map(|(key, _)| key.clone())
                .collect();
            for key in defaulted {
                self.hotkeys_mut(kind).insert(key, NO_HOTKEY);
            }
        }
    }

    fn assign(&mut self, key: &str, value: Value, changed: &mut Vec<String>) {
        let slot = self.values.get_mut(key).expect("known setting");
        if *slot != value {
            *slot = value;
            changed.push(key.to_owned());
        }
    }

    fn sync_quality_preset(&mut self, schema: &Schema) {
        let level = schema
            .quality_presets
            .iter()
            .position(|p| p.iter().all(|(k, v)| self.values.get(k) == Some(v)))
            .unwrap_or(schema.quality_presets.len());
        self.values
            .insert(QUALITY_PRESET.into(), Value::from(level as u64));
    }
}

fn parse_object(stored: &str) -> Result<Map<String, Value>, StoredError> {
    match serde_json::from_str::<Value>(stored) {
        Ok(Value::Object(saved)) => Ok(saved),
        Ok(_) => Err(StoredError::NotObject),
        Err(e) => Err(StoredError::Malformed(e.to_string())),
    }
}

fn number_value(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9.0e15 {
        Value::from(n as i64)
    } else {
        Value::from(n)
    }
}

fn normalize(
    key: &str,
    default: &Value,
    stored: Value,
    range: Option<&Range>,
    issues: &mut Vec<StoredError>,
) -> Option<Value> {
    let mut reject = |expected: &'static str, found: Value| {
        issues.push(StoredError::WrongType {
            key: key.to_owned(),
            expected,
            found,
        });
        None
    };
    match default {
        Value::Bool(_) => match stored {
            Value::Bool(_) => Some(stored),
            Value::String(ref s) if s == "true" || s == "false" => Some(Value::Bool(s == "true")),
            other => reject("a boolean", other),
        },
        Value::Number(_) => {
            let number = match &stored {
                Value::Number(n) => n.as_f64(),
                Value::String(s) => s.trim().parse::<f64>().ok().filter(|n| n.is_finite()),
                _ => None,
            };
            let Some(n) = number else {
                return reject("a number", stored);
            };
            let Some(r) = range else {
                return Some(number_value(n));
            };
            if r.integer && n.fract() != 0.0 {
                return reject("an integer", stored);
            }
            if n < r.min || n > r.max {
                issues.push(StoredError::OutOfRange {
                    key: key.to_owned(),
                    value: n,
                    min: r.min,
                    max: r.max,
                });
            }
            Some(number_value(n.clamp(r.min, r.max)))
        }
        Value::String(_) => match stored {
            Value::String(_) => Some(stored),
            other => reject("a string", other),
        },
        other => unreachable!("setting `{key}` has a non-scalar default {other}"),
    }
}

fn copy_hotkeys(
    target: &mut HotkeyMap,
    group: &str,
    source: Value,
    issues: &mut Vec<StoredError>,
) -> Vec<String> {
    let Value::Object(src) = source else {
        issues.push(StoredError::WrongType {
            key: group.to_owned(),
            expected: "an object",
            found: source,
        });
        return Vec::new();
    };
    let mut copied = Vec::new();
    for (k, v) in src {
        let Some(slot) = target.get_mut(&k) else {
            continue;
        };
        match v.as_i64() {
            Some(code) if code >= NO_HOTKEY => {
                *slot = code;
                copied.push(k);
            }
            _ => issues.push(StoredError::WrongType {
                key: format!("{group}.{k}"),
                expected: "a key code",
                found: v,
            }),
        }
    }
    copied
}
