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
    let sequence_id = project.first_sequence().id;
    let (v1, a1) = (
        project.first_sequence().tracks[0].id,
        project.first_sequence().tracks[1].id,
    );
    for (start, source_start, duration) in [(24, 48, 96), (168, 0, 72)] {
        editor
            .apply(&EditCommand::InsertMedia {
                sequence_id,
                media_id,
                start,
                source_start,
                duration: Some(duration),
                video_track: TrackChoice::Track(v1),
                audio_track: TrackChoice::Track(a1),
            })
            .unwrap();
    }
    // One clip is transformed, so the sample files exercise the motion path and can be
    // opened in Premiere or Resolve to check the framing really survives.
    let framed = editor.project().first_sequence().tracks[0].clips[0].id;
    editor
        .apply(&EditCommand::SetClipsTransform {
            clip_ids: vec![framed],
            position: Some(Offset {
                x: 480 * MILLI,
                y: -270 * MILLI,
            }),
            scale: Some(Scale { x: 450, y: 450 }),
            rotation: Some(-8 * MILLI),
            anchor: None,
            opacity: Some(800),
        })
        .unwrap();
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
    let sequence = project.first_sequence();
    let otio_text = otio::export(&project, sequence);
    let xml_text = fcp7::export(&project, sequence);
    let dir = out_dir();
    std::fs::write(dir.join("sample.otio"), &otio_text).unwrap();
    std::fs::write(dir.join("sample.xml"), &xml_text).unwrap();

    let value: serde_json::Value = serde_json::from_str(&otio_text).unwrap();
    assert_eq!(value["OTIO_SCHEMA"], "Timeline.1");
    assert!(otio_text.contains("Shoot%20Day/A001%20shot%20%26%20take.mov"));
    assert!(xml_text.contains("<name>A001 shot &amp; take.mov</name>"));

    // The transform reaches both formats, each in its own units: fractions of the frame
    // for Basic Motion, the document's own thousandths for Studio's OTIO metadata.
    assert!(xml_text.contains("<name>Basic Motion</name>"), "{xml_text}");
    assert!(xml_text.contains("<horiz>0.25</horiz>"), "{xml_text}");
    assert!(xml_text.contains("<value>45</value>"), "{xml_text}");
    let clip = &value["tracks"]["children"][0]["children"][1];
    assert_eq!(
        clip["metadata"]["studio"]["transform"]["position"]["x"],
        480_000
    );
    assert_eq!(clip["metadata"]["studio"]["transform"]["opacity"], 800);
    // Only the picture: the linked sound carries nothing.
    assert_eq!(xml_text.matches("<filter>").count(), 2, "{xml_text}");
}
