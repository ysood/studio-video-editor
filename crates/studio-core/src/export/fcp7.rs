//! Final Cut Pro 7 XML (`xmeml` version 4), the format Premiere Pro imports most
//! reliably and Resolve also reads.
//!
//! Limits of the format that callers should know:
//! - `in` and `out` are written at the sequence rate. Footage whose rate differs from the
//!   sequence may need a conform in the receiving app.
//! - Each audio clip is one clipitem reading source channel 1; stereo files arrive as one
//!   stereo clip.
//! - Basic Motion carries a single Scale, so a clip scaled differently across and down
//!   travels as its width scale. Nothing in the format holds the other one.

use super::{file_url, sequence_items, Item};
use crate::ids::{ClipId, MediaId};
use crate::model::{
    Clip, MediaAsset, MediaKind, Project, Sequence, SequenceFormat, Track, TrackKind, Transform,
    MILLI,
};
use crate::time::{timecode, Frame, Rational};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

pub fn export(project: &Project, sequence: &Sequence) -> String {
    Writer::new(project, sequence).write()
}

struct Writer<'a> {
    project: &'a Project,
    sequence: &'a Sequence,
    out: String,
    depth: usize,
    /// Clip id to its clipitem id, track index (1-based within kind) and clip index.
    positions: HashMap<ClipId, Position>,
    files_written: HashSet<MediaId>,
}

#[derive(Clone, Copy)]
struct Position {
    item: usize,
    kind: TrackKind,
    track: usize,
    clip: usize,
}

impl<'a> Writer<'a> {
    fn new(project: &'a Project, sequence: &'a Sequence) -> Self {
        let mut positions = HashMap::new();
        let mut item = 0;
        for kind in [TrackKind::Video, TrackKind::Audio] {
            for (track_index, track) in tracks_of(sequence, kind).enumerate() {
                for (clip_index, clip) in track.clips.iter().enumerate() {
                    item += 1;
                    positions.insert(
                        clip.id,
                        Position {
                            item,
                            kind,
                            track: track_index + 1,
                            clip: clip_index + 1,
                        },
                    );
                }
            }
        }
        Self {
            project,
            sequence,
            out: String::new(),
            depth: 0,
            positions,
            files_written: HashSet::new(),
        }
    }

    fn write(mut self) -> String {
        let sequence = self.sequence;
        let format = sequence.format;
        self.out
            .push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE xmeml>\n");
        self.open("xmeml version=\"4\"");
        self.open("sequence id=\"sequence-1\"");
        self.text("uuid", &sequence.id.to_string());
        self.text("name", &sequence.name);
        self.text("duration", &sequence.duration().to_string());
        self.rate(format.frame_rate);
        self.open("timecode");
        self.rate(format.frame_rate);
        self.text("string", &timecode(0, format.frame_rate));
        self.text("frame", "0");
        self.text("displayformat", "NDF");
        self.close("timecode");
        self.open("media");

        self.open("video");
        self.open("format");
        self.open("samplecharacteristics");
        self.rate(format.frame_rate);
        self.text("width", &format.width.to_string());
        self.text("height", &format.height.to_string());
        self.text("pixelaspectratio", "square");
        self.text("fielddominance", "none");
        self.close("samplecharacteristics");
        self.close("format");
        for track in tracks_of(sequence, TrackKind::Video) {
            self.track(track);
        }
        self.close("video");

        self.open("audio");
        self.open("format");
        self.open("samplecharacteristics");
        self.text("depth", "16");
        self.text("samplerate", &format.sample_rate.to_string());
        self.close("samplecharacteristics");
        self.close("format");
        for track in tracks_of(sequence, TrackKind::Audio) {
            self.track(track);
        }
        self.close("audio");

        self.close("media");
        self.close("sequence");
        self.close("xmeml");
        self.out
    }

    fn track(&mut self, track: &Track) {
        self.open("track");
        self.text("enabled", bool_text(track.enabled));
        self.text("locked", bool_text(track.locked));
        for item in sequence_items(track) {
            if let Item::Clip(clip) = item {
                self.clip(track.kind, clip);
            }
        }
        self.close("track");
    }

