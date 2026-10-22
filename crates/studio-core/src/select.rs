//! Naming parts of a document in text, so a caller that is not holding its ids can
//! still say what it means.
//!
//! Every id in the document is a uuid. That is right for the document and wrong for
//! anyone writing a command by hand or from a description: "the clip on V1 under twelve
//! seconds" is the thought, and `3f2a…` is not. These functions turn the thought into
//! ids, and nothing downstream changes, because what reaches [`crate::edit::apply`] and
//! the undo stack is always the id form.
//!
//! This is not a query language, and the limits are the point:
//!
//! - every form resolves to exactly one thing, or it is an error
//! - ambiguity is never guessed. Two tracks can end up sharing a name, so asking for
//!   that name is refused rather than answered with whichever came first
//! - nothing is relative. There is no "the next clip" or "+2 seconds", because relative
//!   needs an origin and a selector does not have one
//! - the playhead is not addressable. Where someone is looking is window state, not part
//!   of the document (ADR-0004), and it means nothing to a caller in another process. A
//!   caller that has a playhead passes the frame

use crate::ids::{ClipId, SequenceId, TrackId};
use crate::model::{Project, Sequence, TrackKind};
use crate::time::{parse_timecode, Frame};
use serde::Serialize;
use thiserror::Error;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, TS)]
#[serde(
    tag = "code",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum SelectError {
    #[error("{input} is not a way of naming a {what}: expected {expected}")]
    Malformed {
        what: &'static str,
        input: String,
        expected: &'static str,
    },
    #[error("no {what} called {input}")]
    NotFound { what: &'static str, input: String },
    #[error("{input} names {count} {what}s, so it is not clear which is meant")]
    Ambiguous {
        what: &'static str,
        input: String,
        count: usize,
    },
}

fn one<T>(what: &'static str, input: &str, mut found: Vec<T>) -> Result<T, SelectError> {
    match found.len() {
        0 => Err(SelectError::NotFound {
            what,
            input: input.to_string(),
        }),
        1 => Ok(found.remove(0)),
        count => Err(SelectError::Ambiguous {
            what,
            input: input.to_string(),
            count,
        }),
    }
}

/// A sequence, by id or by name.
pub fn sequence(project: &Project, input: &str) -> Result<SequenceId, SelectError> {
    let input = input.trim();
    if let Some(id) = SequenceId::parse(input) {
        return project
            .sequence(id)
            .map(|s| s.id)
            .ok_or_else(|| SelectError::NotFound {
                what: "sequence",
                input: input.to_string(),
            });
    }
    let matches: Vec<_> = project
        .sequences
        .iter()
        .filter(|s| s.name.eq_ignore_ascii_case(input))
        .map(|s| s.id)
        .collect();
    one("sequence", input, matches)
}

/// A track, by id, by name (`V1`, `a2`) or by kind and position (`video:1`, one based).
pub fn track(sequence: &Sequence, input: &str) -> Result<TrackId, SelectError> {
    let input = input.trim();
    if let Some(id) = TrackId::parse(input) {
        return sequence
            .tracks
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.id)
            .ok_or_else(|| SelectError::NotFound {
                what: "track",
                input: input.to_string(),
            });
    }
    if let Some((kind, index)) = input.split_once(':') {
        let kind = match kind.trim().to_ascii_lowercase().as_str() {
            "video" | "v" => TrackKind::Video,
            "audio" | "a" => TrackKind::Audio,
            _ => {
                return Err(SelectError::Malformed {
                    what: "track",
                    input: input.to_string(),
                    expected: "an id, a name such as V1, or video:1 / audio:1",
                })
            }
        };
        let index: usize = index.trim().parse().map_err(|_| SelectError::Malformed {
            what: "track",
            input: input.to_string(),
            expected: "a position counting from 1, as in video:1",
        })?;
        return sequence
            .tracks
            .iter()
            .filter(|t| t.kind == kind)
            .nth(index.checked_sub(1).ok_or(SelectError::Malformed {
                what: "track",
                input: input.to_string(),
                expected: "a position counting from 1, so video:0 is not one",
            })?)
            .map(|t| t.id)
            .ok_or_else(|| SelectError::NotFound {
                what: "track",
                input: input.to_string(),
            });
    }
    let matches: Vec<_> = sequence
        .tracks
        .iter()
        .filter(|t| t.name.eq_ignore_ascii_case(input))
        .map(|t| t.id)
        .collect();
    one("track", input, matches)
}

