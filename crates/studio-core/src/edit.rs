//! Edit commands: the only way the document changes.
//!
//! [`apply`] runs a command against a copy of the project, normalizes links, validates the
//! result, and returns the new project. If any step fails the original is untouched, so a
//! rejected command is always a no-op. The UI, keyboard shortcuts and a future agent all
//! issue these same commands.
//!
//! Placement onto occupied frames uses overwrite semantics, as in an NLE overwrite edit:
//! clips underneath are trimmed, split or removed to make room.

use crate::ids::{ClipId, LinkId, MediaId, TrackId};
use crate::model::{Clip, ClipSource, MediaAsset, Project, SequenceFormat, Track, TrackKind};
use crate::time::Frame;
use crate::validate::{ValidationError, MAX_NAME_LEN};
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum EditCommand {
    RenameProject {
        name: String,
    },
    /// The frame rate can change only while the timeline is empty, because every stored
    /// frame count is relative to it.
    SetSequenceFormat {
        format: SequenceFormat,
    },
    /// Adds probed assets to the bin. An asset whose path is already in the bin is skipped.
    ImportMedia {
        assets: Vec<MediaAsset>,
    },
    /// Rejected while any clip uses the media.
    RemoveMedia {
        media_id: MediaId,
    },
    AddTrack {
        kind: TrackKind,
    },
    RemoveTrack {
        track_id: TrackId,
    },
    SetTrackState {
        track_id: TrackId,
        enabled: Option<bool>,
        locked: Option<bool>,
    },
    /// Places media on the timeline. Video goes to `video_track` and audio to
    /// `audio_track` when the media has those streams; both together are linked.
    InsertMedia {
        media_id: MediaId,
        #[ts(type = "number")]
        start: Frame,
        #[ts(type = "number")]
        source_start: Frame,
        /// Defaults to the rest of the media, or five seconds for a still image.
        #[ts(type = "number | null")]
        duration: Option<Frame>,
        video_track: Option<TrackId>,
        audio_track: Option<TrackId>,
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
    DeleteClips {
        clip_ids: Vec<ClipId>,
    },
    SetClipsEnabled {
        clip_ids: Vec<ClipId>,
        enabled: bool,
    },
    RenameClip {
        clip_id: ClipId,
        name: String,
    },
    UnlinkClips {
        clip_ids: Vec<ClipId>,
    },
}

impl EditCommand {
    /// Short human label for undo history and logs.
    pub fn label(&self) -> &'static str {
        match self {
            EditCommand::RenameProject { .. } => "Rename project",
            EditCommand::SetSequenceFormat { .. } => "Change sequence settings",
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
    pub created_track: Option<TrackId>,
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
        EditCommand::SetSequenceFormat { format } => {
            if format.frame_rate != p.sequence.format.frame_rate && p.all_clips().next().is_some() {
                return Err(invalid(
                    "frame rate can change only while the timeline is empty",
                ));
            }
            p.sequence.format = *format;
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
        EditCommand::AddTrack { kind } => {
            report.created_track = Some(add_track(p, *kind));
        }
        EditCommand::RemoveTrack { track_id } => {
            ensure_unlocked(p, *track_id)?;
            p.sequence.tracks.retain(|t| t.id != *track_id);
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
            media_id,
            start,
            source_start,
            duration,
            video_track,
            audio_track,
        } => {
            report.created_clips = insert_media(
                p,
                *media_id,
                *start,
                *source_start,
                *duration,
                *video_track,
                *audio_track,
            )?;
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
        EditCommand::UnlinkClips { clip_ids } => {
            let ids = expand_groups(p, clip_ids)?;
            for_each_clip_mut(p, &ids, |c| c.link = None);
        }
    }
    Ok(report)
}

fn checked_name(name: &str) -> Result<String, EditError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_LEN {
        return Err(invalid("name must be 1 to 256 characters"));
    }
    Ok(name.to_string())
}

fn add_track(p: &mut Project, kind: TrackKind) -> TrackId {
    let count = p.sequence.tracks.iter().filter(|t| t.kind == kind).count();
    let prefix = match kind {
        TrackKind::Video => "V",
        TrackKind::Audio => "A",
    };
    let track = Track::new(kind, format!("{prefix}{}", count + 1));
    let id = track.id;
    // Keep video tracks before audio tracks; within a kind, new tracks go last (on top
    // for video, at the bottom for audio).
    let index = match kind {
        TrackKind::Video => p
            .sequence
            .tracks
            .iter()
            .rposition(|t| t.kind == TrackKind::Video)
            .map_or(0, |i| i + 1),
        TrackKind::Audio => p.sequence.tracks.len(),
    };
    p.sequence.tracks.insert(index, track);
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
    for track in &mut p.sequence.tracks {
        for clip in &mut track.clips {
            if wanted.contains(&clip.id) {
                f(clip);
            }
        }
    }
}

fn remove_clip(p: &mut Project, id: ClipId) -> Option<(TrackId, Clip)> {
    for track in &mut p.sequence.tracks {
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

/// Make room for `[start, end)` on a track by trimming, splitting or removing clips.
/// Clips it changes lose their link, since their partners no longer match.
fn clear_range(
    p: &mut Project,
    track_id: TrackId,
    start: Frame,
    end: Frame,
) -> Result<(), EditError> {
    ensure_unlocked(p, track_id)?;
    let mut broken_links = HashSet::new();
    let track = p.track_mut(track_id).expect("checked above");
    let mut kept = Vec::with_capacity(track.clips.len() + 1);
    for mut clip in std::mem::take(&mut track.clips) {
        if clip.end() <= start || clip.start >= end {
            kept.push(clip);
            continue;
        }
        if let Some(link) = clip.link.take() {
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
    dissolve_links(p, &broken_links);
    Ok(())
}

fn dissolve_links(p: &mut Project, links: &HashSet<LinkId>) {
    if links.is_empty() {
        return;
    }
    for track in &mut p.sequence.tracks {
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
    media_id: MediaId,
    start: Frame,
    source_start: Frame,
    duration: Option<Frame>,
    video_track: Option<TrackId>,
    audio_track: Option<TrackId>,
) -> Result<Vec<ClipId>, EditError> {
    let media = p
        .media(media_id)
        .ok_or_else(|| not_found("media", media_id))?;
    let rate = p.sequence.format.frame_rate;
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

    let mut targets = Vec::new();
    if let (true, Some(track)) = (media.has_video(), video_track) {
        targets.push((track, TrackKind::Video));
    }
    if let (true, Some(track)) = (media.has_audio(), audio_track) {
        targets.push((track, TrackKind::Audio));
    }
    if targets.is_empty() {
        return Err(invalid("no track can hold this media"));
    }
    let name = media.name.clone();
    for (track_id, kind) in &targets {
        let track = ensure_unlocked(p, *track_id)?;
        if track.kind != *kind {
            return Err(EditError::WrongTrackKind);
        }
    }

    let link = (targets.len() > 1).then(LinkId::new);
    let mut created = Vec::new();
    for (track_id, _) in targets {
        clear_range(p, track_id, start, start + duration)?;
        let clip = Clip {
            id: ClipId::new(),
            name: name.clone(),
            start,
            duration,
            enabled: true,
            link,
            source: ClipSource::Media {
                media_id,
                source_start,
            },
        };
        created.push(clip.id);
        insert_sorted(p.track_mut(track_id).expect("checked"), clip);
    }
    Ok(created)
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
    for (track_id, member) in &moving {
        clear_range(p, *track_id, member.start, member.end())?;
    }
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
    let rate = p.sequence.format.frame_rate;
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
        v1: TrackId,
        a1: TrackId,
    }

    fn fixture(seconds: i64) -> Fixture {
        let project = project_with(vec![media_with_av(seconds)]);
        Fixture {
            media: project.media[0].id,
            v1: project.sequence.tracks[0].id,
            a1: project.sequence.tracks[1].id,
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
                media_id: f.media,
                start,
                source_start,
                duration: Some(duration),
                video_track: Some(f.v1),
                audio_track: Some(f.a1),
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
    fn insert_defaults_to_rest_of_media() {
        let mut f = fixture(10);
        let created = run(
            &mut f.project,
            EditCommand::InsertMedia {
                media_id: f.media,
                start: 0,
                source_start: 24,
                duration: None,
                video_track: Some(f.v1),
                audio_track: None,
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
    fn insert_rejects_range_past_media_end() {
        let f = fixture(2);
        let cmd = EditCommand::InsertMedia {
            media_id: f.media,
            start: 0,
            source_start: 10,
            duration: Some(48),
            video_track: Some(f.v1),
            audio_track: None,
        };
        assert!(matches!(
            apply(&f.project, &cmd),
            Err(EditError::InvalidArgument { .. })
        ));
    }

    #[test]
    fn insert_still_image_defaults_to_five_seconds() {
        let mut p = project_with(vec![still_image()]);
        let v1 = p.sequence.tracks[0].id;
        let media_id = p.media[0].id;
        let created = run(
            &mut p,
            EditCommand::InsertMedia {
                media_id,
                start: 0,
                source_start: 0,
                duration: None,
                video_track: Some(v1),
                audio_track: None,
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
                kind: TrackKind::Video,
            },
        )
        .created_track
        .unwrap();
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
    fn move_onto_clip_overwrites_and_unlinks_the_victim() {
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
        // Overwrite breaks the link on every clip it touches, even when both sides of the
        // pair were trimmed identically, because the edit was not made to the pair.
        let (_, trimmed) = f.project.find_clip(first[0]).unwrap();
        assert_eq!(trimmed.link, None);
        let (_, moved) = f.project.find_clip(second[0]).unwrap();
        assert!(moved.link.is_some());
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
        run(
            &mut f.project,
            EditCommand::SetTrackState {
                track_id: f.a1,
                enabled: None,
                locked: Some(true),
            },
        );
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
        let mut format = f.project.sequence.format;
        format.frame_rate = crate::time::Rational::new(25, 1);
        assert!(apply(&f.project, &EditCommand::SetSequenceFormat { format }).is_err());
        format.frame_rate = f.project.sequence.format.frame_rate;
        format.width = 3840;
        format.height = 2160;
        run(&mut f.project, EditCommand::SetSequenceFormat { format });
    }

    #[test]
    fn add_video_track_stays_above_existing_video_and_before_audio() {
        let mut f = fixture(10);
        run(
            &mut f.project,
            EditCommand::AddTrack {
                kind: TrackKind::Video,
            },
        );
        let names: Vec<_> = f
            .project
            .sequence
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
