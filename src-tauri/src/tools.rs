//! Finding ffmpeg and ffprobe.
//!
//! Apps launched from Finder do not inherit the shell's PATH, so Homebrew's directories
//! are searched explicitly after any override and PATH.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use ts_rs::TS;

const FALLBACK_DIRS: [&str; 3] = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"];

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolsStatus {
    pub ffmpeg: Option<String>,
    pub ffprobe: Option<String>,
}

pub fn status() -> ToolsStatus {
    ToolsStatus {
        ffmpeg: locate("ffmpeg").map(|p| p.display().to_string()),
        ffprobe: locate("ffprobe").map(|p| p.display().to_string()),
    }
}

/// `STUDIO_FFMPEG` / `STUDIO_FFPROBE` override, then PATH, then Homebrew locations.
pub fn locate(tool: &str) -> Option<PathBuf> {
    let override_var = format!("STUDIO_{}", tool.to_uppercase());
    let from_env = std::env::var_os(override_var).map(PathBuf::from);
    let from_path = std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths)
                .map(|d| d.join(tool))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let fallbacks = FALLBACK_DIRS.iter().map(|d| Path::new(d).join(tool));
    from_env
        .into_iter()
        .chain(from_path)
        .chain(fallbacks)
        .find(|candidate| runs(candidate))
}

fn runs(candidate: &Path) -> bool {
    candidate.is_file()
        && Command::new(candidate)
            .arg("-version")
            .output()
            .is_ok_and(|o| o.status.success())
}
