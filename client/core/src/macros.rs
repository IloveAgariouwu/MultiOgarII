use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::settings::{HotkeyKind, Schema, Settings};

pub const MAX_MACROS: usize = 32;
pub const MAX_STEPS: usize = 64;
pub const MAX_DELAY_MS: u32 = 5000;
pub const MAX_REPEAT: u8 = 20;
pub const MAX_NAME_CHARS: usize = 20;
const NO_TRIGGER: i64 = -1;
const CONTROL_GROUP: &str = "control";

#[derive(Debug, Error, PartialEq)]
pub enum MacroError {
    #[error("unknown action `{0}`")]
    UnknownAction(String),
    #[error("delay {0} ms is over {MAX_DELAY_MS} ms")]
    DelayOutOfRange(u32),
    #[error("a macro needs at least one step")]
    NoSteps,
    #[error("{0} steps is over the limit of {MAX_STEPS}")]
    TooManySteps(usize),
    #[error("repeat {0} is outside 1-{MAX_REPEAT}")]
    RepeatOutOfRange(u8),
    #[error("name must be 1-{MAX_NAME_CHARS} characters")]
    NameLength,
    #[error("name `{0}` is already used")]
    NameTaken(String),
    #[error("no more than {MAX_MACROS} macros")]
    TooManyMacros,
    #[error("key {0} is already bound")]
    TriggerTaken(i64),
    #[error("macro index {0} does not exist")]
    UnknownIndex(usize),
    #[error("invalid macro JSON: {0}")]
    Json(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MacroPhase {
    Down,
    Up,
    Tap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroStep {
    pub action: String,
    pub phase: MacroPhase,
    pub delay_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Macro {
    pub name: String,
    pub trigger: i64,
    pub repeat: u8,
    pub steps: Vec<MacroStep>,
}

impl Macro {
    pub fn parse(json: &str) -> Result<Self, MacroError> {
        serde_json::from_str(json).map_err(|e| MacroError::Json(e.to_string()))
    }

    pub fn timeline(&self) -> Vec<(f64, &str, bool)> {
        let mut at = 0.0;
        let mut out = Vec::with_capacity(self.steps.len() * usize::from(self.repeat) * 2);
        for _ in 0..self.repeat {
            for step in &self.steps {
                at += f64::from(step.delay_ms);
                let action = step.action.as_str();
                match step.phase {
                    MacroPhase::Down => out.push((at, action, true)),
                    MacroPhase::Up => out.push((at, action, false)),
                    MacroPhase::Tap => {
                        out.push((at, action, true));
                        out.push((at, action, false));
                    }
                }
            }
        }
        out
    }
}

pub fn actions(schema: &Schema) -> Vec<&str> {
    schema
        .group(CONTROL_GROUP)
        .into_iter()
        .filter(|a| !schema.ui_actions().iter().any(|u| u == a))
        .collect()
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct MacroBook {
    macros: Vec<Macro>,
}

impl MacroBook {
    pub fn load(stored: &str, schema: &Schema, settings: &Settings) -> (Self, Vec<MacroError>) {
        let mut book = Self::default();
        let items = match serde_json::from_str::<Value>(stored) {
            Ok(Value::Array(items)) => items,
            Ok(other) => {
                return (
                    book,
                    vec![MacroError::Json(format!(
                        "expected an array, found {other}"
                    ))],
                )
            }
            Err(e) => return (book, vec![MacroError::Json(e.to_string())]),
        };
        let mut issues = Vec::new();
        for item in items {
            let added = serde_json::from_value::<Macro>(item)
                .map_err(|e| MacroError::Json(e.to_string()))
                .and_then(|m| book.set(None, m, schema, settings));
            if let Err(e) = added {
                issues.push(e);
            }
        }
        (book, issues)
    }

    pub fn macros(&self) -> &[Macro] {
        &self.macros
    }

    pub fn to_stored(&self) -> String {
        serde_json::to_string(&self.macros).expect("macros serialize to JSON")
    }

    pub fn release_trigger(&mut self, code: i64, keep: Option<usize>) {
        for (i, m) in self.macros.iter_mut().enumerate() {
            if m.trigger == code && Some(i) != keep {
                m.trigger = NO_TRIGGER;
            }
        }
    }

    pub fn set(
        &mut self,
        index: Option<usize>,
        m: Macro,
        schema: &Schema,
        settings: &Settings,
    ) -> Result<(), MacroError> {
        match index {
            Some(i) if i >= self.macros.len() => return Err(MacroError::UnknownIndex(i)),
            None if self.macros.len() >= MAX_MACROS => return Err(MacroError::TooManyMacros),
            _ => {}
        }
        self.validate(index, &m, schema, settings)?;
        match index {
            Some(i) => self.macros[i] = m,
            None => self.macros.push(m),
        }
        Ok(())
    }

    pub fn remove(&mut self, index: usize) -> Result<(), MacroError> {
        if index >= self.macros.len() {
            return Err(MacroError::UnknownIndex(index));
        }
        self.macros.remove(index);
        Ok(())
    }

    pub fn move_to(&mut self, from: usize, to: usize) -> Result<(), MacroError> {
        if let Some(&bad) = [from, to].iter().find(|&&i| i >= self.macros.len()) {
            return Err(MacroError::UnknownIndex(bad));
        }
        let m = self.macros.remove(from);
        self.macros.insert(to, m);
        Ok(())
    }

    fn validate(
        &self,
        index: Option<usize>,
        m: &Macro,
        schema: &Schema,
        settings: &Settings,
    ) -> Result<(), MacroError> {
        let chars = m.name.chars().count();
        if chars == 0 || chars > MAX_NAME_CHARS {
            return Err(MacroError::NameLength);
        }
        let others = || {
            self.macros
                .iter()
                .enumerate()
                .filter(move |(i, _)| Some(*i) != index)
                .map(|(_, other)| other)
        };
        if others().any(|o| o.name == m.name) {
            return Err(MacroError::NameTaken(m.name.clone()));
        }
        if m.steps.is_empty() {
            return Err(MacroError::NoSteps);
        }
        if m.steps.len() > MAX_STEPS {
            return Err(MacroError::TooManySteps(m.steps.len()));
        }
        if m.repeat == 0 || m.repeat > MAX_REPEAT {
            return Err(MacroError::RepeatOutOfRange(m.repeat));
        }
        let allowed = actions(schema);
        for step in &m.steps {
            if !allowed.contains(&step.action.as_str()) {
                return Err(MacroError::UnknownAction(step.action.clone()));
            }
            if step.delay_ms > MAX_DELAY_MS {
                return Err(MacroError::DelayOutOfRange(step.delay_ms));
            }
        }
        if m.trigger != NO_TRIGGER
            && (!settings.hotkey_available(HotkeyKind::Control, m.trigger)
                || others().any(|o| o.trigger == m.trigger))
        {
            return Err(MacroError::TriggerTaken(m.trigger));
        }
        Ok(())
    }
}
