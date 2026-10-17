//! Turning a project into FFmpeg arguments that render the whole sequence.
//!
//! The plan is pure: it returns an argument vector and never spawns anything, so it is
//! fully testable. The caller runs `ffmpeg` with these arguments directly (no shell).
//!
//! Graph shape: a black canvas and a silent bed set the sequence length. Each enabled
//! video clip is its own seeked input, fitted to the frame, offset to its timeline start
//! and overlaid in track order (V1 first, so higher tracks cover it). Each enabled audio
//! clip is delayed to its start and mixed at unity gain.

use crate::model::{MediaKind, Project, Sequence, TrackKind};
use crate::time::{Frame, Micros, Rational};
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::path::Path;
use thiserror::Error;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum VideoCodec {
    /// H.264 + AAC in MP4, for review and delivery.
    H264,
    /// ProRes 422 HQ + 24-bit PCM in MOV, an intermediate for finishing.
    ProRes422Hq,
}

impl VideoCodec {
    pub fn extension(self) -> &'static str {
        match self {
            VideoCodec::H264 => "mp4",
            VideoCodec::ProRes422Hq => "mov",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RenderSettings {
    pub codec: VideoCodec,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderPlan {
    pub args: Vec<String>,
    pub duration_frames: Frame,
    /// Total output length, for turning FFmpeg's `out_time_us` into a fraction.
    pub duration_us: Micros,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RenderError {
    #[error("the timeline is empty")]
    EmptyTimeline,
    #[error("media path is not absolute: {0}")]
    RelativePath(String),
}

pub fn plan(
    project: &Project,
    sequence: &Sequence,
    settings: RenderSettings,
    output: &Path,
) -> Result<RenderPlan, RenderError> {
    let format = sequence.format;
    let rate = format.frame_rate;
    let total = sequence.duration();
    if total == 0 {
        return Err(RenderError::EmptyTimeline);
    }
    if !output.is_absolute() {
        return Err(RenderError::RelativePath(output.display().to_string()));
    }
    let total_s = seconds(rate, total);
    let fps = format!("{}/{}", rate.num, rate.den);
    let (w, h, sr) = (format.width, format.height, format.sample_rate);

    let mut args: Vec<String> = ["-hide_banner", "-nostdin", "-v", "error", "-y"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    args.extend(strings(&["-progress", "pipe:1", "-nostats"]));
    args.extend(strings(&["-f", "lavfi", "-i"]));
    args.push(format!("color=c=black:s={w}x{h}:r={fps}:d={total_s}"));
    args.extend(strings(&["-f", "lavfi", "-t", &total_s, "-i"]));
    args.push(format!("anullsrc=r={sr}:cl=stereo"));

    let mut graph = String::new();
    let mut input = 2usize;
    let mut video_label = "0:v".to_string();
    let mut audio_labels = vec!["1:a".to_string()];

    for track in sequence.tracks.iter().filter(|t| t.enabled) {
        for clip in track.clips.iter().filter(|c| c.enabled) {
            let media = project
                .media(clip.source.media_id())
                .expect("validated project references existing media");
            if !media.path.starts_with('/') {
                return Err(RenderError::RelativePath(media.path.clone()));
            }
            let duration_s = seconds(rate, clip.duration);
            let offset_s = seconds(rate, clip.start);
            match track.kind {
                TrackKind::Video => {
                    if media.kind == MediaKind::Image {
                        args.extend(strings(&["-loop", "1", "-framerate", &fps]));
                    } else {
                        args.extend(strings(&[
                            "-ss",
                            &seconds(rate, clip.source.source_start()),
                        ]));
                    }
                    args.extend(strings(&["-t", &duration_s, "-i"]));
                    args.push(format!("file:{}", media.path));
                    let fitted = format!("v{input}");
                    let stacked = format!("s{input}");
                    let _ = write!(
                        graph,
                        "[{input}:v]fps={fps},scale={w}:{h}:force_original_aspect_ratio=decrease,\
                         pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1,format=yuva420p,\
                         setpts=PTS-STARTPTS+{offset_s}/TB[{fitted}];\
                         [{video_label}][{fitted}]overlay=eof_action=pass:format=auto[{stacked}];"
                    );
                    video_label = stacked;
                }
                TrackKind::Audio => {
                    args.extend(strings(&[
                        "-ss",
                        &seconds(rate, clip.source.source_start()),
                    ]));
                    args.extend(strings(&["-t", &duration_s, "-i"]));
                    args.push(format!("file:{}", media.path));
                    let delay = samples(rate, clip.start, sr);
                    let label = format!("a{input}");
                    let _ = write!(
                        graph,
                        "[{input}:a]aresample={sr},aformat=sample_fmts=fltp:channel_layouts=stereo,\
                         asetpts=PTS-STARTPTS,adelay={delay}S:all=1[{label}];"
                    );
                    audio_labels.push(label);
                }
            }
            input += 1;
        }
    }

    let pix_fmt = match settings.codec {
        VideoCodec::H264 => "yuv420p",
        VideoCodec::ProRes422Hq => "yuv422p10le",
    };
    let _ = write!(graph, "[{video_label}]format={pix_fmt}[vout];");
    let mix_inputs: String = audio_labels.iter().map(|l| format!("[{l}]")).collect();
    let _ = write!(
        graph,
        "{mix_inputs}amix=inputs={}:duration=first:dropout_transition=0:normalize=0[aout]",
        audio_labels.len()
    );

    args.extend(strings(&["-filter_complex", &graph]));
    args.extend(strings(&[
        "-map", "[vout]", "-map", "[aout]", "-r", &fps, "-t", &total_s,
    ]));
    match settings.codec {
        VideoCodec::H264 => args.extend(strings(&[
            "-c:v",
            "libx264",
            "-preset",
            "medium",
            "-crf",
            "18",
            "-c:a",
            "aac",
            "-b:a",
            "320k",
            "-movflags",
            "+faststart",
        ])),
        VideoCodec::ProRes422Hq => args.extend(strings(&[
            "-c:v",
            "prores_ks",
            "-profile:v",
            "3",
            "-vendor",
            "apl0",
            "-c:a",
            "pcm_s24le",
        ])),
    }
    args.push(format!("file:{}", output.display()));

    Ok(RenderPlan {
        args,
        duration_frames: total,
        duration_us: (rate.frames_to_seconds(total) * 1_000_000.0).round() as Micros,
    })
}

/// Parse one line of FFmpeg `-progress` output into microseconds written so far.
pub fn parse_progress(line: &str) -> Option<Micros> {
    let value = line.strip_prefix("out_time_us=")?;
    value.trim().parse().ok().filter(|us: &Micros| *us >= 0)
}

/// Seconds with microsecond precision. Floats only exist here, at the tool boundary.
fn seconds(rate: Rational, frames: Frame) -> String {
    format!("{:.6}", rate.frames_to_seconds(frames))
}

/// Exact sample offset for a frame, so audio lines up with video on any frame rate.
fn samples(rate: Rational, frames: Frame, sample_rate: u32) -> i64 {
    let value =
        i128::from(frames) * i128::from(rate.den) * i128::from(sample_rate) / i128::from(rate.num);
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::sample_edit;

    const H264: RenderSettings = RenderSettings {
        codec: VideoCodec::H264,
    };

    #[test]
    fn plans_one_input_per_enabled_clip() {
        let project = sample_edit();
        let plan = plan(
            &project,
            project.first_sequence(),
            H264,
            Path::new("/out/cut.mp4"),
        )
        .unwrap();
        // Two lavfi bases plus two video and two audio clip inputs.
        assert_eq!(plan.args.iter().filter(|a| *a == "-i").count(), 6);
        assert_eq!(plan.duration_frames, 124);
        assert_eq!(plan.args.last().unwrap(), "file:/out/cut.mp4");
        let graph = &plan.args[plan
            .args
            .iter()
            .position(|a| a == "-filter_complex")
            .unwrap()
            + 1];
        assert!(graph.contains("setpts=PTS-STARTPTS+0.500000/TB"), "{graph}");
        assert!(graph.contains("adelay=24000S:all=1"), "{graph}");
        assert!(graph.contains("amix=inputs=3"));
    }

    #[test]
    fn seeks_to_source_start() {
        let project = sample_edit();
        let plan = plan(
            &project,
            project.first_sequence(),
            H264,
            Path::new("/out/cut.mp4"),
        )
        .unwrap();
        let seeks: Vec<_> = plan
            .args
            .windows(2)
            .filter(|w| w[0] == "-ss")
            .map(|w| w[1].as_str())
            .collect();
        assert_eq!(seeks, vec!["0.250000", "0.000000", "0.250000", "0.000000"]);
    }

    #[test]
    fn disabled_tracks_are_left_out() {
        let mut project = sample_edit();
        project.first_sequence_mut().tracks[1].enabled = false;
        let plan = plan(
            &project,
            project.first_sequence(),
            H264,
            Path::new("/out/cut.mp4"),
        )
        .unwrap();
        assert_eq!(plan.args.iter().filter(|a| *a == "-i").count(), 4);
    }

    #[test]
    fn empty_timeline_is_an_error() {
        let project = crate::testing::project_with(vec![]);
        assert_eq!(
            plan(
                &project,
                project.first_sequence(),
                H264,
                Path::new("/out/x.mp4")
            ),
            Err(RenderError::EmptyTimeline)
        );
    }

    #[test]
    fn ntsc_sample_offsets_are_exact() {
        // 24 frames at 24000/1001 last exactly 1.001 s, which is 48_048 samples at 48 kHz.
        assert_eq!(samples(Rational::new(24000, 1001), 24, 48_000), 48_048);
    }

    #[test]
    fn parses_progress_lines() {
        assert_eq!(parse_progress("out_time_us=1500000"), Some(1_500_000));
        assert_eq!(parse_progress("out_time_us=N/A"), None);
        assert_eq!(parse_progress("frame=10"), None);
    }
}
