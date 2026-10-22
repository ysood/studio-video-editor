//! Questions about a project, answered without handing the whole document over.
//!
//! A caller that can only fetch the entire project has to re-derive everything it wants
//! to know, and every such caller derives it slightly differently. These functions are
//! the one answer, so the shell, a second window and an agent all read the same thing.
//!
//! Two rules hold throughout.
//!
//! Every frame carries its timecode beside it. A caller that knows a clip starts at
//! frame 288 still has to find the sequence rate and do the arithmetic to say where that
//! is, and the rate is not always 24. Answering both at once costs a format call and
//! removes a whole class of wrong answers.
//!
//! Nothing here touches the filesystem. Whether media is offline is stated by the
//! caller, because only the shell knows, and keeping it out leaves these functions pure
//! and testable.

use crate::ids::{ClipId, MediaId, SequenceId, TrackId};
use crate::model::{Clip, Project, Sequence, SequenceFormat, Track, TrackKind};
use crate::time::{timecode, Frame};
use serde::Serialize;
use ts_rs::TS;

/// One sequence, without its contents.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SequenceSummary {
    pub id: SequenceId,
    pub name: String,
    pub format: SequenceFormat,
    /// One past the last frame of the last clip, so an empty sequence is zero.
    #[ts(type = "number")]
    pub duration: Frame,
    pub duration_timecode: String,
    pub video_tracks: usize,
    pub audio_tracks: usize,
    pub clip_count: usize,
}

/// The project, and enough of each sequence to choose one.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectOverview {
    pub name: String,
    pub media_count: usize,
    pub sequences: Vec<SequenceSummary>,
}

/// One clip, flattened: the things asked about a clip, without walking to its track and
/// its media to find them.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClipSummary {
    pub id: ClipId,
    pub name: String,
    pub track_id: TrackId,
    #[ts(type = "number")]
    pub start: Frame,
    pub start_timecode: String,
    #[ts(type = "number")]
    pub duration: Frame,
    /// Frame after the clip's last, which is where the next clip may begin.
    #[ts(type = "number")]
    pub end: Frame,
    pub end_timecode: String,
    pub enabled: bool,
    pub media_id: MediaId,
    /// Carried so that naming a clip does not need a second lookup in the bin.
    pub media_name: Option<String>,
    #[ts(type = "number")]
    pub source_start: Frame,
    /// Every clip that moves with this one, this clip included. A clip with no link is
    /// a group of one, so a caller never has to tell the two cases apart.
    pub link_group: Vec<ClipId>,
    /// Whether the picture is moved, scaled, turned or faded. The values themselves are
    /// left out: they are five more numbers and are rarely the question.
    pub has_transform: bool,
}

/// One track and what is on it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TrackView {
    pub id: TrackId,
    pub name: String,
    pub kind: TrackKind,
    pub enabled: bool,
    pub locked: bool,
    pub clips: Vec<ClipSummary>,
}

/// A sequence and its tracks, in the order the renderer reads them: video lowest first,
/// so a later track is drawn over an earlier one.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TimelineView {
    pub sequence: SequenceSummary,
    pub tracks: Vec<TrackView>,
    /// The window asked for, when one was, so a caller reading a long timeline in pieces
    /// can tell which piece came back.
    #[ts(type = "number | null")]
    pub window_start: Option<Frame>,
    #[ts(type = "number | null")]
    pub window_end: Option<Frame>,
}

/// The project and its sequences, without any clips.
pub fn overview(project: &Project) -> ProjectOverview {
    ProjectOverview {
        name: project.name.clone(),
        media_count: project.media.len(),
        sequences: project.sequences.iter().map(sequence_summary).collect(),
    }
}

pub fn sequence_summary(sequence: &Sequence) -> SequenceSummary {
    let rate = sequence.format.frame_rate;
    let duration = sequence.duration();
    SequenceSummary {
        id: sequence.id,
        name: sequence.name.clone(),
        format: sequence.format,
        duration,
        duration_timecode: timecode(duration, rate),
        video_tracks: sequence
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Video)
            .count(),
        audio_tracks: sequence
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Audio)
            .count(),
        clip_count: sequence.tracks.iter().map(|t| t.clips.len()).sum(),
    }
}

