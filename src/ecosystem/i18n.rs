use std::collections::HashMap;
use std::fs;

use bevy::prelude::*;
use fluent_bundle::{FluentBundle, FluentResource};

#[derive(Resource)]
pub struct LocaleResources {
    pub current: String,
    pub available: Vec<String>,
    pub translations: HashMap<String, String>,
    all: HashMap<String, HashMap<String, String>>,
}

fn load_all_translations() -> HashMap<String, HashMap<String, String>> {
    let mut all = HashMap::new();
    let locales_dir = "assets/locales";
    if let Ok(entries) = fs::read_dir(locales_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if let Some(name) = entry.file_name().to_str().map(|s| s.to_string()) {
                    let ftl_path = entry.path().join("main.ftl");
                    if let Ok(ftl_content) = fs::read_to_string(&ftl_path) {
                        if let Ok(res) = FluentResource::try_new(ftl_content) {
                            let langid: unic_langid::LanguageIdentifier =
                                name.parse().unwrap_or_else(|_| "en".parse().unwrap());
                            let mut bundle = FluentBundle::new(vec![langid]);
                            bundle.set_use_isolating(false);
                            if bundle.add_resource(res).is_ok() {
                                let translations = resolve_translations(&bundle);
                                all.insert(name, translations);
                            }
                        }
                    }
                }
            }
        }
    }
    if all.is_empty() {
        let langid: unic_langid::LanguageIdentifier = "en".parse().unwrap();
        let mut bundle = FluentBundle::new(vec![langid]);
        bundle.set_use_isolating(false);
        all.insert("en".to_string(), resolve_translations(&bundle));
    }
    all
}

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

fn resolve_translations(bundle: &FluentBundle<FluentResource>) -> HashMap<String, String> {
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
    map
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
        let all = load_all_translations();
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

pub struct I18nPlugin;
impl Plugin for I18nPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LocaleResources>();
    }
}

pub fn translate<'a>(locale: &'a LocaleResources, key: &'a str) -> &'a str {
    locale.translate(key).unwrap_or(key)
}
