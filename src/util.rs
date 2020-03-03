use lazy_static::*;
use std::collections::HashMap;
use tokio::sync::RwLock;

lazy_static! {
    pub static ref WEB_CLIENT: reqwest::Client = reqwest::Client::builder()
        .user_agent("ketera-bot (kiwiyou.dev@gmail.com)")
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Failed to create https client");
    pub static ref CALLBACK_SESSIONS: RwLock<HashMap<(i64, i32), CallbackSession>> =
        RwLock::new(HashMap::new());
}

#[derive(Clone)]
pub enum CallbackSession {
    Docs,
}

pub fn escape_html_entities(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn size_humanize(number: usize) -> String {
    if number >= 1_000_000_000 {
        format!("{:.1}G", number as f64 / 1e9)
    } else if number >= 1_000_000 {
        format!("{:.1}M", number as f64 / 1e6)
    } else if number > 1_000 {
        format!("{:.1}k", number as f64 / 1e3)
    } else {
        number.to_string()
    }
}
