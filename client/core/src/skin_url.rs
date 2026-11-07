pub const MAX_SIDE: u32 = 2048;
const SKIN_DIR: &str = "skins/";
const IMGUR: &str = "https://i.imgur.com/";
const IMGUR_EXTENSIONS: [&str; 4] = [".png", ".jpg", ".jpeg", ".webp"];

pub fn is_imgur_skin(url: &str) -> bool {
    let Some(file) = url.strip_prefix(IMGUR) else {
        return false;
    };
    IMGUR_EXTENSIONS.iter().any(|extension| {
        file.strip_suffix(extension).is_some_and(|id| {
            (5..=12).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric())
        })
    })
}

pub fn sanitize_skin_url(url: &str) -> String {
    let trimmed = url.trim();
    let web = trimmed.starts_with("https://") || trimmed.starts_with("http://");
    if web && !trimmed.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return trimmed.to_owned();
    }
    let path = trimmed
        .strip_prefix("./")
        .or_else(|| trimmed.strip_prefix('/'))
        .unwrap_or(trimmed);
    let plain = path.strip_prefix(SKIN_DIR).is_some_and(|file| {
        !file.is_empty()
            && !file.starts_with('.')
            && file
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    });
    if plain {
        path.to_owned()
    } else {
        String::new()
    }
}
