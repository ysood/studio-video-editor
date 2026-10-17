//! Conform packages: a self-contained folder another editor can open on any machine.
//!
//! ```text
//! <Name>_Conform/
//!   timeline.otio      OpenTimelineIO, paths pointing into media/
//!   timeline.xml       FCP7 XML for Premiere and Resolve
//!   manifest.json      provenance: original paths and fingerprints
//!   media/             every file the timeline uses, copied, names made unique
//! ```
//!
//! Only media used on the timeline is copied. Interchange files reference the copies by
//! absolute path at export time; Premiere and Resolve relink by file name if the folder
//! moves, and `manifest.json` carries the relative paths.

use super::{fcp7, otio};
use crate::ids::MediaId;
use crate::model::Project;
use serde_json::json;
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const MEDIA_DIR: &str = "media";

#[derive(Debug, Clone, PartialEq)]
pub struct FileCopy {
    pub from: PathBuf,
    pub to: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PackagePlan {
    pub root: PathBuf,
    pub copies: Vec<FileCopy>,
    /// The project with media paths rewritten to the packaged copies.
    pub project: Project,
}

impl PackagePlan {
    pub fn total_bytes(&self) -> u64 {
        self.copies
            .iter()
            .filter_map(|c| fs::metadata(&c.from).ok())
            .map(|m| m.len())
            .sum()
    }
}

/// Decide what to copy where. Pure: no file is read or written.
pub fn plan(project: &Project, root: &Path) -> PackagePlan {
    let used: HashSet<MediaId> = project
        .all_clips()
        .map(|(_, c)| c.source.media_id())
        .collect();
    let mut packaged = project.clone();
    let mut taken = BTreeSet::new();
    let mut copies = Vec::new();
    for media in packaged.media.iter_mut().filter(|m| used.contains(&m.id)) {
        let file_name = unique_name(&media.path, &mut taken);
        let to = root.join(MEDIA_DIR).join(&file_name);
        copies.push(FileCopy {
            from: PathBuf::from(&media.path),
            to: to.clone(),
        });
        media.path = to.to_string_lossy().into_owned();
    }
    PackagePlan {
        root: root.to_path_buf(),
        copies,
        project: packaged,
    }
}

/// Copy media and write the interchange files. `on_progress` receives bytes copied so
/// far after each file. Refuses to write into a folder that already has contents.
pub fn write(
    original: &Project,
    plan: &PackagePlan,
    mut on_progress: impl FnMut(u64),
) -> io::Result<()> {
    if plan.root.exists() && fs::read_dir(&plan.root)?.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "the package folder is not empty",
        ));
    }
    fs::create_dir_all(plan.root.join(MEDIA_DIR))?;
    let mut copied = 0;
    for copy in &plan.copies {
        copied += fs::copy(&copy.from, &copy.to)?;
        on_progress(copied);
    }
    fs::write(plan.root.join("timeline.otio"), otio::export(&plan.project))?;
    fs::write(plan.root.join("timeline.xml"), fcp7::export(&plan.project))?;
    fs::write(plan.root.join("manifest.json"), manifest(original, plan))?;
    Ok(())
}

fn manifest(original: &Project, plan: &PackagePlan) -> String {
    let media: Vec<_> = plan
        .project
        .media
        .iter()
        .filter_map(|packaged| {
            let source = original.media(packaged.id)?;
            let relative = Path::new(&packaged.path)
                .strip_prefix(&plan.root)
                .ok()?
                .to_string_lossy()
                .into_owned();
            (source.path != packaged.path).then(|| {
                json!({
                    "id": packaged.id,
                    "name": packaged.name,
                    "packagePath": relative,
                    "originalPath": source.path,
                    "fingerprint": source.fingerprint,
                    "kind": source.kind,
                })
            })
        })
        .collect();
    let value = json!({
        "format": "studio.conform-package",
        "version": 1,
        "project": { "id": original.id, "name": original.name },
        "sequence": {
            "id": original.sequence.id,
            "name": original.sequence.name,
            "format": original.sequence.format,
            "durationFrames": original.duration(),
        },
        "files": {
            "otio": "timeline.otio",
            "fcp7Xml": "timeline.xml",
        },
        "media": media,
    });
    serde_json::to_string_pretty(&value).expect("json values always serialize")
}

/// The file name of `path`, suffixed ` (2)`, ` (3)`... until unused (case-insensitive,
/// because macOS volumes usually are).
fn unique_name(path: &str, taken: &mut BTreeSet<String>) -> String {
    let path = Path::new(path);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "media".into());
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let mut candidate = format!("{stem}{ext}");
    let mut n = 2;
    while !taken.insert(candidate.to_lowercase()) {
        candidate = format!("{stem} ({n}){ext}");
        n += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{media_with_av, sample_edit};

    #[test]
    fn copies_only_used_media_with_unique_names() {
        let mut project = sample_edit();
        let mut same_name = media_with_av(3);
        same_name.path = "/other/SHOT.mov".into();
        let unused = MediaId::new();
        project.media.push(crate::model::MediaAsset {
            id: unused,
            path: "/unused/clip.mov".into(),
            ..media_with_av(3)
        });
        project.media.push(same_name.clone());
        // Put the same-named media on the timeline too.
        let v1 = project.sequence.tracks[0].id;
        let cmd = crate::edit::EditCommand::InsertMedia {
            media_id: same_name.id,
            start: 200,
            source_start: 0,
            duration: Some(10),
            video_track: Some(v1),
            audio_track: None,
        };
        project = crate::edit::apply(&project, &cmd).unwrap().0;

        let plan = plan(&project, Path::new("/pkg"));
        let targets: Vec<_> = plan
            .copies
            .iter()
            .map(|c| c.to.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            targets,
            vec!["/pkg/media/shot.mov", "/pkg/media/SHOT (2).mov"]
        );
        assert_eq!(plan.project.media(unused).unwrap().path, "/unused/clip.mov");
        plan.project.validate().unwrap();
    }

    #[test]
    fn writes_a_complete_package() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("shot.mov");
        fs::write(&source, b"not really a movie").unwrap();
        let mut project = sample_edit();
        project.media[0].path = source.to_string_lossy().into_owned();

        let root = dir.path().join("Cut_Conform");
        let plan = plan(&project, &root);
        let mut reported = 0;
        write(&project, &plan, |bytes| reported = bytes).unwrap();

        assert_eq!(reported, 18);
        assert!(root.join("media/shot.mov").exists());
        let otio_text = fs::read_to_string(root.join("timeline.otio")).unwrap();
        assert!(otio_text.contains("Cut_Conform/media/shot.mov"));
        let manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["media"][0]["packagePath"], "media/shot.mov");
        assert_eq!(manifest["media"][0]["originalPath"], project.media[0].path);

        // A second export into the same folder is refused rather than mixing packages.
        assert!(write(&project, &plan, |_| {}).is_err());
    }
}