/// A frame, as a count, a timecode, or one of the two ends of the sequence.
///
/// `end` is what a caller means by "after everything", so appending needs no command of
/// its own: it is [`crate::edit::EditCommand::InsertMedia`] starting there.
pub fn time(sequence: &Sequence, input: &str) -> Result<Frame, SelectError> {
    let input = input.trim();
    match input.to_ascii_lowercase().as_str() {
        "start" => return Ok(0),
        "end" => return Ok(sequence.duration()),
        _ => {}
    }
    if let Ok(frame) = input.parse::<Frame>() {
        return Ok(frame);
    }
    parse_timecode(input, sequence.format.frame_rate).ok_or(SelectError::Malformed {
        what: "time",
        input: input.to_string(),
        expected: "a frame number, a timecode such as 00:00:12:00, or start / end",
    })
}

/// A clip, by id, by the track and frame it covers (`V1@00:00:12:00`), or by the track
/// and its position along it (`V1#3`, one based in the order the clips start).
pub fn clip(sequence: &Sequence, input: &str) -> Result<ClipId, SelectError> {
    let input = input.trim();
    if let Some(id) = ClipId::parse(input) {
        return sequence
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .find(|c| c.id == id)
            .map(|c| c.id)
            .ok_or_else(|| SelectError::NotFound {
                what: "clip",
                input: input.to_string(),
            });
    }

    let on_track = |name: &str| -> Result<&crate::model::Track, SelectError> {
        let id = track(sequence, name)?;
        sequence
            .tracks
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| SelectError::NotFound {
                what: "track",
                input: name.to_string(),
            })
    };

    if let Some((track_name, at)) = input.split_once('@') {
        let track = on_track(track_name)?;
        let frame = time(sequence, at)?;
        return track
            .clips
            .iter()
            .find(|c| c.contains(frame))
            .map(|c| c.id)
            .ok_or_else(|| SelectError::NotFound {
                what: "clip",
                input: input.to_string(),
            });
    }

    if let Some((track_name, nth)) = input.split_once('#') {
        let track = on_track(track_name)?;
        let nth: usize = nth.trim().parse().map_err(|_| SelectError::Malformed {
            what: "clip",
            input: input.to_string(),
            expected: "a position counting from 1, as in V1#3",
        })?;
        let mut clips: Vec<_> = track.clips.iter().collect();
        clips.sort_by_key(|c| c.start);
        return clips
            .get(nth.wrapping_sub(1))
            .map(|c| c.id)
            .ok_or_else(|| SelectError::NotFound {
                what: "clip",
                input: input.to_string(),
            });
    }

    Err(SelectError::Malformed {
        what: "clip",
        input: input.to_string(),
        expected: "an id, V1@00:00:12:00, or V1#3",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{apply, EditCommand};
    use crate::testing::sample_edit;

    fn sample() -> (Project, Sequence) {
        let project = sample_edit();
        let sequence = project.first_sequence().clone();
        (project, sequence)
    }

    #[test]
    fn a_track_answers_to_its_id_its_name_and_its_position() {
        let (_, sequence) = sample();
        let v1 = sequence.tracks[0].id;
        assert_eq!(track(&sequence, &v1.to_string()).unwrap(), v1);
        assert_eq!(track(&sequence, "V1").unwrap(), v1);
        assert_eq!(track(&sequence, "v1").unwrap(), v1, "case does not matter");
        assert_eq!(track(&sequence, "video:1").unwrap(), v1);
        assert_eq!(track(&sequence, "  V1  ").unwrap(), v1, "space is trimmed");
        assert_eq!(track(&sequence, "audio:1").unwrap(), sequence.tracks[1].id);
    }

    #[test]
    fn positions_count_from_one() {
        let (_, sequence) = sample();
        assert!(matches!(
            track(&sequence, "video:0"),
            Err(SelectError::Malformed { .. })
        ));
        assert!(matches!(
            track(&sequence, "video:2"),
            Err(SelectError::NotFound { .. })
        ));
    }

    #[test]
    fn a_name_two_tracks_share_is_refused_rather_than_guessed() {
        // Reachable: new tracks are named from how many of their kind there are, so
        // removing V1 and adding a track names the new one V2 beside the existing V2.
        let (project, sequence) = sample();
        let sequence_id = sequence.id;
        let v1 = sequence.tracks[0].id;
        let project = apply(
            &project,
            &EditCommand::AddTrack {
                sequence_id,
                kind: TrackKind::Video,
            },
        )
        .unwrap()
        .0;
        let project = apply(&project, &EditCommand::RemoveTrack { track_id: v1 })
            .unwrap()
            .0;
        let project = apply(
            &project,
            &EditCommand::AddTrack {
                sequence_id,
                kind: TrackKind::Video,
            },
        )
        .unwrap()
        .0;
        let sequence = project.first_sequence();
        let names: Vec<_> = sequence.tracks.iter().map(|t| t.name.as_str()).collect();
        assert!(
            names.iter().filter(|n| **n == "V2").count() == 2,
            "the duplicate this test is about did not happen: {names:?}"
        );
        assert!(matches!(
            track(sequence, "V2"),
            Err(SelectError::Ambiguous { count: 2, .. })
        ));
    }

    #[test]
    fn time_reads_frames_timecodes_and_the_two_ends() {
        let (_, sequence) = sample();
        assert_eq!(time(&sequence, "0").unwrap(), 0);
        assert_eq!(time(&sequence, "48").unwrap(), 48);
        assert_eq!(time(&sequence, "00:00:02:00").unwrap(), 48, "24 fps");
        assert_eq!(time(&sequence, "start").unwrap(), 0);
        assert_eq!(time(&sequence, "end").unwrap(), 124, "past the last clip");
        assert_eq!(time(&sequence, "END").unwrap(), 124);
    }

    #[test]
    fn a_time_that_is_neither_is_refused_with_the_forms_that_work() {
        let (_, sequence) = sample();
        let err = time(&sequence, "soon").unwrap_err();
        assert!(matches!(err, SelectError::Malformed { .. }));
        assert!(err.to_string().contains("00:00:12:00"), "{err}");
    }

    #[test]
    fn a_clip_answers_to_the_frame_it_covers() {
        let (_, sequence) = sample();
        let first = sequence.tracks[0].clips[0].id;
        // The clip runs 12..60.
        assert_eq!(clip(&sequence, "V1@12").unwrap(), first);
        assert_eq!(clip(&sequence, "V1@00:00:01:00").unwrap(), first);
        assert_eq!(clip(&sequence, "V1@59").unwrap(), first);
        assert!(
            matches!(clip(&sequence, "V1@60"), Err(SelectError::NotFound { .. })),
            "the end is past the clip"
        );
        assert!(matches!(
            clip(&sequence, "V1@80"),
            Err(SelectError::NotFound { .. })
        ));
    }

    #[test]
    fn a_clip_answers_to_its_position_along_the_track() {
        let (_, sequence) = sample();
        let ids: Vec<_> = sequence.tracks[0].clips.iter().map(|c| c.id).collect();
        assert_eq!(clip(&sequence, "V1#1").unwrap(), ids[0]);
        assert_eq!(clip(&sequence, "V1#2").unwrap(), ids[1]);
        assert!(matches!(
            clip(&sequence, "V1#3"),
            Err(SelectError::NotFound { .. })
        ));
        assert!(matches!(
            clip(&sequence, "V1#0"),
            Err(SelectError::NotFound { .. })
        ));
    }

    #[test]
    fn a_clip_named_in_no_known_way_says_which_ways_are_known() {
        let (_, sequence) = sample();
        let err = clip(&sequence, "the good one").unwrap_err();
        assert!(matches!(err, SelectError::Malformed { .. }));
        assert!(err.to_string().contains("V1#3"), "{err}");
    }

    #[test]
    fn a_sequence_answers_to_its_id_and_its_name() {
        let (project, sequence) = sample();
        assert_eq!(sequence_of(&project, &sequence.id.to_string()), sequence.id);
        assert_eq!(sequence_of(&project, &sequence.name), sequence.id);
    }

    fn sequence_of(project: &Project, input: &str) -> SequenceId {
        super::sequence(project, input).unwrap()
    }

    #[test]
    fn an_id_for_something_not_here_is_not_found_rather_than_malformed() {
        let (project, sequence) = sample();
        let stranger = TrackId::new().to_string();
        assert!(matches!(
            track(&sequence, &stranger),
            Err(SelectError::NotFound { .. })
        ));
        let stranger = SequenceId::new().to_string();
        assert!(matches!(
            super::sequence(&project, &stranger),
            Err(SelectError::NotFound { .. })
        ));
    }
}