    fn clip(&mut self, kind: TrackKind, clip: &Clip) {
        let p = self.project;
        let rate = self.sequence.format.frame_rate;
        let media = p
            .media(clip.source.media_id())
            .expect("validated project references existing media");
        let position = self.positions[&clip.id];
        let source_in = clip.source.source_start();
        let media_frames = media_duration(media, rate, source_in + clip.duration);

        self.open(&format!("clipitem id=\"clipitem-{}\"", position.item));
        self.text("name", &clip.name);
        self.text("enabled", bool_text(clip.enabled));
        self.text("duration", &media_frames.to_string());
        self.rate(rate);
        self.text("start", &clip.start.to_string());
        self.text("end", &clip.end().to_string());
        self.text("in", &source_in.to_string());
        self.text("out", &(source_in + clip.duration).to_string());
        self.file(media, media_frames);
        self.open("sourcetrack");
        match kind {
            TrackKind::Video => self.text("mediatype", "video"),
            TrackKind::Audio => {
                self.text("mediatype", "audio");
                self.text("trackindex", "1");
            }
        }
        self.close("sourcetrack");
        if kind == TrackKind::Video {
            self.motion(clip.transform);
        }
        if clip.link.is_some() {
            for member in p.link_group(clip.id) {
                let pos = self.positions[&member];
                self.open("link");
                self.text("linkclipref", &format!("clipitem-{}", pos.item));
                self.text(
                    "mediatype",
                    match pos.kind {
                        TrackKind::Video => "video",
                        TrackKind::Audio => "audio",
                    },
                );
                self.text("trackindex", &pos.track.to_string());
                self.text("clipindex", &pos.clip.to_string());
                self.close("link");
            }
        }
        self.close("clipitem");
    }

    /// The clip's transform, as the Motion settings Premiere and Resolve read.
    ///
    /// Written only when there is something to say: an untransformed clip produces no
    /// filter at all, so a cut that never used a transform exports exactly the file it
    /// always did. The two effects are independent, so a clip that is only faded gets
    /// Opacity and no Basic Motion.
    fn motion(&mut self, transform: Transform) {
        let format = self.sequence.format;
        let motion = BasicMotion::of(transform, format);
        if !motion.is_identity() {
            self.open("filter");
            self.open("effect");
            self.text("name", "Basic Motion");
            self.text("effectid", "basic");
            self.text("effectcategory", "motion");
            self.text("effecttype", "motion");
            self.text("mediatype", "video");
            self.parameter("scale", "Scale", Some((0.0, 1000.0)), motion.scale);
            self.parameter(
                "rotation",
                "Rotation",
                Some((-8640.0, 8640.0)),
                motion.rotation,
            );
            self.point("center", "Center", motion.center);
            self.point("centerOffset", "Anchor Point", motion.anchor);
            self.close("effect");
            self.close("filter");
        }
        if transform.opacity != MILLI {
            self.open("filter");
            self.open("effect");
            self.text("name", "Opacity");
            self.text("effectid", "opacity");
            self.text("effectcategory", "motion");
            self.text("effecttype", "motion");
            self.text("mediatype", "video");
            self.parameter(
                "opacity",
                "opacity",
                Some((0.0, 100.0)),
                transform.opacity_factor() * 100.0,
            );
            self.close("effect");
            self.close("filter");
        }
    }

    fn parameter(&mut self, id: &str, name: &str, range: Option<(f64, f64)>, value: f64) {
        self.open("parameter");
        self.text("parameterid", id);
        self.text("name", name);
        if let Some((min, max)) = range {
            self.text("valuemin", &number(min));
            self.text("valuemax", &number(max));
        }
        self.text("value", &number(value));
        self.close("parameter");
    }

    /// A parameter whose value is a point rather than a number.
    fn point(&mut self, id: &str, name: &str, (x, y): (f64, f64)) {
        self.open("parameter");
        self.text("parameterid", id);
        self.text("name", name);
        self.open("value");
        self.text("horiz", &number(x));
        self.text("vert", &number(y));
        self.close("value");
        self.close("parameter");
    }

    /// Full file element on first use, then a reference by id, as Premiere expects.
    fn file(&mut self, media: &MediaAsset, frames: Frame) {
        let id = format!("file-{}", media.id);
        if !self.files_written.insert(media.id) {
            self.line(&format!("<file id=\"{id}\"/>"));
            return;
        }
        let rate = self.sequence.format.frame_rate;
        self.open(&format!("file id=\"{id}\""));
        self.text("name", &media.name);
        self.text("pathurl", &file_url(&media.path, "localhost"));
        self.rate(rate);
        self.text("duration", &frames.to_string());
        self.open("media");
        if let Some(video) = &media.video {
            self.open("video");
            self.open("samplecharacteristics");
            self.text("width", &video.width.to_string());
            self.text("height", &video.height.to_string());
            self.close("samplecharacteristics");
            self.close("video");
        }
        if let Some(audio) = &media.audio {
            self.open("audio");
            self.open("samplecharacteristics");
            self.text("depth", "16");
            self.text("samplerate", &audio.sample_rate.to_string());
            self.close("samplecharacteristics");
            self.text("channelcount", &audio.channels.to_string());
            self.close("audio");
        }
        self.close("media");
        self.close("file");
    }

