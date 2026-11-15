use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::world::Tab;

pub const SHARED_PROFILE_COUNT: usize = 100;
pub const NAME_MAX: usize = 15;
pub const LABEL_MAX: usize = 16;
const PROFILE_INDEX_KEY: &str = "profileIndex";
const LINKED_TAB2: &str = "-1";

#[derive(Debug, Error, PartialEq)]
pub enum ProfileError {
    #[error("profile index {0} is out of range")]
    Index(usize),
    #[error("unknown profile field `{0}`")]
    Field(String),
    #[error("stored profiles: {0}")]
    Stored(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    pub skin_url: String,
    pub skin_url2: String,
    pub self_skin_url: String,
    pub self_skin_url2: String,
    pub tab_name: String,
    pub tab_skin_url: String,
    pub tab_skin_url2: String,
    pub self_tab_skin_url: String,
    pub self_tab_skin_url2: String,
    pub profile_index: usize,
    pub label: String,
}

impl Profile {
    pub fn fresh(index: usize) -> Self {
        let skin = String::new;
        Self {
            name: format!("Profile{}", index + 1),
            skin_url: skin(),
            skin_url2: skin(),
            self_skin_url: skin(),
            self_skin_url2: skin(),
            tab_name: format!("Tab{}", index + 1),
            tab_skin_url: skin(),
            tab_skin_url2: skin(),
            self_tab_skin_url: skin(),
            self_tab_skin_url2: skin(),
            profile_index: index,
            label: String::new(),
        }
    }

    pub fn display_name(&self) -> &str {
        if self.label.is_empty() {
            &self.name
        } else {
            &self.label
        }
    }

