//! Edit commands: the only way the document changes.
//!
//! [`apply`] runs a command against a copy of the project, normalizes links, validates the
//! result, and returns the new project. If any step fails the original is untouched, so a
//! rejected command is always a no-op. The UI, keyboard shortcuts and a future agent all
//! issue these same commands.
//!
//! Placement onto occupied frames uses overwrite semantics, as in an NLE overwrite edit:
//! clips underneath are trimmed, split or removed to make room. A linked video and audio
//! pair underneath is overwritten whole and stays linked, unless part of it sits on a
//! track the edit does not touch.

use crate::ids::{ClipId, LinkId, MediaId, SequenceId, TrackId};
use crate::model::{
    Clip, ClipSource, MediaAsset, Milli, Offset, Project, Scale, Sequence, SequenceFormat, Track,
    TrackKind, Transform, MILLI,
};
use crate::time::Frame;
use crate::validate::{ValidationError, MAX_NAME_LEN, MAX_OFFSET, MAX_ROTATION, MAX_SCALE};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use ts_rs::TS;

/// Default length of a still image dropped on the timeline.
pub const DEFAULT_IMAGE_SECONDS: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ClipEdge {
    Start,
    End,
}

/// Where one stream of inserted media goes.
///
/// Explicit on purpose: "this media has no such stream, or I do not want it placed" and
/// "I wanted to place this stream but found no track for it" are different intents, and
/// a single "no track" value cannot tell them apart. Inferring one from the other is how
/// a drop onto a sequence whose only audio track is locked lays down the picture and
/// loses the sound with nothing on screen to say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", content = "trackId", rename_all = "camelCase")]
#[ts(export)]
pub enum TrackChoice {
    /// Place on this track: it must be in the target sequence, of the matching kind and
    /// unlocked.
    Track(TrackId),
    /// Add a track of the matching kind to the target sequence and place there.
    New,
    /// Leave this stream off the timeline.
    Skip,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum EditCommand {
    /// Renames the project. The name is trimmed and must be 1 to 256 characters.
    RenameProject { name: String },
    /// The frame rate can change only while that sequence is empty, because every
    /// frame count stored in it is relative to the rate.
    SetSequenceFormat {
        sequence_id: SequenceId,
        format: SequenceFormat,
    },
    /// A new empty sequence, which becomes the last in the bin.
    AddSequence {
        name: String,
        format: SequenceFormat,
    },
    /// Rejected for the last sequence: a project always has one.
    RemoveSequence { sequence_id: SequenceId },
    /// Renames one sequence. The name is trimmed and must be 1 to 256 characters.
    RenameSequence {
        sequence_id: SequenceId,
        name: String,
    },
    /// Adds probed assets to the bin. An asset whose path is already in the bin is skipped.
    ImportMedia { assets: Vec<MediaAsset> },
    /// Rejected while any clip uses the media.
    RemoveMedia { media_id: MediaId },
    /// Adds an empty track of this kind, named from the kind and how many of that kind
    /// there already are (V1, V2, A1). It goes last among its own kind, which puts a new
    /// video track on top of the stack and a new audio track at the bottom. The new
    /// track's id comes back in the report.
    AddTrack {
        sequence_id: SequenceId,
        kind: TrackKind,
    },
    /// Removes a track and every clip on it. Rejected while the track is locked. A
    /// track knows its own sequence, so the command does not name one.
    RemoveTrack { track_id: TrackId },
    /// Turns a track's output on or off, and locks or unlocks it against editing. A
    /// field left `None` keeps the value it had, so either can be set without sending
    /// the other back unchanged. A disabled track still holds its clips; it is left out
    /// of playback and of the render. Locking changes what may be edited, not what
    /// plays.
    SetTrackState {
        track_id: TrackId,
        enabled: Option<bool>,
        locked: Option<bool>,
    },
    /// Places media on the timeline. `video_track` and `audio_track` each say where that
    /// stream goes, or that it is left out; two streams placed together are linked.
    InsertMedia {
        sequence_id: SequenceId,
        media_id: MediaId,
        #[ts(type = "number")]
        start: Frame,
        #[ts(type = "number")]
        source_start: Frame,
        /// Defaults to the rest of the media, or five seconds for a still image.
        #[ts(type = "number | null")]
        duration: Option<Frame>,
        video_track: TrackChoice,
        audio_track: TrackChoice,
    },
    /// Moves a clip and its link group in time. `track_id` moves only this clip to
    /// another track of the same kind.
    MoveClip {
        clip_id: ClipId,
        #[ts(type = "number")]
        start: Frame,
        track_id: Option<TrackId>,
    },
    /// Moves one edge of a clip and its link group to `frame`, clamped so the clip stays
    /// at least one frame long, inside its media, and clear of neighbouring clips.
    TrimClip {
        clip_id: ClipId,
        edge: ClipEdge,
        #[ts(type = "number")]
        frame: Frame,
    },
    /// Splits every listed clip (and its link group) that spans `at`.
    SplitClips {
        clip_ids: Vec<ClipId>,
        #[ts(type = "number")]
        at: Frame,
    },
    /// Removes each listed clip and the rest of its link group, so deleting a picture
    /// takes its sound with it. Rejected if any of them sits on a locked track.
    DeleteClips { clip_ids: Vec<ClipId> },
    /// Takes clips out of playback and the render while leaving them in place on the
    /// timeline. Reaches the whole link group, and is rejected if any of them sits on a
    /// locked track.
    SetClipsEnabled {
        clip_ids: Vec<ClipId>,
        enabled: bool,
    },
    /// Renames one clip. Unlike the commands that change timing, this does not reach
    /// the link group: a picture and its sound can be named separately. The name is
    /// trimmed and must be 1 to 256 characters, and the clip's track must be unlocked.
    RenameClip { clip_id: ClipId, name: String },
    /// Moves, scales, rotates or fades the picture of each listed clip. A field left
    /// `None` keeps the value it had, so a panel commits one number at a time without
    /// having to send the rest back unchanged. Out of range values are clamped, and
    /// rotation wraps, so this command cannot produce a transform the document would
    /// reject.
    ///
    /// Unlike the timing commands this does not reach the link group: a transform
    /// belongs to the picture, and the audio half of a pair has none.
    SetClipsTransform {
        clip_ids: Vec<ClipId>,
        position: Option<Offset>,
        scale: Option<Scale>,
        rotation: Option<Milli>,
        anchor: Option<Offset>,
        opacity: Option<Milli>,
    },
    /// Breaks the link between a picture and its sound so that each can then be moved,
    /// trimmed and deleted on its own. Unlinking any clip unlinks the whole group it
    /// belonged to.
    UnlinkClips { clip_ids: Vec<ClipId> },
}

impl EditCommand {
    /// Short human label for undo history and logs.
    pub fn label(&self) -> &'static str {
        match self {
            EditCommand::RenameProject { .. } => "Rename project",
            EditCommand::SetSequenceFormat { .. } => "Change sequence settings",
            EditCommand::AddSequence { .. } => "Add sequence",
            EditCommand::RemoveSequence { .. } => "Remove sequence",
            EditCommand::RenameSequence { .. } => "Rename sequence",
            EditCommand::ImportMedia { .. } => "Import media",
            EditCommand::RemoveMedia { .. } => "Remove media",
            EditCommand::AddTrack { .. } => "Add track",
            EditCommand::RemoveTrack { .. } => "Remove track",
            EditCommand::SetTrackState { .. } => "Change track",
            EditCommand::InsertMedia { .. } => "Insert clip",
            EditCommand::MoveClip { .. } => "Move clip",
            EditCommand::TrimClip { .. } => "Trim clip",
            EditCommand::SplitClips { .. } => "Split",
            EditCommand::DeleteClips { .. } => "Delete",
            EditCommand::SetClipsEnabled { .. } => "Enable or disable clips",
            EditCommand::RenameClip { .. } => "Rename clip",
            EditCommand::SetClipsTransform { .. } => "Change transform",
            EditCommand::UnlinkClips { .. } => "Unlink",
        }
    }
}

