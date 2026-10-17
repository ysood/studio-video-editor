//! OpenTimelineIO JSON (`.otio`), the canonical interchange format.
//!
//! Written directly as JSON because the Rust OTIO bindings are immature. Uses the
//! `Timeline.1`, `Stack.1`, `Track.1`, `Clip.2`, `Gap.1` and `ExternalReference.1`
//! schemas. Every OTIO time uses the sequence rate, matching the document's single time
//! unit. Studio ids go in `metadata.studio` so a future importer can round-trip.

use super::{file_url, sequence_items, Item};
use crate::model::{Clip, Project, Track, TrackKind};
use crate::time::Frame;
use serde_json::{json, Value};

/// Serialize the project's sequence as an OTIO timeline.
pub fn export(project: &Project) -> String {
    let rate = project.sequence.format.frame_rate.as_f64();
    let tracks: Vec<Value> = ordered_tracks(project)
        .map(|track| track_json(project, track, rate))
        .collect();
    let timeline = json!({
        "OTIO_SCHEMA": "Timeline.1",
        "name": project.sequence.name,
        "global_start_time": null,
        "metadata": {
            "studio": {
                "projectId": project.id,
                "sequenceId": project.sequence.id,
                "projectName": project.name,
                "width": project.sequence.format.width,
                "height": project.sequence.format.height,
                "frameRate": project.sequence.format.frame_rate,
                "sampleRate": project.sequence.format.sample_rate,
            }
        },
        "tracks": {
            "OTIO_SCHEMA": "Stack.1",
            "name": "tracks",
            "source_range": null,
            "effects": [],
            "markers": [],
            "enabled": true,
            "metadata": {},
            "children": tracks,
        }
    });
    serde_json::to_string_pretty(&timeline).expect("json values always serialize")
}

/// OTIO stacks list children bottom to top; for audio the order has no visual meaning,
/// so tracks keep document order with video first.
fn ordered_tracks(project: &Project) -> impl Iterator<Item = &Track> {
    let tracks = &project.sequence.tracks;
    tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Video)
        .chain(tracks.iter().filter(|t| t.kind == TrackKind::Audio))
}

fn track_json(project: &Project, track: &Track, rate: f64) -> Value {
    let children: Vec<Value> = sequence_items(track)
        .into_iter()
        .map(|item| match item {
            Item::Gap(duration) => json!({
                "OTIO_SCHEMA": "Gap.1",
                "name": "",
                "source_range": range(0, duration, rate),
                "effects": [],
                "markers": [],
                "enabled": true,
                "metadata": {},
            }),
            Item::Clip(clip) => clip_json(project, clip, rate),
        })
        .collect();
    json!({
        "OTIO_SCHEMA": "Track.1",
        "name": track.name,
        "kind": match track.kind {
            TrackKind::Video => "Video",
            TrackKind::Audio => "Audio",
        },
        "source_range": null,
        "effects": [],
        "markers": [],
        "enabled": track.enabled,
        "metadata": { "studio": { "trackId": track.id, "locked": track.locked } },
        "children": children,
    })
}

fn clip_json(project: &Project, clip: &Clip, rate: f64) -> Value {
    let media = project
        .media(clip.source.media_id())
        .expect("validated project references existing media");
    let available = media
        .frames_at(project.sequence.format.frame_rate)
        .map_or(Value::Null, |total| range(0, total, rate));
    json!({
        "OTIO_SCHEMA": "Clip.2",
        "name": clip.name,
        "source_range": range(clip.source.source_start(), clip.duration, rate),
        "media_references": {
            "DEFAULT_MEDIA": {
                "OTIO_SCHEMA": "ExternalReference.1",
                "name": media.name,
                "target_url": file_url(&media.path, ""),
                "available_range": available,
                "available_image_bounds": null,
                "metadata": {},
            }
        },
        "active_media_reference_key": "DEFAULT_MEDIA",
        "effects": [],
        "markers": [],
        "enabled": clip.enabled,
        "metadata": {
            "studio": {
                "clipId": clip.id,
                "mediaId": media.id,
                "link": clip.link,
            }
        },
    })
}

fn range(start: Frame, duration: Frame, rate: f64) -> Value {
    json!({
        "OTIO_SCHEMA": "TimeRange.1",
        "start_time": time(start, rate),
        "duration": time(duration, rate),
    })
}

fn time(value: Frame, rate: f64) -> Value {
    json!({ "OTIO_SCHEMA": "RationalTime.1", "rate": rate, "value": value as f64 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::sample_edit;

    #[test]
    fn writes_gaps_between_clips_and_both_track_kinds() {
        let project = sample_edit();
        let value: Value = serde_json::from_str(&export(&project)).unwrap();
        let tracks = value["tracks"]["children"].as_array().unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0]["kind"], "Video");
        assert_eq!(tracks[1]["kind"], "Audio");
        let children = tracks[0]["children"].as_array().unwrap();
        let schemas: Vec<_> = children
            .iter()
            .map(|c| c["OTIO_SCHEMA"].as_str().unwrap())
            .collect();
        assert_eq!(schemas, vec!["Gap.1", "Clip.2", "Gap.1", "Clip.2"]);
        assert_eq!(children[0]["source_range"]["duration"]["value"], 12.0);
        assert_eq!(children[1]["source_range"]["start_time"]["value"], 6.0);
        assert_eq!(
            children[1]["media_references"]["DEFAULT_MEDIA"]["target_url"],
            "file:///footage/shot.mov"
        );
    }
}
