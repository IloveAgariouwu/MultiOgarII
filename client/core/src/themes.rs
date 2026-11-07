use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::settings::SettingsError;

pub const MAX_THEMES: usize = 32;
pub const MAX_NAME_CHARS: usize = 20;

#[derive(Debug, Error, PartialEq)]
pub enum ThemeError {
    #[error("theme data is not valid: {0}")]
    Malformed(String),
    #[error("theme name must be 1-{MAX_NAME_CHARS} characters")]
    NameLength,
    #[error("theme name `{0}` is already used")]
    NameTaken(String),
    #[error("no more than {MAX_THEMES} themes")]
    TooManyThemes,
    #[error("theme `{theme}`: unknown color `{key}` ignored")]
    UnknownColor { theme: String, key: String },
    #[error("theme `{theme}`: `{key}` = {value} is not an ARGB color, kept the built-in color")]
    NotArgb {
        theme: String,
        key: String,
        value: Value,
    },
    #[error("the built-in theme cannot be renamed, deleted or exported")]
    Builtin,
    #[error("no theme named `{0}`")]
    UnknownTheme(String),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlayerTheme {
    pub name: String,
    pub colors: Map<String, Value>,
}

#[derive(Deserialize)]
struct RawTheme {
    name: String,
    colors: Map<String, Value>,
}

#[derive(Deserialize)]
pub(crate) struct StoredBook {
    pub(crate) selected: Option<String>,
    pub(crate) themes: Vec<Value>,
}

#[derive(Serialize)]
pub(crate) struct StoredBookRef<'a> {
    pub(crate) selected: Option<&'a str>,
    pub(crate) themes: &'a [PlayerTheme],
}

pub(crate) fn argb(value: &Value) -> Option<u32> {
    value.as_u64().and_then(|n| u32::try_from(n).ok())
}

pub(crate) fn parse_builtin(
    json: &str,
    defaults: &Map<String, Value>,
) -> Result<Map<String, Value>, SettingsError> {
    let overlay: Map<String, Value> = serde_json::from_str(json)
        .map_err(|e| SettingsError::Schema(format!("built-in theme: {e}")))?;
    let mut colors = defaults.clone();
    for (key, value) in overlay {
        let Some(slot) = colors.get_mut(&key) else {
            return Err(SettingsError::UnknownColor(key));
        };
        let argb = argb(&value).ok_or_else(|| {
            SettingsError::Schema(format!(
                "built-in theme: `{key}` = {value} is not an ARGB color"
            ))
        })?;
        *slot = Value::from(argb);
    }
    Ok(colors)
}

pub(crate) fn check_name(name: &str) -> Result<&str, ThemeError> {
    let name = name.trim();
    let chars = name.chars().count();
    if chars == 0 || chars > MAX_NAME_CHARS {
        return Err(ThemeError::NameLength);
    }
    Ok(name)
}

impl PlayerTheme {
    pub(crate) fn parse(
        value: Value,
        builtin: &Map<String, Value>,
    ) -> Result<(Self, Vec<ThemeError>), ThemeError> {
        let raw: RawTheme =
            serde_json::from_value(value).map_err(|e| ThemeError::Malformed(e.to_string()))?;
        let name = check_name(&raw.name)?.to_owned();
        let mut colors = builtin.clone();
        let mut issues = Vec::new();
        for (key, value) in raw.colors {
            let Some(slot) = colors.get_mut(&key) else {
                issues.push(ThemeError::UnknownColor {
                    theme: name.clone(),
                    key,
                });
                continue;
            };
            match argb(&value) {
                Some(argb) => *slot = Value::from(argb),
                None => issues.push(ThemeError::NotArgb {
                    theme: name.clone(),
                    key,
                    value,
                }),
            }
        }
        Ok((Self { name, colors }, issues))
    }
}