/// What a successful command created, so the UI can select it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditReport {
    pub created_clips: Vec<ClipId>,
    pub created_media: Vec<MediaId>,
    pub created_tracks: Vec<TrackId>,
    pub created_sequence: Option<SequenceId>,
}

#[derive(Debug, Clone, PartialEq, Error, Serialize, TS)]
#[serde(
    tag = "code",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum EditError {
    #[error("{what} {id} not found")]
    NotFound { what: String, id: String },
    #[error("track {track} is locked")]
    TrackLocked { track: TrackId },
    #[error("track kind does not match")]
    WrongTrackKind,
    #[error("media {media} is used by clips on the timeline")]
    MediaInUse { media: MediaId },
    #[error("{reason}")]
    InvalidArgument { reason: String },
    #[error("edit would leave the project invalid: {source}")]
    Invalid { source: ValidationError },
}

impl From<ValidationError> for EditError {
    fn from(source: ValidationError) -> Self {
        EditError::Invalid { source }
    }
}

fn not_found(what: &str, id: impl ToString) -> EditError {
    EditError::NotFound {
        what: what.into(),
        id: id.to_string(),
    }
}

fn invalid(reason: &str) -> EditError {
    EditError::InvalidArgument {
        reason: reason.into(),
    }
}

/// Apply `command` to a copy of `project`. Returns the new project only if it validates.
pub fn apply(project: &Project, command: &EditCommand) -> Result<(Project, EditReport), EditError> {
    let mut next = project.clone();
    let report = apply_in_place(&mut next, command)?;
    normalize_links(&mut next);
    next.validate()?;
    Ok((next, report))
}

fn apply_in_place(p: &mut Project, command: &EditCommand) -> Result<EditReport, EditError> {
    let mut report = EditReport::default();
    match command {
        EditCommand::RenameProject { name } => {
            p.name = checked_name(name)?;
        }
        EditCommand::SetSequenceFormat {
            sequence_id,
            format,
        } => {
            let sequence = p
                .sequence_mut(*sequence_id)
                .ok_or_else(|| not_found("sequence", sequence_id))?;
            // Only this sequence's own clips matter: another sequence's frame counts
            // are measured against its own rate and are unaffected.
            if format.frame_rate != sequence.format.frame_rate && sequence.clips().next().is_some()
            {
                return Err(invalid(
                    "frame rate can change only while the sequence is empty",
                ));
            }
            sequence.format = *format;
        }
        EditCommand::AddSequence { name, format } => {
            let sequence = Sequence::new(checked_name(name)?, *format);
            report.created_sequence = Some(sequence.id);
            p.sequences.push(sequence);
        }
        EditCommand::RemoveSequence { sequence_id } => {
            if p.sequence(*sequence_id).is_none() {
                return Err(not_found("sequence", sequence_id));
            }
            if p.sequences.len() == 1 {
                return Err(invalid("a project keeps at least one sequence"));
            }
            p.sequences.retain(|s| s.id != *sequence_id);
        }
        EditCommand::RenameSequence { sequence_id, name } => {
            let name = checked_name(name)?;
            let sequence = p
                .sequence_mut(*sequence_id)
                .ok_or_else(|| not_found("sequence", sequence_id))?;
            sequence.name = name;
        }
        EditCommand::ImportMedia { assets } => {
            for asset in assets {
                if let Some(existing) = p.media.iter().find(|m| m.path == asset.path) {
                    report.created_media.push(existing.id);
                    continue;
                }
                report.created_media.push(asset.id);
                p.media.push(asset.clone());
            }
        }
        EditCommand::RemoveMedia { media_id } => {
            if p.media(*media_id).is_none() {
                return Err(not_found("media", media_id));
            }
            if p.all_clips().any(|(_, c)| c.source.media_id() == *media_id) {
                return Err(EditError::MediaInUse { media: *media_id });
            }
            p.media.retain(|m| m.id != *media_id);
        }
        EditCommand::AddTrack { sequence_id, kind } => {
            if p.sequence(*sequence_id).is_none() {
                return Err(not_found("sequence", sequence_id));
            }
            report
                .created_tracks
                .push(add_track(p, *sequence_id, *kind));
        }
        EditCommand::RemoveTrack { track_id } => {
            ensure_unlocked(p, *track_id)?;
            // The track says which sequence it is in, so the command does not have to.
            if let Some(sequence) = p.sequence_of_track_mut(*track_id) {
                sequence.tracks.retain(|t| t.id != *track_id);
            }
        }
        EditCommand::SetTrackState {
            track_id,
            enabled,
            locked,
        } => {
            let track = p
                .track_mut(*track_id)
                .ok_or_else(|| not_found("track", track_id))?;
            if let Some(enabled) = enabled {
                track.enabled = *enabled;
            }
            if let Some(locked) = locked {
                track.locked = *locked;
            }
        }
        EditCommand::InsertMedia {
            sequence_id,
            media_id,
            start,
            source_start,
            duration,
            video_track,
            audio_track,
        } => {
            let (clips, tracks) = insert_media(
                p,
                *sequence_id,
                *media_id,
                *start,
                *source_start,
                *duration,
                *video_track,
                *audio_track,
            )?;
            report.created_clips = clips;
            report.created_tracks = tracks;
        }
        EditCommand::MoveClip {
            clip_id,
            start,
            track_id,
        } => move_clip(p, *clip_id, *start, *track_id)?,
        EditCommand::TrimClip {
            clip_id,
            edge,
            frame,
        } => trim_clip(p, *clip_id, *edge, *frame)?,
        EditCommand::SplitClips { clip_ids, at } => {
            report.created_clips = split_clips(p, clip_ids, *at)?;
        }
        EditCommand::DeleteClips { clip_ids } => {
            let ids = expand_groups(p, clip_ids)?;
            ensure_clips_unlocked(p, &ids)?;
            for id in ids {
                remove_clip(p, id);
            }
        }
        EditCommand::SetClipsEnabled { clip_ids, enabled } => {
            let ids = expand_groups(p, clip_ids)?;
            ensure_clips_unlocked(p, &ids)?;
            for_each_clip_mut(p, &ids, |c| c.enabled = *enabled);
        }
        EditCommand::RenameClip { clip_id, name } => {
            let name = checked_name(name)?;
            ensure_clips_unlocked(p, &[*clip_id])?;
            for_each_clip_mut(p, &[*clip_id], |c| c.name = name.clone());
        }
        EditCommand::SetClipsTransform {
            clip_ids,
            position,
            scale,
            rotation,
            anchor,
            opacity,
        } => set_clips_transform(p, clip_ids, *position, *scale, *rotation, *anchor, *opacity)?,
        EditCommand::UnlinkClips { clip_ids } => {
            let ids = expand_groups(p, clip_ids)?;
            for_each_clip_mut(p, &ids, |c| c.link = None);
        }
    }
    Ok(report)
}

/// Change part of the transform on each listed clip.
///
/// Values are clamped to what a backend can draw rather than rejected: a panel dragging
/// a slider to its end should land on the limit, not fail. Rotation wraps instead, so
/// passing a full turn keeps going and two documents showing the same picture hold the
/// same number.
#[allow(clippy::too_many_arguments)]
fn set_clips_transform(
    p: &mut Project,
    ids: &[ClipId],
    position: Option<Offset>,
    scale: Option<Scale>,
    rotation: Option<Milli>,
    anchor: Option<Offset>,
    opacity: Option<Milli>,
) -> Result<(), EditError> {
    for id in ids {
        let (track, _) = p.find_clip(*id).ok_or_else(|| not_found("clip", id))?;
        // Not silently ignored: asking to transform a sound is a mistake worth hearing
        // about, and the document forbids the result anyway.
        if track.kind != TrackKind::Video {
            return Err(EditError::WrongTrackKind);
        }
    }
    ensure_clips_unlocked(p, ids)?;
    for_each_clip_mut(p, ids, |c| {
        let t = &mut c.transform;
        if let Some(position) = position {
            t.position = clamp_offset(position);
        }
        if let Some(scale) = scale {
            t.scale = clamp_scale(scale);
        }
        if let Some(rotation) = rotation {
            t.rotation = rotation % MAX_ROTATION;
        }
        if let Some(anchor) = anchor {
            t.anchor = clamp_offset(anchor);
        }
        if let Some(opacity) = opacity {
            t.opacity = opacity.clamp(0, MILLI);
        }
    });
    Ok(())
}