    fn rate(&mut self, rate: Rational) {
        self.open("rate");
        self.text("timebase", &rate.timebase().to_string());
        self.text("ntsc", bool_text(rate.is_ntsc()));
        self.close("rate");
    }

    fn open(&mut self, tag: &str) {
        self.line(&format!("<{tag}>"));
        self.depth += 1;
    }

    fn close(&mut self, tag: &str) {
        self.depth -= 1;
        self.line(&format!("</{tag}>"));
    }

    fn text(&mut self, tag: &str, value: &str) {
        let line = format!("<{tag}>{}</{tag}>", escape(value));
        self.line(&line);
    }

    fn line(&mut self, text: &str) {
        let _ = writeln!(self.out, "{}{text}", "  ".repeat(self.depth));
    }
}

/// Studio's transform in the units Basic Motion uses.
///
/// `center` and `centerOffset` are fractions of the frame measured from its middle, so
/// half a frame to the left is -0.5 and the top left corner is (-0.5, -0.5). That is why
/// the format stores them this way rather than in pixels: a clip keeps its framing when
/// the media behind it is relinked at another resolution, which is the reason Apple's
/// reference gives for the Center parameter being resolution independent. Studio's own
/// offsets are already measured from the middle of the frame, so this is a divide and
/// nothing more.
///
/// Scale is a percentage where 100 leaves the picture alone, and rotation is clockwise
/// degrees, the same direction Studio counts.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BasicMotion {
    scale: f64,
    rotation: f64,
    center: (f64, f64),
    anchor: (f64, f64),
}

impl BasicMotion {
    fn of(t: Transform, format: SequenceFormat) -> Self {
        let (w, h) = (f64::from(format.width), f64::from(format.height));
        let (px, py) = t.position_px();
        let (ax, ay) = t.anchor_px();
        // Basic Motion has one Scale. A clip scaled differently across and down travels
        // as its width scale, and the other one is lost: see the module header.
        let (sx, _) = t.scale_factor();
        Self {
            scale: sx * 100.0,
            rotation: t.rotation_degrees(),
            center: (px / w, py / h),
            anchor: (ax / w, ay / h),
        }
    }

    /// Whether this says anything a receiving app would not already assume.
    fn is_identity(self) -> bool {
        self.scale == 100.0
            && self.rotation == 0.0
            && self.center == (0.0, 0.0)
            && self.anchor == (0.0, 0.0)
    }
}

/// A number with no trailing zeros, so a whole value reads as `100` rather than
/// `100.000000`, which is how Premiere writes its own.
fn number(value: f64) -> String {
    let text = format!("{value:.6}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    match trimmed {
        // Negative zero, and anything that rounds away to nothing, both read as plain
        // zero rather than the `-0` a receiving app would have to interpret.
        "" | "-" | "-0" | "0" => "0".to_string(),
        other => other.to_string(),
    }
}

fn tracks_of(sequence: &Sequence, kind: TrackKind) -> impl Iterator<Item = &Track> {
    sequence.tracks.iter().filter(move |t| t.kind == kind)
}

/// Media length in sequence frames. Stills have none, so use what the clip needs.
fn media_duration(media: &MediaAsset, rate: Rational, needed: Frame) -> Frame {
    match media.kind {
        MediaKind::Image => needed,
        _ => media.frames_at(rate).unwrap_or(needed),
    }
}