    pub fn set_field(&mut self, field: &str, value: &str) -> Result<bool, ProfileError> {
        let (slot, max) = match field {
            "name" => (&mut self.name, NAME_MAX),
            "tabName" => (&mut self.tab_name, NAME_MAX),
            "label" => (&mut self.label, LABEL_MAX),
            "skinUrl" => (&mut self.skin_url, usize::MAX),
            "skinUrl2" => (&mut self.skin_url2, usize::MAX),
            "selfSkinUrl" => (&mut self.self_skin_url, usize::MAX),
            "selfSkinUrl2" => (&mut self.self_skin_url2, usize::MAX),
            "tabSkinUrl" => (&mut self.tab_skin_url, usize::MAX),
            "tabSkinUrl2" => (&mut self.tab_skin_url2, usize::MAX),
            "selfTabSkinUrl" => (&mut self.self_tab_skin_url, usize::MAX),
            "selfTabSkinUrl2" => (&mut self.self_tab_skin_url2, usize::MAX),
            _ => return Err(ProfileError::Field(field.to_owned())),
        };
        let clipped: String = value.chars().take(max).collect();
        if *slot == clipped {
            return Ok(false);
        }
        *slot = clipped;
        Ok(true)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StoredProfileKeys<'a> {
    pub entries: Option<&'a str>,
    pub extra: Option<&'a str>,
    pub selected: Option<&'a str>,
    pub selected_extra: Option<&'a str>,
    pub selected_tab2: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredSelection {
    pub selected: String,
    pub selected_extra: Option<String>,
    pub selected_tab2: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredProfiles {
    pub entries: String,
    pub extra: Option<String>,
    #[serde(flatten)]
    pub selection: StoredSelection,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileChanges {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Option<String>>,
    #[serde(flatten)]
    pub selection: StoredSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct ExtraSelection {
    main: usize,
    shared: usize,
}

#[derive(Debug, Clone)]
pub struct Profiles {
    entries: Vec<Profile>,
    main: usize,
    shared_main: usize,
    second: Option<usize>,
    shared_dirty: bool,
    extra_dirty: bool,
}

impl PartialEq for Profiles {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
            && self.main == other.main
            && self.shared_main == other.shared_main
            && self.second == other.second
    }
}

fn merge_stored(slot: &mut Profile, raw: Value, issues: &mut Vec<ProfileError>) {
    let index = slot.profile_index;
    let Value::Object(over) = raw else {
        issues.push(ProfileError::Stored(format!(
            "profile {index} is not an object: {raw}"
        )));
        return;
    };
    let mut merged = serde_json::to_value(&*slot).expect("profile serializes");
    let base = merged
        .as_object_mut()
        .expect("profile serializes to an object");
    for (k, v) in over {
        if k == PROFILE_INDEX_KEY || !base.contains_key(&k) {
            continue;
        }
        if v.is_string() {
            base.insert(k, v);
        } else {
            issues.push(ProfileError::Stored(format!(
                "profile {index} `{k}` is not a string: {v}"
            )));
        }
    }
    let p: Profile = serde_json::from_value(merged).expect("string fields form a profile");
    *slot = Profile {
        profile_index: index,
        ..p
    };
}

impl Default for Profiles {
    fn default() -> Self {
        Self {
            entries: (0..SHARED_PROFILE_COUNT).map(Profile::fresh).collect(),
            main: 0,
            shared_main: 0,
            second: None,
            shared_dirty: false,
            extra_dirty: false,
        }
    }
}

impl Profiles {
    pub fn is_untouched(&self, profile: &Profile) -> bool {
        *profile == Profile::fresh(profile.profile_index)
    }

    pub fn load(&mut self, stored: StoredProfileKeys<'_>) -> Vec<ProfileError> {
        let mut issues = Vec::new();
        match stored.entries.map(serde_json::from_str::<Value>) {
            None => {}
            Some(Ok(Value::Array(saved))) => {
                for (slot, raw) in self.entries[..SHARED_PROFILE_COUNT].iter_mut().zip(saved) {
                    merge_stored(slot, raw, &mut issues);
                }
            }
            Some(Ok(other)) => issues.push(ProfileError::Stored(format!(
                "profiles are not an array: {other}"
            ))),
            Some(Err(e)) => issues.push(ProfileError::Stored(e.to_string())),
        }
        match stored.extra.map(serde_json::from_str::<Value>) {
            None => self.entries.truncate(SHARED_PROFILE_COUNT),
            Some(Ok(Value::Array(saved))) => {
                self.entries.truncate(SHARED_PROFILE_COUNT);
                for raw in saved {
                    let mut slot = Profile::fresh(self.entries.len());
                    merge_stored(&mut slot, raw, &mut issues);
                    self.entries.push(slot);
                }
            }
            Some(Ok(other)) => issues.push(ProfileError::Stored(format!(
                "extra profiles are not an array: {other}"
            ))),
            Some(Err(e)) => issues.push(ProfileError::Stored(format!("extra profiles: {e}"))),
        }
        if let Some(s) = stored.selected {
            match s.trim().parse::<usize>() {
                Ok(i) => {
                    if i >= SHARED_PROFILE_COUNT {
                        issues.push(ProfileError::Stored(format!(
                            "selected profile {i} is out of range"
                        )));
                    }
                    self.shared_main = i.min(SHARED_PROFILE_COUNT - 1);
                }
                Err(_) => issues.push(ProfileError::Stored(format!(
                    "selected profile `{s}` is not an index"
                ))),
            }
        }
        self.main = self.shared_main;
        match stored
            .selected_extra
            .map(serde_json::from_str::<ExtraSelection>)
        {
            None => {}
            Some(Ok(sel)) if sel.main < self.entries.len() => self.set_main(sel.main),
            Some(Ok(sel)) => issues.push(ProfileError::Stored(format!(
                "selected profile {} is out of range",
                sel.main
            ))),
            Some(Err(e)) => {
                issues.push(ProfileError::Stored(format!("selected extra profile: {e}")))
            }
        }
        let last = self.entries.len() - 1;
        self.second = match stored.selected_tab2.map(str::trim) {
            None | Some(LINKED_TAB2) => None,
            Some(s) => match s.parse::<usize>() {
                Ok(i) => {
                    if i > last {
                        issues.push(ProfileError::Stored(format!(
                            "second tab profile {i} is out of range"
                        )));
                    }
                    Some(i.min(last))
                }
                Err(_) => {
                    issues.push(ProfileError::Stored(format!(
                        "second tab profile `{s}` is not an index"
                    )));
                    None
                }
            },
        };
        self.shared_dirty = false;
        self.extra_dirty = false;
        issues
    }

    pub fn stored(&self) -> StoredProfiles {
        let (shared, extra) = self.entries.split_at(SHARED_PROFILE_COUNT);
        StoredProfiles {
            entries: serde_json::to_string(shared).expect("profiles serialize to JSON"),
            extra: (!extra.is_empty())
                .then(|| serde_json::to_string(extra).expect("profiles serialize to JSON")),
            selection: self.stored_selection(),
        }
    }

    pub fn take_changes(&mut self) -> ProfileChanges {
        let (shared, extra) = self.entries.split_at(SHARED_PROFILE_COUNT);
        let changes = ProfileChanges {
            entries: self
                .shared_dirty
                .then(|| serde_json::to_string(shared).expect("profiles serialize to JSON")),
            extra: self.extra_dirty.then(|| {
                (!extra.is_empty())
                    .then(|| serde_json::to_string(extra).expect("profiles serialize to JSON"))
            }),
            selection: self.stored_selection(),
        };
        self.shared_dirty = false;
        self.extra_dirty = false;
        changes
    }

    pub fn stored_selection(&self) -> StoredSelection {
        StoredSelection {
            selected: self.shared_main.to_string(),
            selected_extra: (self.main >= SHARED_PROFILE_COUNT).then(|| {
                serde_json::to_string(&ExtraSelection {
                    main: self.main,
                    shared: self.shared_main,
                })
                .expect("selection serializes to JSON")
            }),
            selected_tab2: self
                .second
                .map_or_else(|| LINKED_TAB2.to_owned(), |i| i.to_string()),
        }
    }

    pub fn entries(&self) -> &[Profile] {
        &self.entries
    }

    pub fn get(&self, i: usize) -> Result<&Profile, ProfileError> {
        self.entries.get(i).ok_or(ProfileError::Index(i))
    }

    pub fn linked(&self) -> bool {
        self.second.is_none()
    }

    pub fn selected(&self, tab: Tab) -> usize {
        match (tab, self.second) {
            (Tab::Second, Some(i)) => i,
            _ => self.main,
        }
    }

    fn set_main(&mut self, i: usize) {
        self.main = i;
        if i < SHARED_PROFILE_COUNT {
            self.shared_main = i;
        }
    }

    pub fn select(&mut self, tab: Tab, i: usize) -> Result<(), ProfileError> {
        if i >= self.entries.len() {
            return Err(ProfileError::Index(i));
        }
        match (tab, self.second.is_some()) {
            (Tab::Second, true) => self.second = Some(i),
            _ => self.set_main(i),
        }
        Ok(())
    }

    pub fn set_linked(&mut self, linked: bool) {
        self.second = if linked { None } else { Some(self.main) };
    }

    pub fn add(&mut self) -> usize {
        let i = self.entries.len();
        self.entries.push(Profile::fresh(i));
        self.extra_dirty = true;
        i
    }

    pub fn remove(&mut self, i: usize) -> Result<(), ProfileError> {
        if i >= self.entries.len() {
            return Err(ProfileError::Index(i));
        }
        let had_extra = self.entries.len() > SHARED_PROFILE_COUNT;
        self.entries.remove(i);
        if self.entries.len() < SHARED_PROFILE_COUNT {
            self.entries.push(Profile::fresh(self.entries.len()));
        }
        self.shared_dirty |= i < SHARED_PROFILE_COUNT;
        self.extra_dirty |= had_extra;
        self.renumber(i, self.entries.len());
        let last = self.entries.len() - 1;
        let shift = |x: usize| if x > i { x - 1 } else { x.min(last) };
        self.shared_main = shift(self.shared_main).min(SHARED_PROFILE_COUNT - 1);
        self.set_main(shift(self.main));
        self.second = self.second.map(shift);
        Ok(())
    }

    pub fn move_to(&mut self, from: usize, to: usize) -> Result<(), ProfileError> {
        let len = self.entries.len();
        if let Some(out) = [from, to].into_iter().find(|&i| i >= len) {
            return Err(ProfileError::Index(out));
        }
        if from == to {
            return Ok(());
        }
        let (low, high) = (from.min(to), from.max(to));
        let moved = self.entries.remove(from);
        self.entries.insert(to, moved);
        self.renumber(low, high + 1);
        self.shared_dirty |= low < SHARED_PROFILE_COUNT;
        self.extra_dirty |= high >= SHARED_PROFILE_COUNT;
        let follow = |x: usize| {
            if x == from {
                to
            } else if !(low..=high).contains(&x) {
                x
            } else if from < to {
                x - 1
            } else {
                x + 1
            }
        };
        self.shared_main = follow(self.shared_main).min(SHARED_PROFILE_COUNT - 1);
        self.set_main(follow(self.main));
        self.second = self.second.map(follow);
        Ok(())
    }

    fn renumber(&mut self, from: usize, to: usize) {
        for (j, p) in self.entries.iter_mut().enumerate().take(to).skip(from) {
            if *p == Profile::fresh(p.profile_index) {
                *p = Profile::fresh(j);
            } else {
                p.profile_index = j;
            }
        }
    }

    pub fn set_field(&mut self, i: usize, field: &str, value: &str) -> Result<bool, ProfileError> {
        let changed = self
            .entries
            .get_mut(i)
            .ok_or(ProfileError::Index(i))?
            .set_field(field, value)?;
        if changed {
            if i < SHARED_PROFILE_COUNT {
                self.shared_dirty = true;
            } else {
                self.extra_dirty = true;
            }
        }
        Ok(changed)
    }
}