fn clamp_offset(offset: Offset) -> Offset {
    Offset {
        x: offset.x.clamp(-MAX_OFFSET, MAX_OFFSET),
        y: offset.y.clamp(-MAX_OFFSET, MAX_OFFSET),
    }
}

/// Scale stays positive: zero draws nothing and a negative would be a flip, which is its
/// own effect when it arrives.
fn clamp_scale(scale: Scale) -> Scale {
    Scale {
        x: scale.x.clamp(1, MAX_SCALE),
        y: scale.y.clamp(1, MAX_SCALE),
    }
}

fn checked_name(name: &str) -> Result<String, EditError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_LEN {
        return Err(invalid("name must be 1 to 256 characters"));
    }
    Ok(name.to_string())
}

fn add_track(p: &mut Project, sequence_id: SequenceId, kind: TrackKind) -> TrackId {
    let sequence = p.sequence(sequence_id).expect("checked by the caller");
    let count = sequence.tracks.iter().filter(|t| t.kind == kind).count();
    let prefix = match kind {
        TrackKind::Video => "V",
        TrackKind::Audio => "A",
    };
    let track = Track::new(kind, format!("{prefix}{}", count + 1));
    let id = track.id;
    // Keep video tracks before audio tracks; within a kind, new tracks go last (on top
    // for video, at the bottom for audio).
    let index = match kind {
        TrackKind::Video => sequence
            .tracks
            .iter()
            .rposition(|t| t.kind == TrackKind::Video)
            .map_or(0, |i| i + 1),
        TrackKind::Audio => sequence.tracks.len(),
    };
    p.sequence_mut(sequence_id)
        .expect("checked by the caller")
        .tracks
        .insert(index, track);
    id
}

fn ensure_unlocked(p: &Project, track_id: TrackId) -> Result<&Track, EditError> {
    let track = p
        .track(track_id)
        .ok_or_else(|| not_found("track", track_id))?;
    if track.locked {
        return Err(EditError::TrackLocked { track: track_id });
    }
    Ok(track)
}

fn ensure_clips_unlocked(p: &Project, ids: &[ClipId]) -> Result<(), EditError> {
    for id in ids {
        let (track, _) = p.find_clip(*id).ok_or_else(|| not_found("clip", id))?;
        if track.locked {
            return Err(EditError::TrackLocked { track: track.id });
        }
    }
    Ok(())
}

/// The listed clips plus every clip linked to them, without duplicates, in input order.
fn expand_groups(p: &Project, ids: &[ClipId]) -> Result<Vec<ClipId>, EditError> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for id in ids {
        if p.find_clip(*id).is_none() {
            return Err(not_found("clip", id));
        }
        for member in p.link_group(*id) {
            if seen.insert(member) {
                out.push(member);
            }
        }
    }
    Ok(out)
}

fn for_each_clip_mut(p: &mut Project, ids: &[ClipId], mut f: impl FnMut(&mut Clip)) {
    let wanted: HashSet<_> = ids.iter().copied().collect();
    for track in p.tracks_mut() {
        for clip in &mut track.clips {
            if wanted.contains(&clip.id) {
                f(clip);
            }
        }
    }
}

fn remove_clip(p: &mut Project, id: ClipId) -> Option<(TrackId, Clip)> {
    for track in p.tracks_mut() {
        if let Some(index) = track.clips.iter().position(|c| c.id == id) {
            return Some((track.id, track.clips.remove(index)));
        }
    }
    None
}

fn insert_sorted(track: &mut Track, clip: Clip) {
    let index = track.clips.partition_point(|c| c.start < clip.start);
    track.clips.insert(index, clip);
}

/// Make room for `[start, end)` on each of `tracks` by trimming, splitting or removing
/// clips, the way an NLE overwrite edit does.
///
/// Link groups are overwritten whole. When this call clears the track of every member of
/// a group, all members take the same cut and keep their link, so dropping a clip over
/// part of a linked pair leaves both halves linked, as in Premiere and Resolve. A group
/// with a member on a track this call does not clear (a locked track, or simply a track
/// the edit never touches) cannot stay in step, since every member must share `start`,
/// `duration` and source range, so there the clips it touches lose their link instead.
fn clear_range(
    p: &mut Project,
    tracks: &[TrackId],
    start: Frame,
    end: Frame,
) -> Result<(), EditError> {
    for track_id in tracks {
        ensure_unlocked(p, *track_id)?;
    }
    let cleared: HashSet<TrackId> = tracks.iter().copied().collect();
    let survivors = groups_cleared_whole(p, &cleared);

    // Right halves of a split group share one new link, so they stay a group as well.
    let mut split_links: HashMap<LinkId, LinkId> = HashMap::new();
    let mut broken_links = HashSet::new();
    let mut done = HashSet::new();
    for track_id in tracks {
        if !done.insert(*track_id) {
            continue;
        }
        let track = p.track_mut(*track_id).expect("checked above");
        let mut kept = Vec::with_capacity(track.clips.len() + 1);
        for mut clip in std::mem::take(&mut track.clips) {
            if clip.end() <= start || clip.start >= end {
                kept.push(clip);
                continue;
            }
            if let Some(link) = clip.link.filter(|l| !survivors.contains(l)) {
                clip.link = None;
                broken_links.insert(link);
            }
            let covers_start = clip.start < start;
            let covers_end = clip.end() > end;
            match (covers_start, covers_end) {
                (false, false) => {} // fully inside: removed
                (true, false) => {
                    clip.duration = start - clip.start;
                    kept.push(clip);
                }
                (false, true) => {
                    let cut = end - clip.start;
                    clip.source = clip
                        .source
                        .with_source_start(clip.source.source_start() + cut);
                    clip.start = end;
                    clip.duration -= cut;
                    kept.push(clip);
                }
                (true, true) => {
                    let mut right = clip.clone();
                    right.id = ClipId::new();
                    right.link = clip.link.map(|l| *split_links.entry(l).or_default());
                    let cut = end - clip.start;
                    right.source = clip
                        .source
                        .with_source_start(clip.source.source_start() + cut);
                    right.start = end;
                    right.duration = clip.end() - end;
                    clip.duration = start - clip.start;
                    kept.push(clip);
                    kept.push(right);
                }
            }
        }
        kept.sort_by_key(|c| c.start);
        track.clips = kept;
    }
    dissolve_links(p, &broken_links);
    Ok(())
}

/// Links whose every member sits on a track in `cleared`. Members share `start`,
/// `duration` and source range, so one range cuts them all the same way.
fn groups_cleared_whole(p: &Project, cleared: &HashSet<TrackId>) -> HashSet<LinkId> {
    let mut outside: HashSet<LinkId> = HashSet::new();
    let mut all: HashSet<LinkId> = HashSet::new();
    for (track, clip) in p.all_clips() {
        if let Some(link) = clip.link {
            all.insert(link);
            if !cleared.contains(&track.id) {
                outside.insert(link);
            }
        }
    }
    all.difference(&outside).copied().collect()
}

fn dissolve_links(p: &mut Project, links: &HashSet<LinkId>) {
    if links.is_empty() {
        return;
    }
    for track in p.tracks_mut() {
        for clip in &mut track.clips {
            if clip.link.is_some_and(|l| links.contains(&l)) {
                clip.link = None;
            }
        }
    }
}

/// Drop links left with fewer than two members, for example after deleting one side.
fn normalize_links(p: &mut Project) {
    let mut counts: HashMap<LinkId, usize> = HashMap::new();
    for (_, clip) in p.all_clips() {
        if let Some(link) = clip.link {
            *counts.entry(link).or_default() += 1;
        }
    }
    let lonely: HashSet<_> = counts
        .into_iter()
        .filter(|(_, n)| *n < 2)
        .map(|(l, _)| l)
        .collect();
    dissolve_links(p, &lonely);
}