fn bool_text(value: bool) -> &'static str {
    if value {
        "TRUE"
    } else {
        "FALSE"
    }
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // XML 1.0 forbids most control characters even when escaped.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::EditCommand;
    use crate::ids::ClipId;
    use crate::model::{Offset, Scale};
    use crate::testing::sample_edit;

    #[test]
    fn writes_clipitems_with_timing_and_links() {
        let project = sample_edit();
        let xml = export(&project, project.first_sequence());
        assert!(xml.contains("<xmeml version=\"4\">"));
        assert_eq!(xml.matches("<clipitem ").count(), 4);
        assert!(xml.contains("<start>12</start>"));
        assert!(xml.contains("<end>60</end>"));
        assert!(xml.contains("<in>6</in>"));
        assert!(xml.contains("<out>54</out>"));
        assert!(xml.contains("<pathurl>file://localhost/footage/shot.mov</pathurl>"));
        // The full file element appears once; later clips reference it.
        assert_eq!(xml.matches("<pathurl>").count(), 1);
        assert_eq!(xml.matches("<link>").count(), 8);
    }

    /// `sample_edit` after transforming its first V1 clip; ids are fresh per fixture.
    fn with_transform(build: impl FnOnce(ClipId) -> EditCommand) -> String {
        let project = sample_edit();
        let command = build(project.first_sequence().tracks[0].clips[0].id);
        let project = crate::edit::apply(&project, &command).unwrap().0;
        export(&project, project.first_sequence())
    }

    fn change(clip: ClipId) -> EditCommand {
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
    fn an_untransformed_cut_writes_no_filter_at_all() {
        // A project that never used a transform exports the file it always did.
        let project = sample_edit();
        let xml = export(&project, project.first_sequence());
        assert!(!xml.contains("<filter>"), "{xml}");
        assert!(!xml.contains("Basic Motion"), "{xml}");
    }

    #[test]
    fn writes_basic_motion_as_fractions_of_the_frame() {
        // 480 px across a 1920 frame is a quarter of it, and 270 px up a 1080 frame is a
        // quarter of that. Negative vert is up, so the top left corner is (-0.5, -0.5).
        let xml = with_transform(|clip| EditCommand::SetClipsTransform {
            clip_ids: vec![clip],
            position: Some(Offset {
                x: 480 * MILLI,
                y: -270 * MILLI,
            }),
            scale: Some(Scale { x: 450, y: 450 }),
            rotation: Some(-8 * MILLI),
            anchor: Some(Offset {
                x: 96 * MILLI,
                y: 0,
            }),
            opacity: None,
        });
        assert!(xml.contains("<name>Basic Motion</name>"), "{xml}");
        assert!(xml.contains("<effectid>basic</effectid>"), "{xml}");
        assert!(xml.contains("<horiz>0.25</horiz>"), "{xml}");
        assert!(xml.contains("<vert>-0.25</vert>"), "{xml}");
        assert!(xml.contains("<horiz>0.05</horiz>"), "anchor: {xml}");
        // Scale is a percentage, rotation clockwise degrees.
        assert!(xml.contains("<value>45</value>"), "{xml}");
        assert!(xml.contains("<value>-8</value>"), "{xml}");
        // Nothing was faded, so no Opacity effect.
        assert!(!xml.contains("<name>Opacity</name>"), "{xml}");
    }

    #[test]
    fn a_fade_writes_opacity_and_nothing_else() {
        // The two effects are independent: a clip that is only faded has no motion.
        let xml = with_transform(|clip| EditCommand::SetClipsTransform {
            clip_ids: vec![clip],
            position: None,
            scale: None,
            rotation: None,
            anchor: None,
            opacity: Some(MILLI / 4),
        });
        assert!(!xml.contains("Basic Motion"), "{xml}");
        assert!(xml.contains("<effectid>opacity</effectid>"), "{xml}");
        assert!(xml.contains("<value>25</value>"), "{xml}");
    }

    #[test]
    fn only_the_video_clip_gets_a_filter() {
        // sample_edit links video and audio; the sound has no picture to move.
        let xml = with_transform(|clip| EditCommand::SetClipsTransform {
            clip_ids: vec![clip],
            position: None,
            scale: Some(Scale { x: 500, y: 500 }),
            rotation: None,
            anchor: None,
            opacity: Some(500),
        });
        assert_eq!(xml.matches("<filter>").count(), 2, "{xml}");
    }

    #[test]
    fn a_clip_scaled_differently_across_and_down_travels_as_its_width() {
        // Basic Motion has one Scale, so the other is lost. Documented in the header.
        let xml = with_transform(|clip| EditCommand::SetClipsTransform {
            clip_ids: vec![clip],
            position: None,
            scale: Some(Scale { x: 500, y: 250 }),
            rotation: None,
            anchor: None,
            opacity: None,
        });
        assert!(xml.contains("<value>50</value>"), "{xml}");
        assert!(!xml.contains("<value>25</value>"), "{xml}");
    }

    #[test]
    fn a_transform_that_says_nothing_writes_nothing() {
        let xml = with_transform(change);
        assert!(!xml.contains("<filter>"), "{xml}");
    }

    #[test]
    fn whole_numbers_lose_their_trailing_zeros() {
        assert_eq!(number(100.0), "100");
        assert_eq!(number(-8.0), "-8");
        assert_eq!(number(0.456250), "0.45625");
        assert_eq!(number(-0.0), "0");
    }

    #[test]
    fn escapes_names() {
        let mut project = sample_edit();
        project.first_sequence_mut().name = "Cut <A> & \"B\"".into();
        let xml = export(&project, project.first_sequence());
        assert!(xml.contains("<name>Cut &lt;A&gt; &amp; &quot;B&quot;</name>"));
    }
}
