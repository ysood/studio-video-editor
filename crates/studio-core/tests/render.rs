//! End-to-end render through the real FFmpeg binary. Skips (passes) when `ffmpeg` or
//! `ffprobe` is not on PATH so the suite still runs on machines without them.

use std::path::Path;
use std::process::Command;
use studio_core::export::render::{plan, RenderSettings, VideoCodec};
use studio_core::*;

fn have(tool: &str) -> bool {
    Command::new(tool).arg("-version").output().is_ok()
}

fn run(tool: &str, args: &[&str]) -> String {
    let out = Command::new(tool).args(args).output().unwrap();
    assert!(
        out.status.success(),
        "{tool} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn probe(path: &Path) -> MediaAsset {
    let mut args: Vec<&str> = probe::FFPROBE_ARGS.to_vec();
    let p = path.to_str().unwrap();
    args.push(p);
    let json = run("ffprobe", &args);
    probe::parse(&json, path, probe::fingerprint(path).unwrap()).unwrap()
}

/// Mean luma of the frame at `seconds`, 0 to 255.
fn luma_at(path: &Path, seconds: f64) -> u8 {
    luma_of(path, seconds, "scale=1:1")
}

/// Mean luma of a 20x20 patch with its top left corner at `x`, `y`.
fn luma_patch(path: &Path, seconds: f64, x: u32, y: u32) -> u8 {
    luma_of(path, seconds, &format!("crop=20:20:{x}:{y},scale=1:1"))
}

fn luma_of(path: &Path, seconds: f64, filter: &str) -> u8 {
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-ss", &seconds.to_string(), "-i"])
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-vf",
            filter,
            "-f",
            "rawvideo",
            "-pix_fmt",
            "gray",
            "-",
        ])
        .output()
        .unwrap();
    out.stdout[0]
}

/// A flat colour source, `seconds` long, with no audio.
fn colour_source(dir: &Path, name: &str, colour: &str, seconds: u32) -> std::path::PathBuf {
    let path = dir.join(name);
    run(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("color=c={colour}:s=640x360:r=24:d={seconds}"),
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            path.to_str().unwrap(),
        ],
    );
    path
}

#[test]
fn renders_sequence_with_gaps_and_linked_audio() {
    if !have("ffmpeg") || !have("ffprobe") {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("white tone.mp4");
    run(
        "ffmpeg",
        &[
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=white:s=640x360:r=24:d=3",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=3",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            source.to_str().unwrap(),
        ],
    );

    let format = SequenceFormat {
        width: 320,
        height: 180,
        ..SequenceFormat::default()
    };
    let mut editor = Editor::new(Project::new("Render", format));
    let asset = probe(&source);
    let media_id = asset.id;
    editor
        .apply(&EditCommand::ImportMedia {
            assets: vec![asset],
        })
        .unwrap();
    let sequence_id = editor.project().first_sequence().id;
    let (v1, a1) = {
        let t = &editor.project().first_sequence().tracks;
        (t[0].id, t[1].id)
    };
    // White from 1 s to 2 s, black before and after.
    editor
        .apply(&EditCommand::InsertMedia {
            sequence_id,
            media_id,
            start: 24,
            source_start: 12,
            duration: Some(24),
            video_track: TrackChoice::Track(v1),
            audio_track: TrackChoice::Track(a1),
        })
        .unwrap();
    editor
        .apply(&EditCommand::InsertMedia {
            sequence_id,
            media_id,
            start: 60,
            source_start: 0,
            duration: Some(12),
            video_track: TrackChoice::Skip,
            audio_track: TrackChoice::Track(a1),
        })
        .unwrap();

    let output = dir.path().join("out.mp4");
    let render = plan(
        editor.project(),
        editor.project().first_sequence(),
        RenderSettings {
            codec: VideoCodec::H264,
        },
        &output,
    )
    .unwrap();
    let status = Command::new("ffmpeg").args(&render.args).output().unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );

    let rendered = probe(&output);
    let us = rendered.duration_us.unwrap();
    assert!((2_950_000..=3_100_000).contains(&us), "duration {us}");
    assert_eq!(rendered.video.as_ref().unwrap().width, 320);
    assert!(rendered.audio.is_some());
    assert!(luma_at(&output, 0.5) < 30, "gap should be black");
    assert!(luma_at(&output, 1.5) > 200, "clip should be white");
    assert!(luma_at(&output, 2.6) < 30, "after the clip should be black");
}

/// The point of compositing: a transformed clip on V2 lets V1 show where it does not
/// reach, instead of covering the frame with its own black padding.
///
/// One render proves both halves. The clip on V2 is black, scaled to half the frame and
/// half transparent, over a white clip on V1 that fills it. So the corner of the frame
/// has to be white (V1 showing through), and the middle has to be grey (V2 drawn, and
/// blended rather than opaque). Before the stack composited, the whole frame was black.
#[test]
fn a_transformed_clip_on_v2_reveals_v1_underneath() {
    if !have("ffmpeg") || !have("ffprobe") {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let white = colour_source(dir.path(), "white.mp4", "white", 2);
    let black = colour_source(dir.path(), "black.mp4", "black", 2);

    let format = SequenceFormat {
        width: 320,
        height: 180,
        ..SequenceFormat::default()
    };
    let mut editor = Editor::new(Project::new("Layers", format));
    let (under, over) = (probe(&white), probe(&black));
    let (under_id, over_id) = (under.id, over.id);
    editor
        .apply(&EditCommand::ImportMedia {
            assets: vec![under, over],
        })
        .unwrap();
    let sequence_id = editor.project().first_sequence().id;
    let v1 = editor.project().first_sequence().tracks[0].id;

    let v2 = editor
        .apply(&EditCommand::AddTrack {
            sequence_id,
            kind: TrackKind::Video,
        })
        .unwrap()
        .created_tracks[0];

    for (media_id, track) in [(under_id, v1), (over_id, v2)] {
        editor
            .apply(&EditCommand::InsertMedia {
                sequence_id,
                media_id,
                start: 0,
                source_start: 0,
                duration: Some(24),
                video_track: TrackChoice::Track(track),
                audio_track: TrackChoice::Skip,
            })
            .unwrap();
    }

    let top = editor.project().track(v2).unwrap().clips[0].id;
    editor
        .apply(&EditCommand::SetClipsTransform {
            clip_ids: vec![top],
            position: None,
            scale: Some(Scale { x: 500, y: 500 }),
            rotation: None,
            anchor: None,
            opacity: Some(500),
        })
        .unwrap();

    let output = dir.path().join("out.mp4");
    let render = plan(
        editor.project(),
        editor.project().first_sequence(),
        RenderSettings {
            codec: VideoCodec::H264,
        },
        &output,
    )
    .unwrap();
    let status = Command::new("ffmpeg").args(&render.args).output().unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );

    // Half of a 320x180 frame is 160x90, centred, so it covers x 80..240 and y 45..135.
    let corner = luma_patch(&output, 0.5, 4, 4);
    let middle = luma_patch(&output, 0.5, 150, 80);
    assert!(corner > 200, "V1 should show at the corner, got {corner}");
    assert!(
        (90..=180).contains(&middle),
        "V2 should be drawn and blended in the middle, got {middle}"
    );
}
