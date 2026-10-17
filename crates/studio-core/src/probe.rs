//! Turning ffprobe output into a [`MediaAsset`].
//!
//! ffprobe output is untrusted input: it describes arbitrary user files. It is parsed
//! into typed values with every field optional, and anything the editor cannot use is
//! rejected here rather than discovered later during playback or render.

use crate::ids::MediaId;
use crate::model::{AudioInfo, Fingerprint, MediaAsset, MediaKind, VideoInfo};
use crate::time::{Micros, Rational};
use serde::Deserialize;
use std::path::Path;
use std::time::UNIX_EPOCH;
use thiserror::Error;

/// Arguments for `ffprobe` that produce the JSON this module parses. The path is appended
/// as a separate argument by the caller; nothing goes through a shell.
pub const FFPROBE_ARGS: [&str; 6] = [
    "-v",
    "error",
    "-print_format",
    "json",
    "-show_format",
    "-show_streams",
];

#[derive(Debug, Error, PartialEq)]
pub enum ProbeError {
    #[error("ffprobe output could not be read: {0}")]
    Parse(String),
    #[error("file has no video or audio stream")]
    NoStreams,
    #[error("file has no usable duration")]
    NoDuration,
}

#[derive(Debug, Deserialize)]
struct Output {
    #[serde(default)]
    streams: Vec<Stream>,
    format: Option<Format>,
}

#[derive(Debug, Deserialize)]
struct Stream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
    duration: Option<String>,
    #[serde(default)]
    disposition: Disposition,
}

#[derive(Debug, Default, Deserialize)]
struct Disposition {
    #[serde(default)]
    attached_pic: u8,
}

#[derive(Debug, Deserialize)]
struct Format {
    format_name: Option<String>,
    duration: Option<String>,
}

/// Build a media asset from ffprobe JSON for the file at `path`.
pub fn parse(json: &str, path: &Path, fingerprint: Fingerprint) -> Result<MediaAsset, ProbeError> {
    let output: Output =
        serde_json::from_str(json).map_err(|e| ProbeError::Parse(e.to_string()))?;
    let format_name = output
        .format
        .as_ref()
        .and_then(|f| f.format_name.clone())
        .unwrap_or_default();

    // Cover art in audio files is a video stream marked attached_pic; it is not picture.
    let video_stream = output
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video") && s.disposition.attached_pic == 0);
    let audio_stream = output
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("audio"));

    let video = video_stream.and_then(video_info);
    let audio = audio_stream.and_then(audio_info);
    if video.is_none() && audio.is_none() {
        return Err(ProbeError::NoStreams);
    }

    let is_image = video.is_some()
        && audio.is_none()
        && (format_name.contains("image2") || format_name.ends_with("_pipe"));
    let kind = match (is_image, video.is_some()) {
        (true, _) => MediaKind::Image,
        (false, true) => MediaKind::Video,
        (false, false) => MediaKind::Audio,
    };

    let duration_us = if kind == MediaKind::Image {
        None
    } else {
        let format_duration = output.format.as_ref().and_then(|f| f.duration.as_deref());
        let stream_duration = video_stream
            .filter(|_| kind == MediaKind::Video)
            .or(audio_stream)
            .and_then(|s| s.duration.as_deref());
        let micros = format_duration
            .or(stream_duration)
            .and_then(parse_seconds)
            .filter(|us| *us > 0)
            .ok_or(ProbeError::NoDuration)?;
        Some(micros)
    };

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());

    Ok(MediaAsset {
        id: MediaId::new(),
        name,
        path: path.to_string_lossy().into_owned(),
        kind,
        duration_us,
        // Audio-only kinds must not carry video info (cover art was already dropped).
        video: if kind == MediaKind::Audio {
            None
        } else {
            video
        },
        audio,
        fingerprint,
    })
}

fn video_info(s: &Stream) -> Option<VideoInfo> {
    let width = s.width.filter(|w| *w > 0)?;
    let height = s.height.filter(|h| *h > 0)?;
    // avg_frame_rate is the real cadence for VFR phone footage; r_frame_rate is the
    // container's nominal rate and is the fallback.
    let frame_rate = s
        .avg_frame_rate
        .as_deref()
        .and_then(Rational::parse_ratio)
        .filter(|r| r.as_f64() <= 1000.0)
        .or_else(|| s.r_frame_rate.as_deref().and_then(Rational::parse_ratio))
        .unwrap_or(Rational::new(25, 1));
    Some(VideoInfo {
        width,
        height,
        frame_rate,
        codec: s.codec_name.clone().unwrap_or_default(),
    })
}

