//! OpenTimelineIO JSON (`.otio`), the canonical interchange format.
//!
//! Written directly as JSON because the Rust OTIO bindings are immature. Uses the
//! `Timeline.1`, `Stack.1`, `Track.1`, `Clip.2`, `Gap.1` and `ExternalReference.1`
//! schemas. Every OTIO time uses the sequence rate, matching the document's single time
//! unit. Studio ids go in `metadata.studio` so a future importer can round-trip, and a
//! clip's transform goes there too, for the reason `transform_json` gives.

use super::{file_url, sequence_items, Item};
use crate::model::{Clip, Project, Sequence, Track, TrackKind, Transform};
use crate::time::Frame;
use serde_json::{json, Value};

/// Serialize the project's sequence as an OTIO timeline.
pub fn export(project: &Project, sequence: &Sequence) -> String {
    let rate = sequence.format.frame_rate.as_f64();
    let tracks: Vec<Value> = ordered_tracks(sequence)
        .map(|track| track_json(project, sequence, track, rate))
        .collect();
    let timeline = json!({
        "OTIO_SCHEMA": "Timeline.1",
        "name": sequence.name,
        "global_start_time": null,
        "metadata": {
            "studio": {
                "projectId": project.id,
                "sequenceId": sequence.id,
                "projectName": project.name,
                "width": sequence.format.width,
                "height": sequence.format.height,
                "frameRate": sequence.format.frame_rate,
                "sampleRate": sequence.format.sample_rate,
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
fn ordered_tracks(sequence: &Sequence) -> impl Iterator<Item = &Track> {
    let tracks = &sequence.tracks;
    tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Video)
        .chain(tracks.iter().filter(|t| t.kind == TrackKind::Audio))
}

fn track_json(project: &Project, sequence: &Sequence, track: &Track, rate: f64) -> Value {
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
            Item::Clip(clip) => clip_json(project, sequence, clip, rate),
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

fn clip_json(project: &Project, sequence: &Sequence, clip: &Clip, rate: f64) -> Value {
    let media = project
        .media(clip.source.media_id())
        .expect("validated project references existing media");
    let available = media
        .frames_at(sequence.format.frame_rate)
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
        "metadata": { "studio": studio_metadata(clip, media.id) },
    })
}

fn studio_metadata(clip: &Clip, media_id: crate::ids::MediaId) -> Value {
    let mut studio = json!({
        "clipId": clip.id,
        "mediaId": media_id,
        "link": clip.link,
    });
    // Left out entirely when there is nothing to say, so a cut that never used a
    // transform exports exactly the file it always did.
    if !clip.transform.is_identity() {
        studio["transform"] = transform_json(clip.transform);
    }
    studio
}

/// The transform, in the document's own units, under Studio's namespace.
///
/// Not an OTIO effect. OTIO has no schema for an affine transform (OTIO issue 253), and
/// its maintainers' advice for anything the format does not model is to carry it as
/// metadata rather than invent an effect name no reader understands: a reader that does
/// not know Studio then ignores it instead of mis-drawing the clip. Emit a schema
/// instead if OTIO ever grows one.
///
/// The values are the document's own thousandths rather than the normalized floats the
/// FCP7 path writes, because nothing but Studio reads this namespace and an importer
/// should get the document back exactly, which is what the rest of `metadata.studio` is
/// for. They are already resolution independent in the sense that matters: positions are
/// offsets from the centre of the frame and scale is a factor of the fitted picture, so
/// relinking to media of another resolution does not move anything.
fn transform_json(transform: Transform) -> Value {
    serde_json::to_value(transform).expect("a transform serializes")
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
        let value: Value =
            serde_json::from_str(&export(&project, project.first_sequence())).unwrap();
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

    /// `sample_edit` after transforming its first V1 clip, parsed back.
    fn with_transform(build: impl FnOnce(crate::ids::ClipId) -> crate::edit::EditCommand) -> Value {
        let project = sample_edit();
        let command = build(project.first_sequence().tracks[0].clips[0].id);
        let project = crate::edit::apply(&project, &command).unwrap().0;
        serde_json::from_str(&export(&project, project.first_sequence())).unwrap()
    }

    #[test]
    fn an_untransformed_clip_carries_no_transform_key() {
        // A project that never used a transform exports the file it always did.
        let project = sample_edit();
        let text = export(&project, project.first_sequence());
        assert!(!text.contains("transform"), "{text}");
    }

    #[test]
    fn a_transform_rides_in_studios_metadata_in_the_documents_own_units() {
        use crate::model::{Offset, Scale, MILLI};
        let value = with_transform(|clip| crate::edit::EditCommand::SetClipsTransform {
            clip_ids: vec![clip],
            position: Some(Offset {
                x: 480 * MILLI,
                y: -270 * MILLI,
            }),
            scale: Some(Scale { x: 450, y: 450 }),
            rotation: Some(-8 * MILLI),
            anchor: None,
            opacity: Some(800),
        });
        let video = &value["tracks"]["children"][0]["children"][1];
        let transform = &video["metadata"]["studio"]["transform"];
        // Thousandths, exactly as the document holds them, so an importer round-trips.
        assert_eq!(transform["position"]["x"], 480_000);
        assert_eq!(transform["position"]["y"], -270_000);
        assert_eq!(transform["scale"]["x"], 450);
        assert_eq!(transform["rotation"], -8_000);
        assert_eq!(transform["opacity"], 800);
        // Not an effect: OTIO has no schema for one, and a reader that does not know
        // Studio has to ignore this rather than mis-draw the clip.
        assert_eq!(video["effects"], json!([]));
    }

    #[test]
    fn the_sound_half_of_a_pair_carries_nothing() {
        use crate::model::{Scale, MILLI};
        let value = with_transform(|clip| crate::edit::EditCommand::SetClipsTransform {
            clip_ids: vec![clip],
            position: None,
            scale: Some(Scale {
                x: MILLI / 2,
                y: MILLI / 2,
            }),
            rotation: None,
            anchor: None,
            opacity: None,
        });
        let audio = &value["tracks"]["children"][1]["children"][1];
        assert!(audio["metadata"]["studio"]["transform"].is_null());
    }
}