/// One clip, flattened. `project` is read for the media name and the link group.
pub fn clip_summary(
    project: &Project,
    sequence: &Sequence,
    track: &Track,
    clip: &Clip,
) -> ClipSummary {
    let rate = sequence.format.frame_rate;
    let media_id = clip.source.media_id();
    ClipSummary {
        id: clip.id,
        name: clip.name.clone(),
        track_id: track.id,
        start: clip.start,
        start_timecode: timecode(clip.start, rate),
        duration: clip.duration,
        end: clip.end(),
        end_timecode: timecode(clip.end(), rate),
        enabled: clip.enabled,
        media_id,
        media_name: project.media(media_id).map(|m| m.name.clone()),
        source_start: clip.source.source_start(),
        link_group: project.link_group(clip.id),
        has_transform: !clip.transform.is_identity(),
    }
}

/// A sequence with its tracks and clips.
///
/// `window` keeps a long timeline readable in pieces: a clip is in the window when any
/// part of it is, so a clip that straddles the edge is reported by both pieces rather
/// than by neither.
pub fn describe(
    project: &Project,
    sequence: &Sequence,
    window: Option<(Frame, Frame)>,
) -> TimelineView {
    let in_window = |clip: &Clip| match window {
        None => true,
        Some((from, to)) => clip.start < to && from < clip.end(),
    };
    TimelineView {
        sequence: sequence_summary(sequence),
        tracks: sequence
            .tracks
            .iter()
            .map(|track| TrackView {
                id: track.id,
                name: track.name.clone(),
                kind: track.kind,
                enabled: track.enabled,
                locked: track.locked,
                clips: track
                    .clips
                    .iter()
                    .filter(|c| in_window(c))
                    .map(|c| clip_summary(project, sequence, track, c))
                    .collect(),
            })
            .collect(),
        window_start: window.map(|(from, _)| from),
        window_end: window.map(|(_, to)| to),
    }
}

/// A clip that is playing, and where in its media it has reached.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActiveClip {
    pub clip: ClipSummary,
    /// How far into the media the frame on screen is, counted in sequence frames, which
    /// is the unit `sourceStart` is already in.
    #[ts(type = "number")]
    pub source_frame: Frame,
    pub source_timecode: String,
}

/// What is playing at one frame.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Stack {
    #[ts(type = "number")]
    pub frame: Frame,
    pub timecode: String,
    /// In painting order, lowest track first, so the last entry is the one in front.
    /// Every layer is listed rather than only the top one, because a clip may be scaled,
    /// turned or faded and then what is under it shows through.
    pub video: Vec<ActiveClip>,
    pub audio: Vec<ActiveClip>,
}

