//! Writes sample interchange files to `target/interchange-sample/` so they can be checked
//! with external tools (for example the OpenTimelineIO Python library and its FCP XML
//! adapter), and asserts the basics here.

use std::path::{Path, PathBuf};
use studio_core::export::{fcp7, otio};
use studio_core::*;

fn sample_project() -> Project {
    let mut project = Project::new("Interchange", SequenceFormat::default());
    let media = MediaAsset {
        id: MediaId::new(),
        name: "A001 shot & take.mov".into(),
        path: "/Volumes/Shoot Day/A001 shot & take.mov".into(),
        kind: MediaKind::Video,
        duration_us: Some(20_000_000),
        video: Some(VideoInfo {
            width: 3840,
            height: 2160,
            frame_rate: Rational::new(24, 1),
            codec: "prores".into(),
        }),
        audio: Some(AudioInfo {
            sample_rate: 48_000,
            channels: 2,
            codec: "pcm_s24le".into(),
        }),
        fingerprint: Fingerprint {
            size_bytes: 1,
            modified_unix_ms: 0,
        },
    };
    let media_id = media.id;
    let mut editor = Editor::new(project.clone());
    editor
        .apply(&EditCommand::ImportMedia {
            assets: vec![media],
        })
        .unwrap();
    project = editor.project().clone();
    let (v1, a1) = (project.sequence.tracks[0].id, project.sequence.tracks[1].id);
    for (start, source_start, duration) in [(24, 48, 96), (168, 0, 72)] {
        editor
            .apply(&EditCommand::InsertMedia {
                media_id,
                start,
                source_start,
                duration: Some(duration),
                video_track: Some(v1),
                audio_track: Some(a1),
            })
            .unwrap();
    }
    editor.project().clone()
}

fn out_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/interchange-sample");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn writes_sample_interchange_files() {
    let project = sample_project();
    let otio_text = otio::export(&project);
    let xml_text = fcp7::export(&project);
    let dir = out_dir();
    std::fs::write(dir.join("sample.otio"), &otio_text).unwrap();
    std::fs::write(dir.join("sample.xml"), &xml_text).unwrap();

    let value: serde_json::Value = serde_json::from_str(&otio_text).unwrap();
    assert_eq!(value["OTIO_SCHEMA"], "Timeline.1");
    assert!(otio_text.contains("Shoot%20Day/A001%20shot%20%26%20take.mov"));
    assert!(xml_text.contains("<name>A001 shot &amp; take.mov</name>"));
}