#[allow(clippy::too_many_arguments)]
fn insert_media(
    p: &mut Project,
    sequence_id: SequenceId,
    media_id: MediaId,
    start: Frame,
    source_start: Frame,
    duration: Option<Frame>,
    video_track: TrackChoice,
    audio_track: TrackChoice,
) -> Result<(Vec<ClipId>, Vec<TrackId>), EditError> {
    let sequence = p
        .sequence(sequence_id)
        .ok_or_else(|| not_found("sequence", sequence_id))?;
    let rate = sequence.format.frame_rate;
    let media = p
        .media(media_id)
        .ok_or_else(|| not_found("media", media_id))?;
    if start < 0 || source_start < 0 {
        return Err(invalid("start and source start must not be negative"));
    }
    let available = media.frames_at(rate);
    let duration = match (duration, available) {
        (Some(d), _) => d,
        (None, Some(total)) => total - source_start,
        (None, None) => rate.frames_in_seconds(DEFAULT_IMAGE_SECONDS),
    };
    if duration < 1 {
        return Err(invalid("clip must be at least one frame long"));
    }
    if available.is_some_and(|total| source_start + duration > total) {
        return Err(invalid("clip extends past the end of the media"));
    }

    let streams = [
        (media.has_video(), video_track, TrackKind::Video),
        (media.has_audio(), audio_track, TrackKind::Audio),
    ];
    let name = media.name.clone();

    let mut destinations = Vec::new();
    let mut created_tracks = Vec::new();
    for (present, choice, kind) in streams {
        if choice != TrackChoice::Skip && !present {
            return Err(invalid("media has no such stream to place"));
        }
        match choice {
            TrackChoice::Skip => {}
            TrackChoice::Track(track_id) => {
                // A track in another sequence would put the clip somewhere the command
                // did not name, with its frames counted at the wrong rate.
                if p.sequence_of_track(track_id).map(|s| s.id) != Some(sequence_id) {
                    return Err(not_found("track", track_id));
                }
                if ensure_unlocked(p, track_id)?.kind != kind {
                    return Err(EditError::WrongTrackKind);
                }
                destinations.push(track_id);
            }
            TrackChoice::New => {
                // Video is resolved first, so a pair of new tracks reports V before A.
                let track_id = add_track(p, sequence_id, kind);
                created_tracks.push(track_id);
                destinations.push(track_id);
            }
        }
    }
    if destinations.is_empty() {
        return Err(invalid("no stream of this media is being placed"));
    }

    let link = (destinations.len() > 1).then(LinkId::new);
    let mut created = Vec::new();
    // One call for every target track, so a linked clip underneath is overwritten whole.
    clear_range(p, &destinations, start, start + duration)?;
    for track_id in destinations {
        let clip = Clip {
            id: ClipId::new(),
            name: name.clone(),
            start,
            duration,
            enabled: true,
            link,
            transform: Transform::IDENTITY,
            source: ClipSource::Media {
                media_id,
                source_start,
            },
        };
        created.push(clip.id);
        insert_sorted(p.track_mut(track_id).expect("checked"), clip);
    }
    Ok((created, created_tracks))
}

fn move_clip(
    p: &mut Project,
    clip_id: ClipId,
    start: Frame,
    target_track: Option<TrackId>,
) -> Result<(), EditError> {
    let (origin_track, clip) = p
        .find_clip(clip_id)
        .ok_or_else(|| not_found("clip", clip_id))?;
    let origin_kind = origin_track.kind;
    let delta = start - clip.start;
    let group = p.link_group(clip_id);
    ensure_clips_unlocked(p, &group)?;
    if let Some(track_id) = target_track {
        let track = ensure_unlocked(p, track_id)?;
        if track.kind != origin_kind {
            return Err(EditError::WrongTrackKind);
        }
    }

    let mut moving = Vec::new();
    for id in &group {
        let (track_id, mut member) = remove_clip(p, *id).expect("group members exist");
        member.start += delta;
        if member.start < 0 {
            return Err(invalid("clip cannot move before the start of the timeline"));
        }
        let destination = if *id == clip_id {
            target_track.unwrap_or(track_id)
        } else {
            track_id
        };
        moving.push((destination, member));
    }
    // Group members share start and duration, so one range covers all of them.
    let Some((_, first)) = moving.first() else {
        return Ok(());
    };
    let (range_start, range_end) = (first.start, first.end());
    let destinations: Vec<_> = moving.iter().map(|(track_id, _)| *track_id).collect();
    clear_range(p, &destinations, range_start, range_end)?;
    for (track_id, member) in moving {
        insert_sorted(p.track_mut(track_id).expect("checked"), member);
    }
    Ok(())
}

fn trim_clip(
    p: &mut Project,
    clip_id: ClipId,
    edge: ClipEdge,
    frame: Frame,
) -> Result<(), EditError> {
    if p.find_clip(clip_id).is_none() {
        return Err(not_found("clip", clip_id));
    }
    let group = p.link_group(clip_id);
    ensure_clips_unlocked(p, &group)?;
    // The clip's own sequence sets the rate its frame counts are measured in.
    let rate = p
        .sequence_of_clip(clip_id)
        .ok_or_else(|| not_found("clip", clip_id))?
        .format
        .frame_rate;
    let members: HashSet<_> = group.iter().copied().collect();

    // Bounds are the intersection across the group so linked clips stay identical.
    let mut lower = Frame::MIN;
    let mut upper = Frame::MAX;
    for id in &group {
        let (track, clip) = p.find_clip(*id).expect("group members exist");
        let media = p
            .media(clip.source.media_id())
            .ok_or_else(|| not_found("media", clip.source.media_id()))?;
        let others = track.clips.iter().filter(|c| !members.contains(&c.id));
        match edge {
            ClipEdge::Start => {
                let neighbour_end = others
                    .filter(|c| c.end() <= clip.start)
                    .map(Clip::end)
                    .max()
                    .unwrap_or(0);
                let media_start = clip.start - clip.source.source_start();
                lower = lower.max(neighbour_end).max(media_start).max(0);
                upper = upper.min(clip.end() - 1);
            }
            ClipEdge::End => {
                let neighbour_start = others
                    .filter(|c| c.start >= clip.end())
                    .map(|c| c.start)
                    .min()
                    .unwrap_or(Frame::MAX);
                let media_end = media.frames_at(rate).map_or(Frame::MAX, |total| {
                    clip.start + total - clip.source.source_start()
                });
                lower = lower.max(clip.start + 1);
                upper = upper.min(neighbour_start).min(media_end);
            }
        }
    }
    if lower > upper {
        return Ok(()); // no room to move this edge
    }
    let target = frame.clamp(lower, upper);
    let ids: Vec<_> = group.clone();
    for_each_clip_mut(p, &ids, |clip| match edge {
        ClipEdge::Start => {
            let delta = target - clip.start;
            clip.start = target;
            clip.duration -= delta;
            clip.source = clip
                .source
                .with_source_start(clip.source.source_start() + delta);
        }
        ClipEdge::End => clip.duration = target - clip.start,
    });
    // Trimming never reorders clips, so tracks stay sorted.
    Ok(())
}

