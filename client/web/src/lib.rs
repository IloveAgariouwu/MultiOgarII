use ogar_core::colors::Colors;
use ogar_core::macros::{self, Macro, MacroBook, MacroError};
use ogar_core::profiles::{self, Profile, Profiles, StoredProfileKeys};
use ogar_core::settings::{HotkeyKind, Schema, Settings, UI_JSON};
use ogar_core::themes::{self, ThemeError};
use ogar_core::world::Tab;
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

mod net;
mod render;
mod replays;
mod solo;
mod world;

pub use render::Renderer;
pub use world::WorldHandle;

fn js_err(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

fn macro_error_key(e: &MacroError) -> &'static str {
    match e {
        MacroError::DelayOutOfRange(_) => "macroErrDelay",
        MacroError::NoSteps => "macroErrNoSteps",
        MacroError::TooManySteps(_) => "macroErrTooManySteps",
        MacroError::RepeatOutOfRange(_) => "macroErrRepeat",
        MacroError::NameLength => "macroErrNameLength",
        MacroError::NameTaken(_) => "macroErrNameTaken",
        MacroError::TooManyMacros => "macroErrTooManyMacros",
        MacroError::TriggerTaken(_) => "macroErrTriggerTaken",
        MacroError::UnknownAction(_) | MacroError::UnknownIndex(_) | MacroError::Json(_) => {
            "macroErrInvalid"
        }
    }
}

fn theme_error_key(e: &ThemeError) -> &'static str {
    match e {
        ThemeError::NameLength => "themeErrNameLength",
        ThemeError::NameTaken(_) => "themeErrNameTaken",
        ThemeError::TooManyThemes => "themeErrTooMany",
        ThemeError::Builtin => "themeErrBuiltin",
        ThemeError::Malformed(_)
        | ThemeError::UnknownColor { .. }
        | ThemeError::NotArgb { .. }
        | ThemeError::UnknownTheme(_) => "themeErrInvalid",
    }
}

fn hotkey_kind(kind: u8) -> Result<HotkeyKind, JsValue> {
    match kind {
        0 => Ok(HotkeyKind::Control),
        1 => Ok(HotkeyKind::Toggle),
        2 => Ok(HotkeyKind::Hold),
        3 => Ok(HotkeyKind::Mouse),
        k => Err(JsValue::from_str(&format!("unknown hotkey kind {k}"))),
    }
}

fn tab(index: u8) -> Result<Tab, JsValue> {
    match index {
        0 => Ok(Tab::Main),
        1 => Ok(Tab::Second),
        t => Err(JsValue::from_str(&format!("unknown tab {t}"))),
    }
}

#[wasm_bindgen]
pub struct Core {
    schema: Schema,
    settings: Settings,
    colors: Colors,
    profiles: Profiles,
    macros: MacroBook,
    revision: u32,
}

#[wasm_bindgen]
impl Core {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<Core, JsValue> {
        let schema = Schema::bundled().map_err(js_err)?;
        Ok(Core {
            settings: Settings::new(&schema),
            colors: Colors::new(&schema),
            profiles: Profiles::default(),
            macros: MacroBook::default(),
            revision: 0,
            schema,
        })
    }

