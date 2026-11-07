use serde_json::{Map, Value};

use crate::settings::Schema;
use crate::themes::{
    check_name, PlayerTheme, StoredBook, StoredBookRef, ThemeError, MAX_NAME_CHARS, MAX_THEMES,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Colors {
    builtin: Map<String, Value>,
    themes: Vec<PlayerTheme>,
    selected: Option<usize>,
}

impl Colors {
    pub fn new(schema: &Schema) -> Self {
        Self {
            builtin: schema.builtin_colors().clone(),
            themes: Vec::new(),
            selected: None,
        }
    }

    pub fn load(&mut self, stored: &str) -> Vec<ThemeError> {
        let book: StoredBook = match serde_json::from_str(stored) {
            Ok(book) => book,
            Err(e) => return vec![ThemeError::Malformed(e.to_string())],
        };
        let mut issues = Vec::new();
        for value in book.themes {
            match PlayerTheme::parse(value, &self.builtin) {
                Ok((theme, theme_issues)) => {
                    issues.extend(theme_issues);
                    if let Err(e) = self.admit(&theme.name) {
                        issues.push(e);
                    } else {
                        self.themes.push(theme);
                    }
                }
                Err(e) => issues.push(e),
            }
        }
        if let Some(name) = book.selected {
            match self.position(&name) {
                Some(i) => self.selected = Some(i),
                None => issues.push(ThemeError::UnknownTheme(name)),
            }
        }
        issues
    }

    pub fn get(&self, key: &str) -> Option<u32> {
        self.defs()
            .get(key)
            .and_then(Value::as_f64)
            .map(|v| v as u32)
    }

    pub fn defs(&self) -> &Map<String, Value> {
        self.selected
            .map_or(&self.builtin, |i| &self.themes[i].colors)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.themes.iter().map(|t| t.name.as_str())
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.map(|i| self.themes[i].name.as_str())
    }

    pub fn set(&mut self, key: &str, argb: u32, fork_name: &str) -> Result<bool, ThemeError> {
        let value = Value::from(argb);
        let Some(current) = self.defs().get(key) else {
            return Err(ThemeError::UnknownColor {
                theme: self.selected().unwrap_or(fork_name).to_owned(),
                key: key.to_owned(),
            });
        };
        if *current == value {
            return Ok(false);
        }
        let index = match self.selected {
            Some(i) => i,
            None => {
                if self.themes.len() >= MAX_THEMES {
                    return Err(ThemeError::TooManyThemes);
                }
                self.push(PlayerTheme {
                    name: self.unique_name(fork_name),
                    colors: self.builtin.clone(),
                })
            }
        };
        self.themes[index].colors.insert(key.to_owned(), value);
        Ok(true)
    }

    pub fn save_as(&mut self, name: &str) -> Result<(), ThemeError> {
        let name = check_name(name)?;
        self.admit(name)?;
        self.push(PlayerTheme {
            name: name.to_owned(),
            colors: self.defs().clone(),
        });
        Ok(())
    }

    pub fn select(&mut self, name: Option<&str>) -> Result<bool, ThemeError> {
        let index = name
            .map(|n| {
                self.position(n)
                    .ok_or_else(|| ThemeError::UnknownTheme(n.to_owned()))
            })
            .transpose()?;
        let changed = index != self.selected;
        self.selected = index;
        Ok(changed)
    }

    pub fn rename(&mut self, name: &str) -> Result<(), ThemeError> {
        let index = self.selected.ok_or(ThemeError::Builtin)?;
        let name = check_name(name)?;
        if self.position(name).is_some_and(|i| i != index) {
            return Err(ThemeError::NameTaken(name.to_owned()));
        }
        name.clone_into(&mut self.themes[index].name);
        Ok(())
    }

    pub fn delete(&mut self) -> Result<(), ThemeError> {
        let index = self.selected.take().ok_or(ThemeError::Builtin)?;
        self.themes.remove(index);
        Ok(())
    }

    pub fn export(&self) -> Result<String, ThemeError> {
        let index = self.selected.ok_or(ThemeError::Builtin)?;
        Ok(serde_json::to_string(&self.themes[index]).expect("theme serializes to JSON"))
    }

    pub fn import(&mut self, json: &str) -> Result<Vec<ThemeError>, ThemeError> {
        let value: Value =
            serde_json::from_str(json).map_err(|e| ThemeError::Malformed(e.to_string()))?;
        let (mut theme, issues) = PlayerTheme::parse(value, &self.builtin)?;
        if self.themes.len() >= MAX_THEMES {
            return Err(ThemeError::TooManyThemes);
        }
        theme.name = self.unique_name(&theme.name);
        self.push(theme);
        Ok(issues)
    }

    pub fn select_builtin(&mut self) {
        self.selected = None;
    }

    pub fn to_stored(&self) -> String {
        serde_json::to_string(&StoredBookRef {
            selected: self.selected(),
            themes: &self.themes,
        })
        .expect("themes serialize to JSON")
    }

    fn position(&self, name: &str) -> Option<usize> {
        self.themes.iter().position(|t| t.name == name)
    }

    fn admit(&self, name: &str) -> Result<(), ThemeError> {
        if self.position(name).is_some() {
            return Err(ThemeError::NameTaken(name.to_owned()));
        }
        if self.themes.len() >= MAX_THEMES {
            return Err(ThemeError::TooManyThemes);
        }
        Ok(())
    }

    fn push(&mut self, theme: PlayerTheme) -> usize {
        self.themes.push(theme);
        let index = self.themes.len() - 1;
        self.selected = Some(index);
        index
    }

    fn unique_name(&self, base: &str) -> String {
        let base: String = base.chars().take(MAX_NAME_CHARS).collect();
        if self.position(&base).is_none() {
            return base;
        }
        (2..)
            .map(|n| {
                let suffix = format!(" {n}");
                let keep = MAX_NAME_CHARS - suffix.chars().count();
                let mut name: String = base.chars().take(keep).collect();
                name.push_str(&suffix);
                name
            })
            .find(|name| self.position(name).is_none())
            .expect("fewer themes than numbers")
    }
}
