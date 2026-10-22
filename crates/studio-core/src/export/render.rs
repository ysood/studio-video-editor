//! Turning a project into FFmpeg arguments that render the whole sequence.
//!
//! The plan is pure: it returns an argument vector and never spawns anything, so it is
//! fully testable. The caller runs `ffmpeg` with these arguments directly (no shell).
//!
//! Graph shape: a black canvas and a silent bed set the sequence length. Each enabled
//! video clip is its own seeked input, fitted to the frame, transformed, offset to its
//! timeline start and overlaid in track order (V1 first, so higher tracks sit over it).
//! Each enabled audio clip is delayed to its start and mixed at unity gain.
//!
//! The stack really composites: a clip is overlaid at its own size rather than padded
//! out to the frame, so wherever it does not reach, whatever is under it shows through.
//! That is what makes a scaled or faded clip on V2 reveal V1 instead of black.

use crate::model::{MediaKind, Project, Sequence, TrackKind, Transform, MILLI};
use crate::time::{Frame, Micros, Rational};
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::path::Path;
use thiserror::Error;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
                    let t = clip.transform;
                    // Fit to the frame first, exactly as before transforms existed, so an
                    // identity transform draws the picture it always drew. The transform
                    // then works in that fitted space, which makes it mean the same thing
                    // whatever the source resolution is.
                    let _ = write!(
                        graph,
                        "[{input}:v]fps={fps},scale={w}:{h}:force_original_aspect_ratio=decrease"
                    );
                    let (sx, sy) = t.scale_factor();
                    if sx != 1.0 || sy != 1.0 {
                        // Even and never zero: 4:2:0 chroma is sampled in pairs.
                        let _ = write!(
                            graph,
                            ",scale=w='max(2\\,trunc(iw*{sx:.6}/2)*2)':h='max(2\\,trunc(ih*{sy:.6}/2)*2)'"
                        );
                    }
                    let _ = write!(graph, ",setsar=1,format=yuva420p");
                    if t.rotation != 0 {
                        // rotw and roth grow the frame to hold the turned picture, and
                        // the corners it leaves behind stay transparent, so the layers
                        // under this one still show at them.
                        let rad = t.rotation_radians();
                        let _ = write!(
                            graph,
                            ",rotate={rad:.9}:ow=rotw({rad:.9}):oh=roth({rad:.9}):c=none"
                        );
                    }
                    if t.opacity != MILLI {
                        let _ = write!(graph, ",colorchannelmixer=aa={:.6}", t.opacity_factor());
                    }
                    let (dx, dy) = centre_offset(t);
                    // No pad: the picture is overlaid at its own size, so everywhere it
                    // does not cover, the layer underneath shows through.
                    let _ = write!(
                        graph,
                        ",setpts=PTS-STARTPTS+{offset_s}/TB[{fitted}];\
                         [{video_label}][{fitted}]overlay=x={x}:y={y}:eval=init:\
                         eof_action=pass:format=auto[{stacked}];",
                        x = placed("W", "w", dx),
                        y = placed("H", "h", dy),
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

/// How far the picture's centre sits from the middle of the frame, in frame pixels.
///
/// `position` places the clip's anchor, Premiere's convention, so moving the anchor
/// alone moves the picture. Scale and rotation turn about that anchor, but FFmpeg's
/// `rotate` turns about the picture's own centre, so the difference between the two is
/// worked out here and the overlay does the moving.
fn centre_offset(t: Transform) -> (f64, f64) {
    let (sx, sy) = t.scale_factor();
    let (ax, ay) = t.anchor_px();
    let (px, py) = t.position_px();
    // The anchor travels with the picture when it is scaled.
    let (gx, gy) = (ax * sx, ay * sy);
    // Clockwise, because y counts downwards here.
    let (sin, cos) = t.rotation_radians().sin_cos();
    (px - (gx * cos - gy * sin), py - (gx * sin + gy * cos))
}

/// An overlay coordinate: centred in the frame, then shifted by the transform.
///
/// The picture's size is left to FFmpeg (`w`, `h`) rather than worked out here, because
/// the real size after a camera's rotation metadata is only known once the file is
/// decoded, and the stored dimensions would fit a sideways clip wrong.
fn placed(frame: &str, picture: &str, delta: f64) -> String {
    if delta == 0.0 {
        format!("({frame}-{picture})/2")
    } else {
        format!("({frame}-{picture})/2{delta:+.3}")
    }
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
    use crate::edit::EditCommand;
    use crate::ids::ClipId;
    use crate::model::{Offset, Scale};
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

    /// The filter graph for `sample_edit` after transforming its first V1 clip. The
    /// command is built from the clip id, because ids are fresh on every fixture.
    fn with_transform(build: impl FnOnce(ClipId) -> EditCommand) -> String {
        let project = sample_edit();
        let command = build(project.first_sequence().tracks[0].clips[0].id);
        let project = crate::edit::apply(&project, &command).unwrap().0;
        let plan = plan(
            &project,
            project.first_sequence(),
            H264,
            Path::new("/out/cut.mp4"),
        )
        .unwrap();
        let i = plan
            .args
            .iter()
            .position(|a| a == "-filter_complex")
            .unwrap();
        plan.args[i + 1].clone()
    }

    /// A command that changes nothing, for checking the untransformed graph.
    fn untouched(clip: ClipId) -> EditCommand {
        EditCommand::SetClipsTransform {
            clip_ids: vec![clip],
            position: None,
            scale: None,
            rotation: None,
            anchor: None,
            opacity: None,
        }
    }

    #[test]
    fn an_untransformed_clip_runs_the_filters_it_always_did() {
        let graph = with_transform(untouched);
        assert!(!graph.contains("rotate="), "{graph}");
        assert!(!graph.contains("colorchannelmixer"), "{graph}");
        // One scale per video clip: the fit, and no transform pass on top of it.
        assert_eq!(graph.matches("scale=").count(), 2, "{graph}");
        assert!(graph.contains("overlay=x=(W-w)/2:y=(H-h)/2:"), "{graph}");
    }

    #[test]
    fn a_clip_is_overlaid_at_its_own_size_so_the_layer_under_it_shows() {
        // The old graph padded every clip out to the frame with opaque black, which made
        // the highest track cover everything below it. Nothing may pad any more.
        let graph = with_transform(untouched);
        assert!(!graph.contains("pad="), "{graph}");
    }

    #[test]
    fn scale_rotation_and_opacity_each_add_their_filter() {
        let graph = with_transform(|clip| EditCommand::SetClipsTransform {
            clip_ids: vec![clip],
            position: None,
            scale: Some(Scale { x: 500, y: 250 }),
            rotation: Some(30 * MILLI),
            anchor: None,
            opacity: Some(800),
        });
        assert!(graph.contains("trunc(iw*0.500000/2)*2"), "{graph}");
        assert!(graph.contains("trunc(ih*0.250000/2)*2"), "{graph}");
        assert!(graph.contains("rotate=0.523598776"), "{graph}");
        assert!(graph.contains("c=none"), "transparent corners: {graph}");
        assert!(graph.contains("colorchannelmixer=aa=0.800000"), "{graph}");
    }

    #[test]
    fn position_shifts_the_overlay_from_the_centre() {
        let graph = with_transform(|clip| EditCommand::SetClipsTransform {
            clip_ids: vec![clip],
            position: Some(Offset {
                x: 40 * MILLI,
                y: -10 * MILLI,
            }),
            scale: None,
            rotation: None,
            anchor: None,
            opacity: None,
        });
        assert!(
            graph.contains("overlay=x=(W-w)/2+40.000:y=(H-h)/2-10.000:"),
            "{graph}"
        );
    }

    #[test]
    fn rotation_turns_about_the_anchor_not_the_centre() {
        // An anchor 100 px right of centre, scaled by half, is 50 px out in the scaled
        // picture. Turning 30 degrees clockwise about it moves the picture's centre by
        // (50cos30, 50sin30) = (43.3, 25), and the overlay takes that back off.
        let graph = with_transform(|clip| EditCommand::SetClipsTransform {
            clip_ids: vec![clip],
            position: None,
            scale: Some(Scale { x: 500, y: 500 }),
            rotation: Some(30 * MILLI),
            anchor: Some(Offset {
                x: 100 * MILLI,
                y: 0,
            }),
            opacity: None,
        });
        assert!(
            graph.contains("overlay=x=(W-w)/2-43.301:y=(H-h)/2-25.000:"),
            "{graph}"
        );
    }

    #[test]
    fn an_anchor_alone_moves_the_picture() {
        // Premiere's convention: position places the anchor, so pulling the anchor to
        // the right slides the picture left to keep the anchor where position put it.
        let graph = with_transform(|clip| EditCommand::SetClipsTransform {
            clip_ids: vec![clip],
            position: None,
            scale: None,
            rotation: None,
            anchor: Some(Offset {
                x: 100 * MILLI,
                y: 0,
            }),
            opacity: None,
        });
        assert!(graph.contains("overlay=x=(W-w)/2-100.000:"), "{graph}");
    }

    #[test]
    fn parses_progress_lines() {
        assert_eq!(parse_progress("out_time_us=1500000"), Some(1_500_000));
        assert_eq!(parse_progress("out_time_us=N/A"), None);
        assert_eq!(parse_progress("frame=10"), None);
    }
}
