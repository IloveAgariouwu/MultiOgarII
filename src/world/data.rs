use super::player::{ROLE_ADMIN, ROLE_GUEST, ROLE_MODER, ROLE_USER};
use crate::core::logger;
use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct User {
    pub ip: String,
    pub password: String,
    pub role: u8,
    pub name: String,
}

#[derive(Deserialize)]
struct RawUser {
    ip: String,
    password: String,
    role: String,
    name: String,
}

#[derive(Default)]
pub struct Data {
    pub bad_words: Vec<String>,
    pub random_skins: Vec<String>,
    pub bot_names: Vec<String>,
    pub ip_bans: Vec<String>,
    pub users: Vec<User>,
}

fn read_lines(path: &Path) -> Option<Vec<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Some(
            text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).collect(),
        ),
        Err(_) => {
            logger::warn(&format!("{} not found", path.display()));
            None
        }
    }
}

impl Data {
    pub fn load(dir: &Path) -> Self {
        let mut data = Data::default();
        if let Some(words) = read_lines(&dir.join("badwords.txt")) {
            data.bad_words =
                words.iter().map(|w| format!(" {} ", w.to_lowercase())).collect();
            logger::info(&format!("{} bad words loaded", data.bad_words.len()));
        }
        data.random_skins = read_lines(&dir.join("randomskins.txt")).unwrap_or_default();
        if dir.join("botnames.txt").exists() {
            data.bot_names = read_lines(&dir.join("botnames.txt")).unwrap_or_default();
        }
        if let Some(bans) = read_lines(&dir.join("ipbanlist.txt")) {
            logger::info(&format!("{} IP ban records loaded.", bans.len()));
            data.ip_bans = bans;
        }
        match std::fs::read_to_string(dir.join("userRoles.json")) {
            Ok(text) => match parse_users(&text) {
                Ok(users) => {
                    logger::info(&format!("{} user records loaded.", users.len()));
                    data.users = users;
                }
                Err(e) => logger::error(&format!("Failed to load userRoles.json: {e}")),
            },
            Err(_) => logger::warn("userRoles.json is missing"),
        }
        data
    }

    pub fn contains_bad_word(&self, text: &str) -> bool {
        let padded = format!(" {} ", text.trim().to_lowercase());
        self.bad_words.iter().any(|word| padded.contains(word.as_str()))
    }

    pub fn is_ip_banned(&self, ip: &str) -> bool {
        if ip == "127.0.0.1" {
            return false;
        }
        if self.ip_bans.iter().any(|b| b == ip) {
            return true;
        }
        let parts: Vec<&str> = ip.split('.').collect();
        let [a, b, c, _] = parts[..] else { return false };
        let (subnet2, subnet1) = (format!("{a}.{b}.*.*"), format!("{a}.{b}.{c}.*"));
        self.ip_bans.iter().any(|ban| *ban == subnet2 || *ban == subnet1)
    }
}

pub fn parse_users(text: &str) -> Result<Vec<User>, serde_json::Error> {
    let raw: Vec<serde_json::Value> = serde_json::from_str(text)?;
    let mut users = Vec::new();
    for entry in raw {
        let Ok(item) = serde_json::from_value::<RawUser>(entry) else { continue };
        let password = item.password.trim();
        if password.is_empty() {
            logger::warn(&format!("User account \"{}\" disabled", item.name));
            continue;
        }
        let role = match item.role.as_str() {
            "GUEST" => ROLE_GUEST,
            "USER" => ROLE_USER,
            "MODER" => ROLE_MODER,
            "ADMIN" => ROLE_ADMIN,
            other => {
                logger::warn(&format!("Unknown user role: {other}"));
                ROLE_USER
            }
        };
        users.push(User {
            ip: item.ip.trim().to_owned(),
            password: password.to_owned(),
            role,
            name: item.name.trim().to_owned(),
        });
    }
    Ok(users)
}
