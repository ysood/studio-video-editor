//! Final Cut Pro 7 XML (`xmeml` version 4), the format Premiere Pro imports most
//! reliably and Resolve also reads.
//!
//! Limits of the format that callers should know:
//! - `in` and `out` are written at the sequence rate. Footage whose rate differs from the
//!   sequence may need a conform in the receiving app.
//! - Each audio clip is one clipitem reading source channel 1; stereo files arrive as one
//!   stereo clip.

use super::{file_url, sequence_items, Item};
use crate::ids::{ClipId, MediaId};
use crate::model::{Clip, MediaAsset, MediaKind, Project, Track, TrackKind};
use crate::time::{timecode, Frame, Rational};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

pub fn export(project: &Project) -> String {
    Writer::new(project).write()
}

struct Writer<'a> {
    project: &'a Project,
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
    fn new(project: &'a Project) -> Self {
        let mut positions = HashMap::new();
        let mut item = 0;
        for kind in [TrackKind::Video, TrackKind::Audio] {
            for (track_index, track) in tracks_of(project, kind).enumerate() {
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
            out: String::new(),
            depth: 0,
            positions,
            files_written: HashSet::new(),
        }
    }

    fn write(mut self) -> String {
        let p = self.project;
        let format = p.sequence.format;
        self.out
            .push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE xmeml>\n");
        self.open("xmeml version=\"4\"");
        self.open("sequence id=\"sequence-1\"");
        self.text("uuid", &p.sequence.id.to_string());
        self.text("name", &p.sequence.name);
        self.text("duration", &p.duration().to_string());
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
        for track in tracks_of(p, TrackKind::Video) {
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
        for track in tracks_of(p, TrackKind::Audio) {
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
        let rate = p.sequence.format.frame_rate;
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

    /// Full file element on first use, then a reference by id, as Premiere expects.
    fn file(&mut self, media: &MediaAsset, frames: Frame) {
        let id = format!("file-{}", media.id);
        if !self.files_written.insert(media.id) {
            self.line(&format!("<file id=\"{id}\"/>"));
            return;
        }
        let rate = self.project.sequence.format.frame_rate;
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

fn tracks_of(project: &Project, kind: TrackKind) -> impl Iterator<Item = &Track> {
    project
        .sequence
        .tracks
        .iter()
        .filter(move |t| t.kind == kind)
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
    use crate::testing::sample_edit;

    #[test]
    fn writes_clipitems_with_timing_and_links() {
        let xml = export(&sample_edit());
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

    #[test]
    fn escapes_names() {
        let mut project = sample_edit();
        project.sequence.name = "Cut <A> & \"B\"".into();
        let xml = export(&project);
        assert!(xml.contains("<name>Cut &lt;A&gt; &amp; &quot;B&quot;</name>"));
    }
}
