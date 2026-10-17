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
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-ss", &seconds.to_string(), "-i"])
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-vf",
            "scale=1:1",
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
    let (v1, a1) = {
        let t = &editor.project().sequence.tracks;
        (t[0].id, t[1].id)
    };
    // White from 1 s to 2 s, black before and after.
    editor
        .apply(&EditCommand::InsertMedia {
            media_id,
            start: 24,
            source_start: 12,
            duration: Some(24),
            video_track: Some(v1),
            audio_track: Some(a1),
        })
        .unwrap();
    editor
        .apply(&EditCommand::InsertMedia {
            media_id,
            start: 60,
            source_start: 0,
            duration: Some(12),
            video_track: None,
            audio_track: Some(a1),
        })
        .unwrap();

    let output = dir.path().join("out.mp4");
    let render = plan(
        editor.project(),
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
