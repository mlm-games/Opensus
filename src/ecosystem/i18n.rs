use std::collections::HashMap;

use bevy::prelude::*;
use fluent_bundle::{FluentBundle, FluentResource};

const TRANSLATION_KEYS: &[&str] = &[
    "app-title",
    "start-game",
    "settings",
    "credits",
    "quit",
    "paused",
    "resume",
    "quit-to-title",
    "save",
    "back",
    "master-volume",
    "sfx-volume",
    "music-volume",
    "language",
    "score",
    "best",
    "controls-hint",
    "loading",
];

fn load_ftl(locale: &str, ftl: &str) -> (String, HashMap<String, String>) {
    if let Ok(res) = FluentResource::try_new(ftl.to_string()) {
        let langid: unic_langid::LanguageIdentifier =
            locale.parse().unwrap_or_else(|_| "en".parse().unwrap());
        let mut bundle = FluentBundle::new(vec![langid]);
        bundle.set_use_isolating(false);
        if bundle.add_resource(res).is_ok() {
            let mut map = HashMap::new();
            for key in TRANSLATION_KEYS {
                let value = bundle
                    .get_message(key)
                    .and_then(|msg| msg.value())
                    .map(|pattern| {
                        bundle
                            .format_pattern(pattern, None, &mut Vec::new())
                            .into_owned()
                    })
                    .unwrap_or_else(|| key.to_string());
                map.insert(key.to_string(), value);
            }
            return (locale.to_string(), map);
        }
    }
    (locale.to_string(), HashMap::new())
}

/// Embedded at compile time (for WASM/Android).
fn embedded_translations() -> HashMap<String, HashMap<String, String>> {
    let mut all = HashMap::new();
    for (locale, ftl) in [
        ("en", include_str!("../../assets/locales/en/main.ftl")),
        ("es", include_str!("../../assets/locales/es/main.ftl")),
        ("fr", include_str!("../../assets/locales/fr/main.ftl")),
        ("de", include_str!("../../assets/locales/de/main.ftl")),
        ("ja", include_str!("../../assets/locales/ja/main.ftl")),
        ("zh", include_str!("../../assets/locales/zh/main.ftl")),
        ("pt", include_str!("../../assets/locales/pt/main.ftl")),
    ] {
        let (loc, map) = load_ftl(locale, ftl);
        all.insert(loc, map);
    }
    all
}

#[derive(Resource)]
pub struct LocaleResources {
    pub current: String,
    pub available: Vec<String>,
    pub translations: HashMap<String, String>,
    all: HashMap<String, HashMap<String, String>>,
}

impl LocaleResources {
    pub fn set_locale(&mut self, locale: &str) {
        if self.all.contains_key(locale) {
            self.current = locale.to_string();
            self.translations = self.all[locale].clone();
        }
    }

    pub fn translate(&self, key: &str) -> Option<&str> {
        self.translations.get(key).map(String::as_str)
    }
}

impl Default for LocaleResources {
    fn default() -> Self {
        let all = embedded_translations();
        let mut available: Vec<String> = all.keys().cloned().collect();
        available.sort();
        let current = if available.contains(&"en".to_string()) {
            "en".to_string()
        } else {
            available
                .first()
                .cloned()
                .unwrap_or_else(|| "en".to_string())
        };
        let translations = all.get(&current).cloned().unwrap_or_default();
        Self {
            current,
            available,
            translations,
            all,
        }
    }
}

pub fn get_current_translations(locale: &LocaleResources) -> HashMap<String, String> {
    locale.translations.clone()
}

pub fn translate<'a>(locale: &'a LocaleResources, key: &'a str) -> &'a str {
    locale.translate(key).unwrap_or(key)
}

pub struct I18nPlugin;
impl Plugin for I18nPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LocaleResources>();
    }
}