/// What plays at `frame`.
///
/// This is the rule the renderer uses, in `export::render`: a disabled track or a
/// disabled clip is not there, and within a track at most one clip can contain a frame,
/// because the document forbids them from overlapping.
///
/// `src/playback/resolve.ts` is the same rule again, for the preview, and drops a clip
/// whose media is not in the bin. This does not: it reports the clip with no media name.
/// Validation makes that unreachable in a document anyone can get hold of, and if it
/// ever were reached, saying a clip is there without its media is more use than saying
/// nothing is there.
pub fn clips_at(project: &Project, sequence: &Sequence, frame: Frame) -> Stack {
    let rate = sequence.format.frame_rate;
    let on = |kind: TrackKind| -> Vec<ActiveClip> {
        sequence
            .tracks
            .iter()
            .filter(|t| t.kind == kind && t.enabled)
            .filter_map(|track| {
                let clip = track
                    .clips
                    .iter()
                    .find(|c| c.enabled && c.contains(frame))?;
                let source_frame = clip.source.source_start() + frame - clip.start;
                Some(ActiveClip {
                    clip: clip_summary(project, sequence, track, clip),
                    source_frame,
                    source_timecode: timecode(source_frame, rate),
                })
            })
            .collect()
    };
    Stack {
        frame,
        timecode: timecode(frame, rate),
        video: on(TrackKind::Video),
        audio: on(TrackKind::Audio),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{media_with_av, project_with, sample_edit};

    /// `sample_edit` puts linked video and audio at frames 12..60 and 100..124, at 24 fps.
    fn sample() -> (Project, Sequence) {
        let project = sample_edit();
        let sequence = project.first_sequence().clone();
        (project, sequence)
    }

    #[test]
    fn overview_counts_without_listing_clips() {
        let (project, _) = sample();
        let view = overview(&project);
        assert_eq!(view.media_count, 1);
        assert_eq!(view.sequences.len(), 1);
        let s = &view.sequences[0];
        assert_eq!(s.video_tracks, 1);
        assert_eq!(s.audio_tracks, 1);
        assert_eq!(s.clip_count, 4, "two linked pairs");
        assert_eq!(s.duration, 124, "one past the last frame of the last clip");
    }

    #[test]
    fn an_empty_sequence_is_zero_long() {
        let project = project_with(vec![media_with_av(5)]);
        let summary = sequence_summary(project.first_sequence());
        assert_eq!(summary.duration, 0);
        assert_eq!(summary.duration_timecode, "00:00:00:00");
        assert_eq!(summary.clip_count, 0);
    }

    #[test]
    fn every_frame_is_answered_with_its_timecode() {
        let (project, sequence) = sample();
        let view = describe(&project, &sequence, None);
        let clip = &view.tracks[0].clips[0];
        assert_eq!(clip.start, 12);
        assert_eq!(
            clip.start_timecode, "00:00:00:12",
            "24 fps, so frame 12 is :12"
        );
        assert_eq!(clip.end, 60);
        assert_eq!(clip.end_timecode, "00:00:02:12");
        assert_eq!(view.sequence.duration_timecode, "00:00:05:04", "frame 124");
    }

    #[test]
    fn a_clip_carries_its_link_group_and_its_media_name() {
        let (project, sequence) = sample();
        let view = describe(&project, &sequence, None);
        let clip = &view.tracks[0].clips[0];
        assert_eq!(clip.media_name.as_deref(), Some("shot.mov"));
        assert_eq!(clip.link_group.len(), 2, "the picture and its sound");
        assert!(clip.link_group.contains(&clip.id));
        assert!(!clip.has_transform, "nothing has been moved or scaled");
    }

    #[test]
    fn a_window_keeps_a_clip_that_straddles_its_edge() {
        let (project, sequence) = sample();
        // The first clip runs 12..60. A window starting inside it must still report it,
        // or a caller reading a timeline in pieces would lose every clip on a seam.
        let view = describe(&project, &sequence, Some((30, 90)));
        let ids: Vec<_> = view.tracks[0].clips.iter().map(|c| c.start).collect();
        assert_eq!(
            ids,
            vec![12],
            "straddling clip kept, the 100.. one left out"
        );

        // Touching is not overlapping: a window ending exactly where a clip starts does
        // not include it, so two adjacent windows do not both report the same clip.
        let view = describe(&project, &sequence, Some((0, 12)));
        assert!(view.tracks[0].clips.is_empty());
    }

    #[test]
    fn a_window_is_reported_back() {
        let (project, sequence) = sample();
        assert_eq!(describe(&project, &sequence, None).window_start, None);
        let view = describe(&project, &sequence, Some((10, 20)));
        assert_eq!((view.window_start, view.window_end), (Some(10), Some(20)));
    }

    #[test]
    fn tracks_come_back_in_painting_order() {
        let (project, sequence) = sample();
        let view = describe(&project, &sequence, None);
        let kinds: Vec<_> = view.tracks.iter().map(|t| t.kind).collect();
        assert_eq!(kinds, vec![TrackKind::Video, TrackKind::Audio]);
    }
}

#[cfg(test)]
mod playing {
    use super::*;
    use crate::edit::{apply, EditCommand};
    use crate::testing::sample_edit;

    fn sample() -> (Project, Sequence) {
        let project = sample_edit();
        let sequence = project.first_sequence().clone();
        (project, sequence)
    }

    #[test]
    fn nothing_plays_in_a_gap() {
        let (project, sequence) = sample();
        // The clips are 12..60 and 100..124, so 80 is between them.
        let stack = clips_at(&project, &sequence, 80);
        assert!(stack.video.is_empty() && stack.audio.is_empty());
        assert_eq!(stack.timecode, "00:00:03:08");
    }

    #[test]
    fn the_picture_and_its_sound_play_together() {
        let (project, sequence) = sample();
        let stack = clips_at(&project, &sequence, 24);
        assert_eq!(stack.video.len(), 1);
        assert_eq!(stack.audio.len(), 1);
    }

    #[test]
    fn source_position_counts_from_where_the_clip_reads() {
        let (project, sequence) = sample();
        // The first clip starts at 12 and reads its media from 6, so twelve frames in
        // at frame 24 the media is at 18.
        let stack = clips_at(&project, &sequence, 24);
        assert_eq!(stack.video[0].source_frame, 18);
        assert_eq!(stack.video[0].source_timecode, "00:00:00:18");
    }

    #[test]
    fn the_last_frame_of_a_clip_is_the_one_before_its_end() {
        let (project, sequence) = sample();
        assert_eq!(clips_at(&project, &sequence, 59).video.len(), 1, "inside");
        assert!(
            clips_at(&project, &sequence, 60).video.is_empty(),
            "end is exclusive"
        );
    }

    #[test]
    fn a_disabled_clip_is_not_playing() {
        let (project, sequence) = sample();
        let clip = sequence.tracks[0].clips[0].id;
        let project = apply(
            &project,
            &EditCommand::SetClipsEnabled {
                clip_ids: vec![clip],
                enabled: false,
            },
        )
        .unwrap()
        .0;
        let sequence = project.first_sequence().clone();
        let stack = clips_at(&project, &sequence, 24);
        assert!(stack.video.is_empty(), "the picture was disabled");
        assert!(stack.audio.is_empty(), "and its linked sound with it");
    }

    #[test]
    fn a_disabled_track_is_not_playing() {
        let (project, sequence) = sample();
        let track = sequence.tracks[0].id;
        let project = apply(
            &project,
            &EditCommand::SetTrackState {
                track_id: track,
                enabled: Some(false),
                locked: None,
            },
        )
        .unwrap()
        .0;
        let sequence = project.first_sequence().clone();
        let stack = clips_at(&project, &sequence, 24);
        assert!(stack.video.is_empty());
        assert_eq!(stack.audio.len(), 1, "only the video track was turned off");
    }

    #[test]
    fn video_comes_back_lowest_track_first() {
        let (project, sequence) = sample();
        let sequence_id = sequence.id;
        let media_id = project.media[0].id;
        // Put a second video clip over the first, on a new track above it.
        let project = apply(
            &project,
            &EditCommand::InsertMedia {
                sequence_id,
                media_id,
                start: 12,
                source_start: 0,
                duration: Some(48),
                video_track: crate::edit::TrackChoice::New,
                audio_track: crate::edit::TrackChoice::Skip,
            },
        )
        .unwrap()
        .0;
        let sequence = project.first_sequence().clone();
        let stack = clips_at(&project, &sequence, 24);
        assert_eq!(
            stack.video.len(),
            2,
            "both layers are reported, not just the top"
        );
        let tracks: Vec<_> = sequence.tracks.iter().map(|t| t.id).collect();
        let first = stack.video[0].clip.track_id;
        let last = stack.video[1].clip.track_id;
        let pos = |id| tracks.iter().position(|t| *t == id).unwrap();
        assert!(
            pos(first) < pos(last),
            "painting order: the one in front is last"
        );
    }
}