    #[wasm_bindgen(js_name = loadSettings)]
    pub fn load_settings(&mut self, stored: &str) -> Vec<String> {
        self.revision += 1;
        self.settings
            .load(&self.schema, stored)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[wasm_bindgen(js_name = loadThemes)]
    pub fn load_themes(&mut self, stored: Option<String>) -> Vec<String> {
        self.revision += 1;
        stored.map_or_else(Vec::new, |stored| {
            self.colors
                .load(&stored)
                .iter()
                .map(ToString::to_string)
                .collect()
        })
    }

    #[wasm_bindgen(js_name = loadProfiles)]
    pub fn load_profiles(
        &mut self,
        entries: Option<String>,
        extra: Option<String>,
        selected: Option<String>,
        selected_extra: Option<String>,
        selected_tab2: Option<String>,
    ) -> Vec<String> {
        self.profiles
            .load(StoredProfileKeys {
                entries: entries.as_deref(),
                extra: extra.as_deref(),
                selected: selected.as_deref(),
                selected_extra: selected_extra.as_deref(),
                selected_tab2: selected_tab2.as_deref(),
            })
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[wasm_bindgen(js_name = uiSchema)]
    pub fn ui_schema(&self) -> String {
        UI_JSON.to_owned()
    }

    pub fn defaults(&self) -> String {
        self.schema.defaults_json()
    }

    pub fn groups(&self) -> String {
        self.schema.groups_json()
    }

    pub fn settings(&self) -> String {
        Value::Object(self.settings.values().clone()).to_string()
    }

    pub fn hotkeys(&self) -> String {
        json!({
            "control": self.settings.hotkeys(HotkeyKind::Control),
            "toggle": self.settings.hotkeys(HotkeyKind::Toggle),
            "hold": self.settings.hotkeys(HotkeyKind::Hold),
            "mouse": self.settings.hotkeys(HotkeyKind::Mouse),
        })
        .to_string()
    }

    #[wasm_bindgen(js_name = setSetting)]
    pub fn set_setting(&mut self, key: &str, value_json: &str) -> Result<String, JsValue> {
        let value: Value = serde_json::from_str(value_json).map_err(js_err)?;
        let changed = self
            .settings
            .set(&self.schema, key, value)
            .map_err(js_err)?;
        self.revision += 1;
        Ok(json!(changed).to_string())
    }

    #[wasm_bindgen(js_name = needsReload)]
    pub fn needs_reload(&self, key: &str) -> bool {
        self.schema.needs_reload(key)
    }

    #[wasm_bindgen(js_name = setHotkey)]
    pub fn set_hotkey(&mut self, kind: u8, key: &str, code: i32) -> Result<(), JsValue> {
        let kind = hotkey_kind(kind)?;
        if code >= 0 && kind != HotkeyKind::Mouse {
            self.macros.release_trigger(i64::from(code), None);
        }
        self.settings
            .set_hotkey(kind, key, i64::from(code))
            .map_err(js_err)
    }

    pub fn limits(&self) -> String {
        json!({
            "macroMaxDelayMs": macros::MAX_DELAY_MS,
            "macroMaxRepeat": macros::MAX_REPEAT,
            "macroNameMax": macros::MAX_NAME_CHARS,
            "macroMaxSteps": macros::MAX_STEPS,
            "macroMaxCount": macros::MAX_MACROS,
            "profileNameMax": profiles::NAME_MAX,
            "profileLabelMax": profiles::LABEL_MAX,
            "themeNameMax": themes::MAX_NAME_CHARS,
            "themeMaxCount": themes::MAX_THEMES,
        })
        .to_string()
    }

    #[wasm_bindgen(js_name = loadMacros)]
    pub fn load_macros(&mut self, stored: &str) -> Vec<String> {
        let (book, issues) = MacroBook::load(stored, &self.schema, &self.settings);
        self.macros = book;
        issues.iter().map(ToString::to_string).collect()
    }

    pub fn macros(&self) -> String {
        serde_json::to_string(self.macros.macros()).expect("macros serialize to JSON")
    }

    #[wasm_bindgen(js_name = macroActions)]
    pub fn macro_actions(&self) -> Vec<String> {
        macros::actions(&self.schema)
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    #[wasm_bindgen(js_name = setMacro)]
    pub fn set_macro(&mut self, index: i32, json: &str) -> String {
        let slot = usize::try_from(index).ok();
        let result = Macro::parse(json).and_then(|m| {
            let mut settings = self.settings.clone();
            let mut macros = self.macros.clone();
            if m.trigger >= 0 {
                settings.release_code(HotkeyKind::Control, m.trigger);
                macros.release_trigger(m.trigger, slot);
            }
            macros.set(slot, m, &self.schema, &settings)?;
            self.settings = settings;
            self.macros = macros;
            Ok(())
        });
        result.err().map_or("", |e| macro_error_key(&e)).to_owned()
    }

    #[wasm_bindgen(js_name = removeMacro)]
    pub fn remove_macro(&mut self, index: usize) -> Result<(), JsValue> {
        self.macros.remove(index).map_err(js_err)
    }

    #[wasm_bindgen(js_name = moveMacro)]
    pub fn move_macro(&mut self, from: usize, to: usize) -> String {
        self.macros
            .move_to(from, to)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default()
    }

    #[wasm_bindgen(js_name = storedMacros)]
    pub fn stored_macros(&self) -> String {
        self.macros.to_stored()
    }

    #[wasm_bindgen(js_name = resetKeys)]
    pub fn reset_keys(&mut self, keys: Vec<String>) -> Result<(), JsValue> {
        self.settings
            .reset_keys(&self.schema, &keys)
            .map_err(js_err)?;
        self.revision += 1;
        Ok(())
    }

    #[wasm_bindgen(js_name = resetHotkeys)]
    pub fn reset_hotkeys(&mut self) {
        self.settings.reset_hotkeys(&self.schema);
    }

    #[wasm_bindgen(js_name = resetColors)]
    pub fn reset_colors(&mut self) {
        self.colors.select_builtin();
        self.revision += 1;
    }

    #[wasm_bindgen(js_name = clearMacros)]
    pub fn clear_macros(&mut self) {
        self.macros = MacroBook::default();
    }

    #[wasm_bindgen(js_name = storedSettings)]
    pub fn stored_settings(&self) -> String {
        self.settings.to_stored(&self.schema)
    }

    pub fn colors(&self) -> String {
        Value::Object(self.colors.defs().clone()).to_string()
    }

    pub fn themes(&self) -> String {
        json!({
            "names": self.colors.names().collect::<Vec<_>>(),
            "selected": self.colors.selected(),
        })
        .to_string()
    }

    #[wasm_bindgen(js_name = setColor)]
    pub fn set_color(&mut self, key: &str, argb: u32, fork_name: &str) -> String {
        let result = self.colors.set(key, argb, fork_name);
        self.revision += u32::from(result == Ok(true));
        result.err().map_or("", |e| theme_error_key(&e)).to_owned()
    }

    #[wasm_bindgen(js_name = saveThemeAs)]
    pub fn save_theme_as(&mut self, name: &str) -> String {
        self.theme_op(|c| c.save_as(name))
    }

    #[wasm_bindgen(js_name = selectTheme)]
    pub fn select_theme(&mut self, name: Option<String>) -> String {
        self.theme_op(|c| c.select(name.as_deref()).map(drop))
    }

    #[wasm_bindgen(js_name = renameTheme)]
    pub fn rename_theme(&mut self, name: &str) -> String {
        self.theme_op(|c| c.rename(name))
    }

    #[wasm_bindgen(js_name = deleteTheme)]
    pub fn delete_theme(&mut self) -> String {
        self.theme_op(Colors::delete)
    }

    #[wasm_bindgen(js_name = exportTheme)]
    pub fn export_theme(&self) -> Result<String, JsValue> {
        self.colors.export().map_err(js_err)
    }

    #[wasm_bindgen(js_name = importTheme)]
    pub fn import_theme(&mut self, json: &str) -> String {
        let result = self.colors.import(json);
        self.revision += u32::from(result.is_ok());
        match result {
            Ok(issues) => json!({ "ok": true, "error": "", "issues": issues.iter().map(ToString::to_string).collect::<Vec<_>>() }),
            Err(e) => json!({ "ok": false, "error": theme_error_key(&e), "issues": [e.to_string()] }),
        }
        .to_string()
    }

    #[wasm_bindgen(js_name = storedThemes)]
    pub fn stored_themes(&self) -> String {
        self.colors.to_stored()
    }

    pub fn profiles(&self) -> String {
        let entries: Vec<Value> = self
            .profiles
            .entries()
            .iter()
            .map(|p| self.profile_value(p))
            .collect();
        let mut state = self.selection_value();
        state["entries"] = Value::Array(entries);
        state.to_string()
    }

    pub fn profile(&self, index: usize) -> Result<String, JsValue> {
        let p = self.profiles.get(index).map_err(js_err)?;
        Ok(self.profile_value(p).to_string())
    }

    #[wasm_bindgen(js_name = profileSelection)]
    pub fn profile_selection(&self) -> String {
        self.selection_value().to_string()
    }

    #[wasm_bindgen(js_name = setProfileField)]
    pub fn set_profile_field(
        &mut self,
        index: usize,
        field: &str,
        value: &str,
    ) -> Result<bool, JsValue> {
        self.profiles.set_field(index, field, value).map_err(js_err)
    }

    #[wasm_bindgen(js_name = selectProfile)]
    pub fn select_profile(&mut self, tab: u8, index: usize) -> Result<(), JsValue> {
        self.profiles.select(self::tab(tab)?, index).map_err(js_err)
    }

    #[wasm_bindgen(js_name = setLinked)]
    pub fn set_linked(&mut self, linked: bool) {
        self.profiles.set_linked(linked);
    }

    #[wasm_bindgen(js_name = takeProfileChanges)]
    pub fn take_profile_changes(&mut self) -> String {
        serde_json::to_string(&self.profiles.take_changes())
            .expect("profile changes serialize to JSON")
    }

    #[wasm_bindgen(js_name = storedProfileSelection)]
    pub fn stored_profile_selection(&self) -> String {
        serde_json::to_string(&self.profiles.stored_selection())
            .expect("stored selection serializes to JSON")
    }

    #[wasm_bindgen(js_name = addProfile)]
    pub fn add_profile(&mut self) -> usize {
        self.profiles.add()
    }

    #[wasm_bindgen(js_name = moveProfile)]
    pub fn move_profile(&mut self, from: usize, to: usize) -> Result<(), JsValue> {
        self.profiles.move_to(from, to).map_err(js_err)
    }

    #[wasm_bindgen(js_name = removeProfile)]
    pub fn remove_profile(&mut self, index: usize) -> Result<(), JsValue> {
        self.profiles.remove(index).map_err(js_err)
    }
}

impl Core {
    fn theme_op(&mut self, op: impl FnOnce(&mut Colors) -> Result<(), ThemeError>) -> String {
        let result = op(&mut self.colors);
        self.revision += u32::from(result.is_ok());
        result.err().map_or("", |e| theme_error_key(&e)).to_owned()
    }

    fn profile_value(&self, p: &Profile) -> Value {
        let mut v = serde_json::to_value(p).expect("profile serializes");
        let m = v.as_object_mut().expect("profile serializes to an object");
        m.insert("used".into(), Value::Bool(!self.profiles.is_untouched(p)));
        m.insert("displayName".into(), Value::from(p.display_name()));
        v
    }

    fn selection_value(&self) -> Value {
        json!({
            "main": self.profiles.selected(Tab::Main),
            "second": self.profiles.selected(Tab::Second),
            "linked": self.profiles.linked(),
        })
    }

    pub(crate) fn render_inputs(&self) -> (u32, &Settings, &Colors) {
        (self.revision, &self.settings, &self.colors)
    }
}