fn split_clips(p: &mut Project, clip_ids: &[ClipId], at: Frame) -> Result<Vec<ClipId>, EditError> {
    let ids = expand_groups(p, clip_ids)?;
    let spanning: Vec<_> = ids
        .into_iter()
        .filter(|id| {
            p.find_clip(*id)
                .is_some_and(|(_, c)| c.start < at && at < c.end())
        })
        .collect();
    ensure_clips_unlocked(p, &spanning)?;

    // Right halves of a linked pair get a fresh shared link.
    let mut new_links: HashMap<LinkId, LinkId> = HashMap::new();
    let mut created = Vec::new();
    for id in spanning {
        let (track_id, mut left) = remove_clip(p, id).expect("exists");
        let mut right = left.clone();
        right.id = ClipId::new();
        right.link = left.link.map(|l| *new_links.entry(l).or_default());
        let cut = at - left.start;
        right.start = at;
        right.duration = left.duration - cut;
        right.source = left
            .source
            .with_source_start(left.source.source_start() + cut);
        left.duration = cut;
        created.push(right.id);
        let track = p.track_mut(track_id).expect("exists");
        insert_sorted(track, left);
        insert_sorted(track, right);
    }
    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{media_with_av, project_with, still_image};

    struct Fixture {
        project: Project,
        media: MediaId,
        sequence: SequenceId,
        v1: TrackId,
        a1: TrackId,
    }

    fn fixture(seconds: i64) -> Fixture {
        let project = project_with(vec![media_with_av(seconds)]);
        Fixture {
            media: project.media[0].id,
            sequence: project.first_sequence().id,
            v1: project.first_sequence().tracks[0].id,
            a1: project.first_sequence().tracks[1].id,
            project,
        }
    }

    fn run(p: &mut Project, cmd: EditCommand) -> EditReport {
        let (next, report) = apply(p, &cmd).unwrap_or_else(|e| panic!("{cmd:?} failed: {e}"));
        *p = next;
        report
    }

    fn insert(f: &mut Fixture, start: Frame, source_start: Frame, duration: Frame) -> Vec<ClipId> {
        run(
            &mut f.project,
            EditCommand::InsertMedia {
                sequence_id: f.sequence,
                media_id: f.media,
                start,
                source_start,
                duration: Some(duration),
                video_track: TrackChoice::Track(f.v1),
                audio_track: TrackChoice::Track(f.a1),
            },
        )
        .created_clips
    }

    fn spans(p: &Project, track: TrackId) -> Vec<(Frame, Frame, Frame)> {
        p.track(track)
            .unwrap()
            .clips
            .iter()
            .map(|c| (c.start, c.duration, c.source.source_start()))
            .collect()
    }

    /// Link of the clip that begins on `start`, which identifies a half after a split.
    fn link_at(p: &Project, track: TrackId, start: Frame) -> Option<LinkId> {
        p.track(track)
            .unwrap()
            .clips
            .iter()
            .find(|c| c.start == start)
            .expect("clip at start")
            .link
    }

    /// Only the field named changes; the rest of the transform is left where it was.
    fn set(f: &mut Fixture, id: ClipId, command: EditCommand) -> Transform {
        run(&mut f.project, command);
        f.project.find_clip(id).expect("clip").1.transform
    }

    fn transform_of(f: &Fixture, id: ClipId) -> Transform {
        f.project.find_clip(id).expect("clip").1.transform
    }

    fn only_opacity(clip_ids: Vec<ClipId>, opacity: Milli) -> EditCommand {
        EditCommand::SetClipsTransform {
            clip_ids,
            position: None,
            scale: None,
            rotation: None,
            anchor: None,
            opacity: Some(opacity),
        }
    }

    #[test]
    fn a_new_clip_starts_untransformed() {
        let mut f = fixture(10);
        let ids = insert(&mut f, 0, 0, 24);
        for id in ids {
            assert!(transform_of(&f, id).is_identity());
        }
    }

    #[test]
    fn sets_one_field_and_leaves_the_others() {
        let mut f = fixture(10);
        let video = insert(&mut f, 0, 0, 24)[0];
        let scaled = set(
            &mut f,
            video,
            EditCommand::SetClipsTransform {
                clip_ids: vec![video],
                position: None,
                scale: Some(Scale { x: 500, y: 500 }),
                rotation: None,
                anchor: None,
                opacity: None,
            },
        );
        assert_eq!(scaled.scale, Scale { x: 500, y: 500 });
        assert_eq!(scaled.opacity, MILLI, "opacity untouched");
        assert_eq!(scaled.position, Offset { x: 0, y: 0 });

        let faded = set(&mut f, video, only_opacity(vec![video], 250));
        assert_eq!(faded.opacity, 250);
        assert_eq!(faded.scale, Scale { x: 500, y: 500 }, "scale survives");
    }

    #[test]
    fn clamps_a_scale_past_the_limit_instead_of_failing() {
        // A panel dragging a slider to its end should land on the limit.
        let mut f = fixture(10);
        let video = insert(&mut f, 0, 0, 24)[0];
        let t = set(
            &mut f,
            video,
            EditCommand::SetClipsTransform {
                clip_ids: vec![video],
                position: Some(Offset {
                    x: MAX_OFFSET * 2,
                    y: 0,
                }),
                scale: Some(Scale {
                    x: MAX_SCALE + 1,
                    y: 0,
                }),
                rotation: None,
                anchor: None,
                opacity: Some(MILLI * 9),
            },
        );
        assert_eq!(t.scale, Scale { x: MAX_SCALE, y: 1 });
        assert_eq!(t.position.x, MAX_OFFSET);
        assert_eq!(t.opacity, MILLI);
        f.project.validate().unwrap();
    }

    #[test]
    fn rotation_comes_back_round_rather_than_stopping() {
        let mut f = fixture(10);
        let video = insert(&mut f, 0, 0, 24)[0];
        let t = set(
            &mut f,
            video,
            EditCommand::SetClipsTransform {
                clip_ids: vec![video],
                position: None,
                scale: None,
                rotation: Some(450 * MILLI),
                anchor: None,
                opacity: None,
            },
        );
        assert_eq!(t.rotation, 90 * MILLI);
    }

    #[test]
    fn a_transform_does_not_reach_the_linked_audio() {
        let mut f = fixture(10);
        let ids = insert(&mut f, 0, 0, 24);
        let (video, audio) = (ids[0], ids[1]);
        set(&mut f, video, only_opacity(vec![video], 500));
        assert_eq!(transform_of(&f, video).opacity, 500);
        assert!(
            transform_of(&f, audio).is_identity(),
            "the sound has no picture to move"
        );
    }

    #[test]
    fn rejects_transforming_a_sound() {
        let mut f = fixture(10);
        let audio = insert(&mut f, 0, 0, 24)[1];
        assert_eq!(
            apply(&f.project, &only_opacity(vec![audio], 500)),
            Err(EditError::WrongTrackKind)
        );
    }

    #[test]
    fn rejects_a_transform_on_a_locked_track() {
        let mut f = fixture(10);
        let video = insert(&mut f, 0, 0, 24)[0];
        lock(&mut f.project, f.v1);
        assert!(matches!(
            apply(&f.project, &only_opacity(vec![video], 500)),
            Err(EditError::TrackLocked { .. })
        ));
    }

    #[test]
    fn transforms_several_clips_at_once() {
        let mut f = fixture(10);
        let first = insert(&mut f, 0, 0, 24)[0];
        let second = insert(&mut f, 48, 0, 24)[0];
        run(&mut f.project, only_opacity(vec![first, second], 600));
        assert_eq!(transform_of(&f, first).opacity, 600);
        assert_eq!(transform_of(&f, second).opacity, 600);
    }

    #[test]
    fn both_halves_of_a_split_keep_the_transform() {
        let mut f = fixture(10);
        let video = insert(&mut f, 0, 0, 48)[0];
        run(&mut f.project, only_opacity(vec![video], 400));
        run(
            &mut f.project,
            EditCommand::SplitClips {
                clip_ids: vec![video],
                at: 24,
            },
        );
        let faded = f
            .project
            .track(f.v1)
            .unwrap()
            .clips
            .iter()
            .filter(|c| c.transform.opacity == 400)
            .count();
        assert_eq!(faded, 2);
    }

    fn lock(p: &mut Project, track_id: TrackId) {
        run(
            p,
            EditCommand::SetTrackState {
                track_id,
                enabled: None,
                locked: Some(true),
            },
        );
    }

    #[test]
    fn insert_creates_linked_video_and_audio() {
        let mut f = fixture(10);
        let created = insert(&mut f, 0, 0, 48);
        assert_eq!(created.len(), 2);
        let (_, v) = f.project.find_clip(created[0]).unwrap();
        let (_, a) = f.project.find_clip(created[1]).unwrap();
        assert!(v.link.is_some());
        assert_eq!(v.link, a.link);
    }

    #[test]
    fn insert_onto_a_new_track_places_both_halves_and_reports_the_track() {
        let mut f = fixture(10);
        // The only audio track is locked, so the UI has no track to name for the sound.
        lock(&mut f.project, f.a1);
        let report = run(
            &mut f.project,
            EditCommand::InsertMedia {
                sequence_id: f.sequence,
                media_id: f.media,
                start: 0,
                source_start: 0,
                duration: Some(48),
                video_track: TrackChoice::Track(f.v1),
                audio_track: TrackChoice::New,
            },
        );
        let a2 = *report.created_tracks.first().expect("made an audio track");
        assert_eq!(report.created_tracks, vec![a2]);
        assert_eq!(f.project.track(a2).unwrap().name, "A2");
        assert_eq!(report.created_clips.len(), 2);
        assert_eq!(spans(&f.project, f.v1), vec![(0, 48, 0)]);
        assert_eq!(spans(&f.project, a2), vec![(0, 48, 0)]);
        assert!(spans(&f.project, f.a1).is_empty());
        // Both halves landed, so they are still a linked pair.
        let link = link_at(&f.project, f.v1, 0);
        assert!(link.is_some());
        assert_eq!(link, link_at(&f.project, a2, 0));
    }

    #[test]
    fn insert_onto_two_new_tracks_keeps_video_above_audio() {
        let mut f = fixture(10);
        let report = run(
            &mut f.project,
            EditCommand::InsertMedia {
                sequence_id: f.sequence,
                media_id: f.media,
                start: 0,
                source_start: 0,
                duration: Some(24),
                video_track: TrackChoice::New,
                audio_track: TrackChoice::New,
            },
        );
        let names: Vec<_> = report
            .created_tracks
            .iter()
            .map(|id| f.project.track(*id).unwrap().name.clone())
            .collect();
        assert_eq!(names, vec!["V2", "A2"]);
        let order: Vec<_> = f
            .project
            .sequence(f.sequence)
            .unwrap()
            .tracks
            .iter()
            .map(|t| t.name.clone())
            .collect();
        assert_eq!(order, vec!["V1", "V2", "A1", "A2"]);
    }

    #[test]
    fn insert_leaves_out_the_stream_it_is_told_to_skip() {
        let mut f = fixture(10);
        let report = run(
            &mut f.project,
            EditCommand::InsertMedia {
                sequence_id: f.sequence,
                media_id: f.media,
                start: 0,
                source_start: 0,
                duration: Some(48),
                video_track: TrackChoice::Track(f.v1),
                audio_track: TrackChoice::Skip,
            },
        );
        assert_eq!(report.created_clips.len(), 1);
        assert!(report.created_tracks.is_empty());
        assert_eq!(spans(&f.project, f.v1), vec![(0, 48, 0)]);
        assert!(spans(&f.project, f.a1).is_empty());
        // Nothing to link a lone picture to.
        assert_eq!(link_at(&f.project, f.v1, 0), None);
    }

    #[test]
    fn insert_rejects_a_stream_the_media_does_not_have() {
        let p = project_with(vec![still_image()]);
        let sequence_id = p.first_sequence().id;
        let a1 = p.first_sequence().tracks[1].id;
        let v1 = p.first_sequence().tracks[0].id;
        let media_id = p.media[0].id;
        let err = apply(
            &p,
            &EditCommand::InsertMedia {
                sequence_id,
                media_id,
                start: 0,
                source_start: 0,
                duration: Some(24),
                video_track: TrackChoice::Track(v1),
                audio_track: TrackChoice::Track(a1),
            },
        )
        .unwrap_err();
        assert!(matches!(err, EditError::InvalidArgument { .. }), "{err}");
    }

    #[test]
    fn insert_rejects_skipping_every_stream() {
        let f = fixture(10);
        let err = apply(
            &f.project,
            &EditCommand::InsertMedia {
                sequence_id: f.sequence,
                media_id: f.media,
                start: 0,
                source_start: 0,
                duration: Some(24),
                video_track: TrackChoice::Skip,
                audio_track: TrackChoice::Skip,
            },
        )
        .unwrap_err();
        assert!(matches!(err, EditError::InvalidArgument { .. }), "{err}");
    }

    #[test]
    fn insert_defaults_to_rest_of_media() {
        let mut f = fixture(10);
        let created = run(
            &mut f.project,
            EditCommand::InsertMedia {
                sequence_id: f.sequence,
                media_id: f.media,
                start: 0,
                source_start: 24,
                duration: None,
                video_track: TrackChoice::Track(f.v1),
                audio_track: TrackChoice::Skip,
            },
        )
        .created_clips;
        assert_eq!(
            f.project.find_clip(created[0]).unwrap().1.duration,
            240 - 24
        );
    }

    #[test]
    fn insert_overwrites_middle_of_existing_clip() {
        let mut f = fixture(10);
        insert(&mut f, 0, 0, 100);
        insert(&mut f, 40, 0, 20);
        // Left remainder, new clip, right remainder with source advanced by 60 frames.
        assert_eq!(
            spans(&f.project, f.v1),
            vec![(0, 40, 0), (40, 20, 0), (60, 40, 60)]
        );
        assert_eq!(
            spans(&f.project, f.a1),
            vec![(0, 40, 0), (40, 20, 0), (60, 40, 60)]
        );
        f.project.validate().unwrap();
    }

    #[test]
    fn overwriting_the_middle_of_a_linked_pair_keeps_both_halves_linked() {
        let mut f = fixture(10);
        insert(&mut f, 0, 0, 100);
        insert(&mut f, 40, 0, 20);
        // Both sides of the pair were split the same way, so each pair of halves is a
        // link group of its own: video and audio stay together on both sides of the drop.
        let left = link_at(&f.project, f.v1, 0);
        let right = link_at(&f.project, f.v1, 60);
        assert!(left.is_some() && right.is_some());
        assert_eq!(left, link_at(&f.project, f.a1, 0));
        assert_eq!(right, link_at(&f.project, f.a1, 60));
        assert_ne!(left, right);
        // The halves are not linked to the clip that was dropped on them either.
        assert_ne!(left, link_at(&f.project, f.v1, 40));
        f.project.validate().unwrap();
    }

    #[test]
    fn overwriting_the_end_of_a_linked_pair_keeps_the_link() {
        let mut f = fixture(10);
        insert(&mut f, 0, 0, 100);
        insert(&mut f, 80, 0, 40);
        assert_eq!(spans(&f.project, f.v1), vec![(0, 80, 0), (80, 40, 0)]);
        assert_eq!(spans(&f.project, f.a1), vec![(0, 80, 0), (80, 40, 0)]);
        let trimmed = link_at(&f.project, f.v1, 0);
        assert!(trimmed.is_some());
        assert_eq!(trimmed, link_at(&f.project, f.a1, 0));
        f.project.validate().unwrap();
    }

    #[test]
    fn overwriting_only_the_video_of_a_pair_unlinks_it() {
        let mut f = fixture(10);
        insert(&mut f, 0, 0, 100);
        // Video only, so the audio under the drop keeps its timing and the pair cannot
        // stay in step. The link is the thing that gives way.
        run(
            &mut f.project,
            EditCommand::InsertMedia {
                sequence_id: f.sequence,
                media_id: f.media,
                start: 40,
                source_start: 0,
                duration: Some(20),
                video_track: TrackChoice::Track(f.v1),
                audio_track: TrackChoice::Skip,
            },
        );
        assert_eq!(
            spans(&f.project, f.v1),
            vec![(0, 40, 0), (40, 20, 0), (60, 40, 60)]
        );
        assert_eq!(spans(&f.project, f.a1), vec![(0, 100, 0)]);
        assert_eq!(link_at(&f.project, f.v1, 0), None);
        assert_eq!(link_at(&f.project, f.a1, 0), None);
        f.project.validate().unwrap();
    }

    #[test]
    fn overwriting_a_pair_whose_audio_is_locked_unlinks_it() {
        let mut f = fixture(10);
        insert(&mut f, 0, 0, 100);
        lock(&mut f.project, f.a1);
        // A locked track is never cut, so the pair cannot be overwritten whole: the video
        // takes the edit and the link dissolves rather than leaving the two out of sync.
        run(
            &mut f.project,
            EditCommand::InsertMedia {
                sequence_id: f.sequence,
                media_id: f.media,
                start: 40,
                source_start: 0,
                duration: Some(20),
                video_track: TrackChoice::Track(f.v1),
                audio_track: TrackChoice::Skip,
            },
        );
        assert_eq!(spans(&f.project, f.a1), vec![(0, 100, 0)]);
        assert_eq!(link_at(&f.project, f.v1, 0), None);
        assert_eq!(link_at(&f.project, f.a1, 0), None);
        f.project.validate().unwrap();
    }

    #[test]
    fn add_sequence_appends_an_empty_one() {
        let mut f = fixture(10);
        let format = f.project.first_sequence().format;
        let report = run(
            &mut f.project,
            EditCommand::AddSequence {
                name: "Sequence 2".into(),
                format,
            },
        );
        let id = report.created_sequence.expect("reports the new sequence");
        assert_eq!(f.project.sequences.len(), 2);
        let added = f.project.sequence(id).expect("findable by id");
        assert_eq!(added.name, "Sequence 2");
        assert!(added.clips().next().is_none(), "starts empty");
        assert_eq!(added.tracks.len(), 2, "starts with V1 and A1");
    }

    #[test]
    fn remove_sequence_refuses_the_last_one() {
        let f = fixture(10);
        let id = f.sequence;
        assert!(apply(&f.project, &EditCommand::RemoveSequence { sequence_id: id }).is_err());
    }

    #[test]
    fn remove_sequence_drops_only_the_named_one() {
        let mut f = fixture(10);
        let format = f.project.first_sequence().format;
        let report = run(
            &mut f.project,
            EditCommand::AddSequence {
                name: "Sequence 2".into(),
                format,
            },
        );
        let added = report.created_sequence.expect("reports the new sequence");
        run(
            &mut f.project,
            EditCommand::RemoveSequence {
                sequence_id: f.sequence,
            },
        );
        assert_eq!(f.project.sequences.len(), 1);
        assert_eq!(f.project.first_sequence().id, added);
    }

    #[test]
    fn a_sequence_rate_is_locked_only_by_its_own_clips() {
        // Clips in another sequence are counted against that sequence's rate, so they
        // have no say in whether this one can change.
        let mut f = fixture(10);
        let format = f.project.first_sequence().format;
        let report = run(
            &mut f.project,
            EditCommand::AddSequence {
                name: "Sequence 2".into(),
                format,
            },
        );
        let empty = report.created_sequence.expect("reports the new sequence");

        // Put a clip in the first sequence only.
        run(
            &mut f.project,
            EditCommand::InsertMedia {
                sequence_id: f.sequence,
                media_id: f.media,
                start: 0,
                source_start: 0,
                duration: Some(24),
                video_track: TrackChoice::Track(f.v1),
                audio_track: TrackChoice::Skip,
            },
        );

        let mut faster = format;
        faster.frame_rate = crate::time::Rational::new(25, 1);
        assert!(
            apply(
                &f.project,
                &EditCommand::SetSequenceFormat {
                    sequence_id: f.sequence,
                    format: faster,
                },
            )
            .is_err(),
            "the sequence holding the clip stays locked"
        );
        assert!(
            apply(
                &f.project,
                &EditCommand::SetSequenceFormat {
                    sequence_id: empty,
                    format: faster,
                },
            )
            .is_ok(),
            "the empty sequence is free to change"
        );
    }

    #[test]
    fn insert_rejects_a_track_from_another_sequence() {
        let mut f = fixture(10);
        let format = f.project.first_sequence().format;
        let report = run(
            &mut f.project,
            EditCommand::AddSequence {
                name: "Sequence 2".into(),
                format,
            },
        );
        let other = report.created_sequence.expect("reports the new sequence");
        // f.v1 belongs to the first sequence, so naming the second is a contradiction.
        let cmd = EditCommand::InsertMedia {
            sequence_id: other,
            media_id: f.media,
            start: 0,
            source_start: 0,
            duration: Some(24),
            video_track: TrackChoice::Track(f.v1),
            audio_track: TrackChoice::Skip,
        };
        assert!(apply(&f.project, &cmd).is_err());
    }

    #[test]
    fn insert_rejects_range_past_media_end() {
        let f = fixture(2);
        let cmd = EditCommand::InsertMedia {
            sequence_id: f.sequence,
            media_id: f.media,
            start: 0,
            source_start: 10,
            duration: Some(48),
            video_track: TrackChoice::Track(f.v1),
            audio_track: TrackChoice::Skip,
        };
        assert!(matches!(
            apply(&f.project, &cmd),
            Err(EditError::InvalidArgument { .. })
        ));
    }

    #[test]
    fn insert_still_image_defaults_to_five_seconds() {
        let mut p = project_with(vec![still_image()]);
        let sequence_id = p.first_sequence().id;
        let v1 = p.first_sequence().tracks[0].id;
        let media_id = p.media[0].id;
        let created = run(
            &mut p,
            EditCommand::InsertMedia {
                sequence_id,
                media_id,
                start: 0,
                source_start: 0,
                duration: None,
                video_track: TrackChoice::Track(v1),
                audio_track: TrackChoice::Skip,
            },
        )
        .created_clips;
        assert_eq!(p.find_clip(created[0]).unwrap().1.duration, 120);
    }

    #[test]
    fn move_carries_linked_audio() {
        let mut f = fixture(10);
        let ids = insert(&mut f, 0, 0, 48);
        run(
            &mut f.project,
            EditCommand::MoveClip {
                clip_id: ids[0],
                start: 100,
                track_id: None,
            },
        );
        assert_eq!(spans(&f.project, f.v1), vec![(100, 48, 0)]);
        assert_eq!(spans(&f.project, f.a1), vec![(100, 48, 0)]);
    }

    #[test]
    fn move_to_another_track_keeps_audio_in_place_and_in_sync() {
        let mut f = fixture(10);
        let ids = insert(&mut f, 0, 0, 48);
        let v2 = run(
            &mut f.project,
            EditCommand::AddTrack {
                sequence_id: f.sequence,
                kind: TrackKind::Video,
            },
        )
        .created_tracks[0];
        run(
            &mut f.project,
            EditCommand::MoveClip {
                clip_id: ids[0],
                start: 10,
                track_id: Some(v2),
            },
        );
        assert_eq!(spans(&f.project, v2), vec![(10, 48, 0)]);
        assert!(spans(&f.project, f.v1).is_empty());
        assert_eq!(spans(&f.project, f.a1), vec![(10, 48, 0)]);
    }

    #[test]
    fn move_onto_a_pair_overwrites_it_whole_and_keeps_its_link() {
        let mut f = fixture(10);
        let first = insert(&mut f, 0, 0, 48);
        let second = insert(&mut f, 100, 0, 48);
        run(
            &mut f.project,
            EditCommand::MoveClip {
                clip_id: second[0],
                start: 24,
                track_id: None,
            },
        );
        assert_eq!(spans(&f.project, f.v1), vec![(0, 24, 0), (24, 48, 0)]);
        assert_eq!(spans(&f.project, f.a1), vec![(0, 24, 0), (24, 48, 0)]);
        // The move cleared both tracks of the pair underneath, so it was trimmed on both
        // sides at once and stays linked.
        let (_, trimmed) = f.project.find_clip(first[0]).unwrap();
        assert!(trimmed.link.is_some());
        assert_eq!(trimmed.link, f.project.find_clip(first[1]).unwrap().1.link);
        let (_, moved) = f.project.find_clip(second[0]).unwrap();
        assert!(moved.link.is_some());
        assert_ne!(moved.link, trimmed.link);
    }

    #[test]
    fn move_onto_one_side_of_a_pair_unlinks_it() {
        let mut f = fixture(10);
        let pair = insert(&mut f, 0, 0, 48);
        // A video-only clip, so moving it clears V1 alone.
        let lone = run(
            &mut f.project,
            EditCommand::InsertMedia {
                sequence_id: f.sequence,
                media_id: f.media,
                start: 100,
                source_start: 0,
                duration: Some(48),
                video_track: TrackChoice::Track(f.v1),
                audio_track: TrackChoice::Skip,
            },
        )
        .created_clips;
        run(
            &mut f.project,
            EditCommand::MoveClip {
                clip_id: lone[0],
                start: 24,
                track_id: None,
            },
        );
        assert_eq!(spans(&f.project, f.v1), vec![(0, 24, 0), (24, 48, 0)]);
        assert_eq!(spans(&f.project, f.a1), vec![(0, 48, 0)]);
        assert_eq!(f.project.find_clip(pair[0]).unwrap().1.link, None);
        assert_eq!(f.project.find_clip(pair[1]).unwrap().1.link, None);
    }

    #[test]
    fn move_to_wrong_kind_is_rejected_without_change() {
        let mut f = fixture(10);
        let ids = insert(&mut f, 0, 0, 48);
        let before = f.project.clone();
        let cmd = EditCommand::MoveClip {
            clip_id: ids[0],
            start: 0,
            track_id: Some(f.a1),
        };
        assert_eq!(
            apply(&f.project, &cmd).unwrap_err(),
            EditError::WrongTrackKind
        );
        assert_eq!(f.project, before);
    }

    #[test]
    fn move_before_zero_is_rejected() {
        let mut f = fixture(10);
        let ids = insert(&mut f, 10, 0, 48);
        let cmd = EditCommand::MoveClip {
            clip_id: ids[0],
            start: -5,
            track_id: None,
        };
        assert!(apply(&f.project, &cmd).is_err());
    }

    #[test]
    fn trim_start_clamps_to_media_start_and_neighbour() {
        let mut f = fixture(10);
        insert(&mut f, 0, 0, 20);
        let ids = insert(&mut f, 50, 30, 40);
        // Media allows reaching back 30 frames (to 20), and the neighbour ends at 20.
        run(
            &mut f.project,
            EditCommand::TrimClip {
                clip_id: ids[0],
                edge: ClipEdge::Start,
                frame: 0,
            },
        );
        assert_eq!(spans(&f.project, f.v1)[1], (20, 70, 0));
        assert_eq!(spans(&f.project, f.a1)[1], (20, 70, 0));
    }

    #[test]
    fn trim_end_clamps_to_media_end() {
        let mut f = fixture(2);
        let ids = insert(&mut f, 0, 0, 10);
        run(
            &mut f.project,
            EditCommand::TrimClip {
                clip_id: ids[0],
                edge: ClipEdge::End,
                frame: 1_000,
            },
        );
        assert_eq!(spans(&f.project, f.v1), vec![(0, 48, 0)]);
    }

    #[test]
    fn trim_never_shrinks_below_one_frame() {
        let mut f = fixture(10);
        let ids = insert(&mut f, 10, 0, 10);
        run(
            &mut f.project,
            EditCommand::TrimClip {
                clip_id: ids[0],
                edge: ClipEdge::End,
                frame: 0,
            },
        );
        assert_eq!(spans(&f.project, f.v1), vec![(10, 1, 0)]);
    }

    #[test]
    fn split_keeps_halves_linked_pairwise() {
        let mut f = fixture(10);
        let ids = insert(&mut f, 0, 0, 100);
        let created = run(
            &mut f.project,
            EditCommand::SplitClips {
                clip_ids: vec![ids[0]],
                at: 40,
            },
        )
        .created_clips;
        assert_eq!(created.len(), 2);
        assert_eq!(spans(&f.project, f.v1), vec![(0, 40, 0), (40, 60, 40)]);
        let left_link = f.project.find_clip(ids[0]).unwrap().1.link;
        let right_link = f.project.find_clip(created[0]).unwrap().1.link;
        assert!(left_link.is_some() && right_link.is_some());
        assert_ne!(left_link, right_link);
        f.project.validate().unwrap();
    }

    #[test]
    fn split_outside_clip_changes_nothing() {
        let mut f = fixture(10);
        let ids = insert(&mut f, 0, 0, 10);
        let before = f.project.clone();
        run(
            &mut f.project,
            EditCommand::SplitClips {
                clip_ids: ids.clone(),
                at: 10,
            },
        );
        assert_eq!(f.project, before);
    }

    #[test]
    fn delete_removes_link_group() {
        let mut f = fixture(10);
        let ids = insert(&mut f, 0, 0, 10);
        run(
            &mut f.project,
            EditCommand::DeleteClips {
                clip_ids: vec![ids[1]],
            },
        );
        assert!(f.project.all_clips().next().is_none());
    }

    #[test]
    fn deleting_one_side_after_unlink_leaves_other() {
        let mut f = fixture(10);
        let ids = insert(&mut f, 0, 0, 10);
        run(
            &mut f.project,
            EditCommand::UnlinkClips {
                clip_ids: vec![ids[0]],
            },
        );
        run(
            &mut f.project,
            EditCommand::DeleteClips {
                clip_ids: vec![ids[0]],
            },
        );
        assert_eq!(spans(&f.project, f.a1), vec![(0, 10, 0)]);
    }

    #[test]
    fn locked_track_rejects_edits() {
        let mut f = fixture(10);
        let ids = insert(&mut f, 0, 0, 10);
        lock(&mut f.project, f.a1);
        // Moving the video moves its linked audio, which sits on a locked track.
        let cmd = EditCommand::MoveClip {
            clip_id: ids[0],
            start: 50,
            track_id: None,
        };
        assert_eq!(
            apply(&f.project, &cmd).unwrap_err(),
            EditError::TrackLocked { track: f.a1 }
        );
    }

    #[test]
    fn remove_media_in_use_is_rejected() {
        let mut f = fixture(10);
        insert(&mut f, 0, 0, 10);
        let cmd = EditCommand::RemoveMedia { media_id: f.media };
        assert!(matches!(
            apply(&f.project, &cmd),
            Err(EditError::MediaInUse { .. })
        ));
    }

    #[test]
    fn import_skips_duplicate_paths() {
        let mut f = fixture(10);
        let mut again = media_with_av(10);
        again.path = f.project.media[0].path.clone();
        let report = run(
            &mut f.project,
            EditCommand::ImportMedia {
                assets: vec![again],
            },
        );
        assert_eq!(f.project.media.len(), 1);
        assert_eq!(report.created_media, vec![f.media]);
    }

    #[test]
    fn frame_rate_is_frozen_once_clips_exist() {
        let mut f = fixture(10);
        insert(&mut f, 0, 0, 10);
        let mut format = f.project.first_sequence().format;
        format.frame_rate = crate::time::Rational::new(25, 1);
        assert!(apply(
            &f.project,
            &EditCommand::SetSequenceFormat {
                sequence_id: f.sequence,
                format,
            }
        )
        .is_err());
        format.frame_rate = f.project.first_sequence().format.frame_rate;
        format.width = 3840;
        format.height = 2160;
        run(
            &mut f.project,
            EditCommand::SetSequenceFormat {
                sequence_id: f.sequence,
                format,
            },
        );
    }

    #[test]
    fn add_video_track_stays_above_existing_video_and_before_audio() {
        let mut f = fixture(10);
        run(
            &mut f.project,
            EditCommand::AddTrack {
                sequence_id: f.sequence,
                kind: TrackKind::Video,
            },
        );
        let names: Vec<_> = f
            .project
            .first_sequence()
            .tracks
            .iter()
            .map(|t| t.name.as_str())
            .collect();
        assert_eq!(names, vec!["V1", "V2", "A1"]);
    }

    #[test]
    fn commands_round_trip_through_json() {
        let cmd = EditCommand::TrimClip {
            clip_id: ClipId::new(),
            edge: ClipEdge::End,
            frame: 12,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"type\":\"TrimClip\""));
        assert!(json.contains("\"clipId\""));
        assert_eq!(serde_json::from_str::<EditCommand>(&json).unwrap(), cmd);
    }
}