fn audio_info(s: &Stream) -> Option<AudioInfo> {
    let sample_rate = s.sample_rate.as_deref()?.parse().ok().filter(|r| *r > 0)?;
    Some(AudioInfo {
        sample_rate,
        channels: s.channels.unwrap_or(2).max(1),
        codec: s.codec_name.clone().unwrap_or_default(),
    })
}

/// ffprobe prints seconds as a decimal string such as `"3.003000"`.
fn parse_seconds(text: &str) -> Option<Micros> {
    let seconds: f64 = text.trim().parse().ok()?;
    if !seconds.is_finite() || !(0.0..=1e9).contains(&seconds) {
        return None;
    }
    Some((seconds * 1_000_000.0).round() as Micros)
}

/// Size and modification time, used to notice a file replaced after import.
pub fn fingerprint(path: &Path) -> std::io::Result<Fingerprint> {
    let meta = std::fs::metadata(path)?;
    let modified_unix_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .unwrap_or(0);
    Ok(Fingerprint {
        size_bytes: meta.len(),
        modified_unix_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = format!(
            "{}/tests/fixtures/ffprobe/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read_to_string(path).unwrap()
    }

    fn print() -> Fingerprint {
        Fingerprint {
            size_bytes: 10,
            modified_unix_ms: 1,
        }
    }

    #[test]
    fn video_with_audio() {
        let asset = parse(&fixture("av.mp4"), Path::new("/m/av.mp4"), print()).unwrap();
        assert_eq!(asset.kind, MediaKind::Video);
        assert_eq!(asset.name, "av.mp4");
        let video = asset.video.unwrap();
        assert_eq!((video.width, video.height), (1280, 720));
        assert_eq!(video.frame_rate, Rational::new(24000, 1001));
        assert_eq!(asset.audio.unwrap().sample_rate, 48_000);
        let us = asset.duration_us.unwrap();
        assert!((2_900_000..=3_100_000).contains(&us), "{us}");
    }

    #[test]
    fn audio_only() {
        let asset = parse(&fixture("tone.wav"), Path::new("/m/tone.wav"), print()).unwrap();
        assert_eq!(asset.kind, MediaKind::Audio);
        assert!(asset.video.is_none());
        assert_eq!(asset.audio.unwrap().channels, 2);
    }

    #[test]
    fn still_image_has_no_duration() {
        let asset = parse(&fixture("still.png"), Path::new("/m/still.png"), print()).unwrap();
        assert_eq!(asset.kind, MediaKind::Image);
        assert_eq!(asset.duration_us, None);
    }

    #[test]
    fn cover_art_is_not_video() {
        let asset = parse(&fixture("cover.mp3"), Path::new("/m/cover.mp3"), print()).unwrap();
        assert_eq!(asset.kind, MediaKind::Audio);
        assert!(asset.video.is_none());
    }

    #[test]
    fn rejects_streamless_and_garbage_output() {
        let empty = r#"{"streams": [], "format": {"format_name": "mov"}}"#;
        assert_eq!(
            parse(empty, Path::new("/x"), print()),
            Err(ProbeError::NoStreams)
        );
        assert!(matches!(
            parse("not json", Path::new("/x"), print()),
            Err(ProbeError::Parse(_))
        ));
    }

    #[test]
    fn rejects_timed_media_without_duration() {
        let json = r#"{"streams": [{"codec_type": "audio", "sample_rate": "48000"}],
                       "format": {"format_name": "wav"}}"#;
        assert_eq!(
            parse(json, Path::new("/x"), print()),
            Err(ProbeError::NoDuration)
        );
    }

    #[test]
    fn parsed_assets_validate_in_a_project() {
        let mut project = crate::testing::project_with(vec![]);
        for name in ["av.mp4", "tone.wav", "still.png", "cover.mp3"] {
            project
                .media
                .push(parse(&fixture(name), Path::new(name), print()).unwrap());
        }
        project.validate().unwrap();
    }
}
