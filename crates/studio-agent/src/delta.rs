//! What an edit actually did, including the parts nobody asked for.
//!
//! `EditReport` says what a command created, which is what the interface needs to select
//! the new thing. It is not what a caller driving the document needs, because several
//! commands change clips they were never given.
//!
//! Placing a clip clears the room it needs: whatever was under it is trimmed, split or
//! removed. Moving one does the same where it lands. Removing a track takes its clips.
//! And a link whose other half has gone is dissolved. A caller told only "one clip
//! created" after an insert that removed three has a picture of the document that is
//! quietly wrong, and every later decision is made against that picture.
//!
//! So this compares the document before and after instead of asking the command what it
//! meant to do. It needs no change to the core, it cannot be out of date with respect
//! to the edit, and it is the same answer whatever command produced it.

use serde::Serialize;
use std::collections::BTreeMap;
use studio_core::ids::{ClipId, TrackId};
use studio_core::model::{Clip, Project};
use studio_core::time::Frame;
use ts_rs::TS;

/// Enough of a clip to say which one, including for one that is no longer there to look
/// up.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClipNote {
    pub id: ClipId,
    pub name: String,
    pub track_id: TrackId,
    #[ts(type = "number")]
    pub start: Frame,
    #[ts(type = "number")]
    pub duration: Frame,
}

impl ClipNote {
    fn of(track: TrackId, clip: &Clip) -> Self {
        Self {
            id: clip.id,
            name: clip.name.clone(),
            track_id: track,
            start: clip.start,
            duration: clip.duration,
        }
    }
}

/// How a clip that is still there differs from how it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Change {
    /// Starts somewhere else.
    Moved,
    /// A different length, so an edge was trimmed.
    Trimmed,
    /// Reads its media from somewhere else.
    Reslipped,
    /// On another track.
    Retracked,
    Renamed,
    /// Turned on or off.
    Toggled,
    /// Moved, scaled, turned or faded.
    Transformed,
    /// No longer tied to the clip it moved with. Worth its own name because nothing
    /// undoes it except undo.
    Unlinked,
    /// Tied to a clip it was not tied to before.
    Linked,
}

/// A clip that is still there, and what is different about it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClipChange {
    pub clip: ClipNote,
    pub changes: Vec<Change>,
}

/// What an edit did.
#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Delta {
    pub clips_created: Vec<ClipNote>,
    /// Clips that are gone, named here because they cannot be looked up any more.
    pub clips_removed: Vec<ClipNote>,
    pub clips_changed: Vec<ClipChange>,
    pub tracks_created: Vec<TrackId>,
    pub tracks_removed: Vec<TrackId>,
    pub media_added: usize,
    pub media_removed: usize,
    pub sequences_added: usize,
    pub sequences_removed: usize,
}

impl Delta {
    /// Whether anything happened at all. A command that changes nothing is not recorded
    /// in history, and this says the same thing to a caller.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Whether the edit reached past what it was given: clips removed or changed that
    /// the caller did not name. The caller knows what it asked for, so it can tell.
    pub fn touched_beyond(&self, named: &[ClipId]) -> bool {
        self.clips_removed.iter().any(|c| !named.contains(&c.id))
            || self
                .clips_changed
                .iter()
                .any(|c| !named.contains(&c.clip.id))
    }
}

fn clips_of(project: &Project) -> BTreeMap<ClipId, (TrackId, Clip)> {
    project
        .all_clips()
        .map(|(track, clip)| (clip.id, (track.id, clip.clone())))
        .collect()
}

/// What changed between two states of the document.
pub fn delta(before: &Project, after: &Project) -> Delta {
    let was = clips_of(before);
    let now = clips_of(after);

    let mut out = Delta {
        clips_created: now
            .iter()
            .filter(|(id, _)| !was.contains_key(*id))
            .map(|(_, (track, clip))| ClipNote::of(*track, clip))
            .collect(),
        clips_removed: was
            .iter()
            .filter(|(id, _)| !now.contains_key(*id))
            .map(|(_, (track, clip))| ClipNote::of(*track, clip))
            .collect(),
        ..Delta::default()
    };

    for (id, (track, clip)) in &now {
        let Some((was_track, old)) = was.get(id) else {
            continue;
        };
        let mut changes = Vec::new();
        if old.start != clip.start {
            changes.push(Change::Moved);
        }
        if old.duration != clip.duration {
            changes.push(Change::Trimmed);
        }
        if old.source.source_start() != clip.source.source_start() {
            changes.push(Change::Reslipped);
        }
        if was_track != track {
            changes.push(Change::Retracked);
        }
        if old.name != clip.name {
            changes.push(Change::Renamed);
        }
        if old.enabled != clip.enabled {
            changes.push(Change::Toggled);
        }
        if old.transform != clip.transform {
            changes.push(Change::Transformed);
        }
        match (old.link, clip.link) {
            (Some(_), None) => changes.push(Change::Unlinked),
            (None, Some(_)) => changes.push(Change::Linked),
            _ => {}
        }
        if !changes.is_empty() {
            out.clips_changed.push(ClipChange {
                clip: ClipNote::of(*track, clip),
                changes,
            });
        }
    }

    let track_ids =
        |project: &Project| -> Vec<TrackId> { project.tracks().map(|t| t.id).collect() };
    let (was_tracks, now_tracks) = (track_ids(before), track_ids(after));
    out.tracks_created = now_tracks
        .iter()
        .filter(|id| !was_tracks.contains(id))
        .copied()
        .collect();
    out.tracks_removed = was_tracks
        .iter()
        .filter(|id| !now_tracks.contains(id))
        .copied()
        .collect();

    out.media_added = after.media.len().saturating_sub(before.media.len());
    out.media_removed = before.media.len().saturating_sub(after.media.len());
    out.sequences_added = after.sequences.len().saturating_sub(before.sequences.len());
    out.sequences_removed = before.sequences.len().saturating_sub(after.sequences.len());

    out
}
