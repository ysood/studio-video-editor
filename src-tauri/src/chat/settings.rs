//! What the person has chosen about how the assistant runs.
//!
//! Kept apart from the credentials, and written in the clear. These are preferences, not
//! secrets: a model slug and a word like "medium" say nothing about anybody, and reading
//! them is not a way in. The credentials file is the one that gets careful handling,
//! and mixing a thing you rewrite whenever a dropdown changes into the file holding the
//! only copy of a refresh token would be a way to lose the token to a bug in a dropdown.
//!
//! Both fields are optional, and absent means "whatever the service thinks". That is not
//! laziness: a slug written down here outlives the model it names, so the stored form has
//! to be able to say "I have no opinion" and mean it.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use ts_rs::TS;

const VERSION: u32 = 1;
const FILE: &str = "openai-settings.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatSettings {
    /// The model to ask, or none to take whatever the service lists first.
    pub model: Option<String>,
    /// How hard to let it think, or none to take the model's own default.
    pub effort: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Stored {
    version: u32,
    settings: ChatSettings,
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_config_dir().ok()?.join(FILE))
}

/// What was chosen, or nothing chosen.
///
/// A file that will not parse is treated as no choice, which is the right answer here
/// and the wrong one for credentials: the worst case is somebody picks their model
/// again, and there is nothing in it that cannot be recreated by one click.
pub fn load(app: &AppHandle) -> ChatSettings {
    let Some(path) = file(app) else {
        return ChatSettings::default();
    };
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Stored>(&text).ok())
        .filter(|stored| stored.version == VERSION)
        .map(|stored| stored.settings)
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, settings: &ChatSettings) -> std::io::Result<()> {
    let path = file(app)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no config directory"))?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(&Stored {
        version: VERSION,
        settings: settings.clone(),
    })?;
    fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn having_no_opinion_is_a_thing_the_stored_form_can_say() {
        // A slug outlives the model it names. Without this, the only way to express
        // "whatever is current" would be to write down today's answer forever.
        let empty = ChatSettings::default();
        assert_eq!(empty.model, None);
        assert_eq!(empty.effort, None);
        let text = serde_json::to_string(&Stored {
            version: VERSION,
            settings: empty.clone(),
        })
        .unwrap();
        let read: Stored = serde_json::from_str(&text).unwrap();
        assert_eq!(read.settings, empty);
    }

    #[test]
    fn a_choice_survives_the_round_trip() {
        let chosen = ChatSettings {
            model: Some("gpt-6-astra".into()),
            effort: Some("high".into()),
        };
        let text = serde_json::to_string(&Stored {
            version: VERSION,
            settings: chosen.clone(),
        })
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Stored>(&text).unwrap().settings,
            chosen
        );
    }

    #[test]
    fn a_file_from_a_later_version_is_not_read_as_this_one() {
        let text = serde_json::to_string(&serde_json::json!({
            "version": VERSION + 1,
            "settings": { "model": "something-else" },
        }))
        .unwrap();
        let usable = serde_json::from_str::<Stored>(&text)
            .ok()
            .filter(|s| s.version == VERSION);
        assert!(usable.is_none());
    }
}
