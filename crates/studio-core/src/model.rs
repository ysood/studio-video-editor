//! The project document. `docs/schema.md` describes these types and their invariants;
//! keep the two in step.

use crate::ids::{ClipId, LinkId, MediaId, ProjectId, SequenceId, TrackId};
use crate::time::{Frame, Micros, Rational};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Bump when a persisted type changes, and add a migration in `persist.rs`.
pub const SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Project {
    pub schema_version: u32,
    pub id: ProjectId,
    pub name: String,
    pub media: Vec<MediaAsset>,
    /// At least one, in the order they appear in the bin. A command names the sequence
    /// it acts on, so nothing here records which one a window happens to show (ADR-0004).
    pub sequences: Vec<Sequence>,
}

impl Project {
    /// A new project with one empty video track and one empty audio track.
    pub fn new(name: impl Into<String>, format: SequenceFormat) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            id: ProjectId::new(),
            name: name.into(),
            media: Vec::new(),
            sequences: vec![Sequence::new("Sequence 1", format)],
        }
    }

    /// The sequence a command named. Ids are unique project wide, so this cannot be
    /// ambiguous.
    pub fn sequence(&self, id: SequenceId) -> Option<&Sequence> {
        self.sequences.iter().find(|s| s.id == id)
    }

    pub fn sequence_mut(&mut self, id: SequenceId) -> Option<&mut Sequence> {
        self.sequences.iter_mut().find(|s| s.id == id)
    }

    /// The sequence to show when nothing has said which, such as a project just opened.
    /// Validation guarantees there is at least one.
    pub fn first_sequence(&self) -> &Sequence {
        self.sequences.first().expect("a project has a sequence")
    }

    /// Mutable counterpart of `first_sequence`, for fixtures and tests that work with
    /// the sequence a new project starts with. Commands name their sequence instead.
    pub fn first_sequence_mut(&mut self) -> &mut Sequence {
        self.sequences
            .first_mut()
            .expect("a project has a sequence")
    }

    /// The sequence holding a track. A command that names a track does not need to name
    /// a sequence as well: ids are unique, so the track already says which one.
    pub fn sequence_of_track(&self, id: TrackId) -> Option<&Sequence> {
        self.sequences
            .iter()
            .find(|s| s.tracks.iter().any(|t| t.id == id))
    }

    pub fn sequence_of_track_mut(&mut self, id: TrackId) -> Option<&mut Sequence> {
        self.sequences
            .iter_mut()
            .find(|s| s.tracks.iter().any(|t| t.id == id))
    }

    /// The sequence holding a clip, and so the rate its frame counts are measured in.
    pub fn sequence_of_clip(&self, id: ClipId) -> Option<&Sequence> {
        self.sequences
            .iter()
            .find(|s| s.clips().any(|(_, c)| c.id == id))
    }

    /// Every track in the project, mutably.
    pub fn tracks_mut(&mut self) -> impl Iterator<Item = &mut Track> {
        self.sequences.iter_mut().flat_map(|s| s.tracks.iter_mut())
    }

    pub fn media(&self, id: MediaId) -> Option<&MediaAsset> {
        self.media.iter().find(|m| m.id == id)
    }

    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.tracks().find(|t| t.id == id)
    }

    pub fn track_mut(&mut self, id: TrackId) -> Option<&mut Track> {
        self.sequences
            .iter_mut()
            .flat_map(|s| s.tracks.iter_mut())
            .find(|t| t.id == id)
    }

    /// Every track in the project, across sequences.
    pub fn tracks(&self) -> impl Iterator<Item = &Track> {
        self.sequences.iter().flat_map(|s| s.tracks.iter())
    }

    /// The clip and the track that holds it.
    pub fn find_clip(&self, id: ClipId) -> Option<(&Track, &Clip)> {
        self.tracks()
            .find_map(|t| t.clips.iter().find(|c| c.id == id).map(|c| (t, c)))
    }

    /// Clip ids of the clip's link group, including the clip itself.
    pub fn link_group(&self, id: ClipId) -> Vec<ClipId> {
        let Some((_, clip)) = self.find_clip(id) else {
            return Vec::new();
        };
        match clip.link {
            None => vec![id],
            Some(link) => self
                .all_clips()
                .filter(|(_, c)| c.link == Some(link))
                .map(|(_, c)| c.id)
                .collect(),
        }
    }

    pub fn all_clips(&self) -> impl Iterator<Item = (&Track, &Clip)> {
        self.tracks()
            .flat_map(|t| t.clips.iter().map(move |c| (t, c)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum MediaKind {
    /// Has a video stream, and possibly audio.
    Video,
    /// Audio only.
    Audio,
    /// A single still frame with no inherent duration.
    Image,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MediaAsset {
    pub id: MediaId,
    /// Display name, initially the file name.
    pub name: String,
    /// Absolute path on the machine that imported it.
    pub path: String,
    pub kind: MediaKind,
    /// `None` for still images.
    #[ts(type = "number | null")]
    pub duration_us: Option<Micros>,
    pub video: Option<VideoInfo>,
    pub audio: Option<AudioInfo>,
    /// Detects a file replaced on disk since import.
    pub fingerprint: Fingerprint,
}

impl MediaAsset {
    pub fn has_video(&self) -> bool {
        self.video.is_some()
    }

    pub fn has_audio(&self) -> bool {
        self.audio.is_some()
    }

    /// Length in frames at `rate`, or `None` when the media has no inherent duration.
    pub fn frames_at(&self, rate: Rational) -> Option<Frame> {
        match self.kind {
            MediaKind::Image => None,
            _ => self.duration_us.map(|d| rate.micros_to_frames_floor(d)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub frame_rate: Rational,
    pub codec: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AudioInfo {
    pub sample_rate: u32,
    pub channels: u32,
    pub codec: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Fingerprint {
    #[ts(type = "number")]
    pub size_bytes: u64,
    #[ts(type = "number")]
    pub modified_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Sequence {
    pub id: SequenceId,
    pub name: String,
    pub format: SequenceFormat,
    /// Video tracks bottom to top, then audio tracks top to bottom, in one list. Order
    /// within a kind is stacking order: a later video track covers an earlier one.
    pub tracks: Vec<Track>,
}

impl Sequence {
    /// A sequence with one empty video track and one empty audio track.
    pub fn new(name: impl Into<String>, format: SequenceFormat) -> Self {
        Self {
            id: SequenceId::new(),
            name: name.into(),
            format,
            tracks: vec![
                Track::new(TrackKind::Video, "V1"),
                Track::new(TrackKind::Audio, "A1"),
            ],
        }
    }

    pub fn clips(&self) -> impl Iterator<Item = (&Track, &Clip)> {
        self.tracks
            .iter()
            .flat_map(|t| t.clips.iter().map(move |c| (t, c)))
    }

    /// Frame after the last clip on any track; zero for an empty sequence.
    pub fn duration(&self) -> Frame {
        self.clips().map(|(_, c)| c.end()).max().unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SequenceFormat {
    pub width: u32,
    pub height: u32,
    pub frame_rate: Rational,
    pub sample_rate: u32,
}

impl Default for SequenceFormat {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            frame_rate: Rational::new(24, 1),
            sample_rate: 48_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum TrackKind {
    Video,
    Audio,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Track {
    pub id: TrackId,
    pub kind: TrackKind,
    pub name: String,
    /// Disabled tracks are not rendered, heard or exported as active.
    pub enabled: bool,
    /// Locked tracks reject every edit that would change their clips.
    pub locked: bool,
    /// Sorted by `start`, never overlapping.
    pub clips: Vec<Clip>,
}

impl Track {
    pub fn new(kind: TrackKind, name: impl Into<String>) -> Self {
        Self {
            id: TrackId::new(),
            kind,
            name: name.into(),
            enabled: true,
            locked: false,
            clips: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Clip {
    pub id: ClipId,
    pub name: String,
    /// First frame on the timeline.
    #[ts(type = "number")]
    pub start: Frame,
    /// Length on the timeline, at least one frame.
    #[ts(type = "number")]
    pub duration: Frame,
    pub enabled: bool,
    /// Clips sharing a link move, trim, split and delete together and must share
    /// `start`, `duration` and source range. A transform is not shared: it belongs to
    /// the picture, and the audio half of a pair has none.
    pub link: Option<LinkId>,
    /// Where the picture sits in the frame. Identity on an audio clip.
    pub transform: Transform,
    pub source: ClipSource,
}

impl Clip {
    /// Frame after the clip's last frame.
    pub fn end(&self) -> Frame {
        self.start + self.duration
    }

    pub fn contains(&self, frame: Frame) -> bool {
        frame >= self.start && frame < self.end()
    }
}

/// A transform quantity in thousandths of its unit.
///
/// The document keeps no floats (`docs/schema.md`), so a transform is exact, comparable
/// and diffable: undo and dirty tracking compare whole documents, which float drift from
/// a dragged slider would make unreliable. It becomes a float only where it leaves the
/// document, the same treatment `Frame` gets. Thousandths throughout, so one divisor
/// reads every field.
pub type Milli = i32;

/// Thousandths in one unit: one pixel, one degree, or a factor of one.
pub const MILLI: Milli = 1000;

/// A thousandths value as the unit it counts. Only for tool boundaries.
pub fn units(value: Milli) -> f64 {
    f64::from(value) / f64::from(MILLI)
}

/// A point in the frame, as an offset from its centre in thousandths of a pixel.
///
/// Centre relative rather than absolute, which is how Resolve measures pan and tilt and
/// unlike Premiere's Position. Two reasons: the identity transform is then all zeros, and
/// a centred clip stays centred when the sequence is resized, so no default has to be
/// recomputed when the format changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Offset {
    /// Positive is right.
    pub x: Milli,
    /// Positive is down.
    pub y: Milli,
}

/// Horizontal and vertical size as thousandths of the fitted picture.
///
/// Its own type rather than an `Offset`: the unit is a factor, not a pixel, and one is
/// the identity rather than zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Scale {
    pub x: Milli,
    pub y: Milli,
}

/// Where a clip's picture sits in the sequence frame, and how opaque it is.
///
/// Applied after the clip has been fitted to the frame the way the renderer fits it
/// (scaled to fit without cropping, centred), so the same numbers mean the same picture
/// whatever the source resolution is, and an identity transform is exactly today's
/// behaviour. Order: scale, then rotate, both about `anchor`, then translate by
/// `position`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Transform {
    /// Where the anchor lands, from the centre of the frame.
    pub position: Offset,
    /// Size as a factor of the fitted picture: `MILLI` leaves it alone. Always positive,
    /// so there is no flip here; that is its own effect when it arrives.
    pub scale: Scale,
    /// Clockwise rotation in thousandths of a degree.
    pub rotation: Milli,
    /// The point scale and rotation turn about, from the centre of the fitted picture.
    pub anchor: Offset,
    /// `MILLI` is opaque, zero invisible.
    pub opacity: Milli,
}

impl Transform {
    /// The picture the renderer drew before transforms existed: fitted, centred, opaque.
    pub const IDENTITY: Self = Self {
        position: Offset { x: 0, y: 0 },
        scale: Scale { x: MILLI, y: MILLI },
        rotation: 0,
        anchor: Offset { x: 0, y: 0 },
        opacity: MILLI,
    };

    pub fn is_identity(self) -> bool {
        self == Self::IDENTITY
    }

    /// True when the clip fills the frame and hides whatever is under it, which lets a
    /// compositor skip the layers below. Conservative: it answers no whenever a
    /// transform is present at all, because a scaled or rotated clip may still cover the
    /// frame and proving it is not worth the arithmetic.
    pub fn is_opaque_fill(self) -> bool {
        self.is_identity()
    }

    /// Position in frame pixels, right and down from the centre.
    pub fn position_px(self) -> (f64, f64) {
        (units(self.position.x), units(self.position.y))
    }

    /// Anchor in pixels of the fitted picture, right and down from its centre.
    pub fn anchor_px(self) -> (f64, f64) {
        (units(self.anchor.x), units(self.anchor.y))
    }

    /// Size as a plain factor, where 1.0 is the fitted picture.
    pub fn scale_factor(self) -> (f64, f64) {
        (units(self.scale.x), units(self.scale.y))
    }

    pub fn rotation_degrees(self) -> f64 {
        units(self.rotation)
    }

    pub fn rotation_radians(self) -> f64 {
        self.rotation_degrees().to_radians()
    }

    /// Alpha as a plain factor, where 1.0 is opaque.
    pub fn opacity_factor(self) -> f64 {
        units(self.opacity)
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// What a clip plays. Generated (AI) and 3D sources are planned as further variants; they
/// will bake to media before export so interchange stays media-only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ClipSource {
    Media {
        media_id: MediaId,
        /// Offset into the media where the clip begins, in sequence frames.
        #[ts(type = "number")]
        source_start: Frame,
    },
}

impl ClipSource {
    pub fn media_id(&self) -> MediaId {
        match self {
            ClipSource::Media { media_id, .. } => *media_id,
        }
    }

    pub fn source_start(&self) -> Frame {
        match self {
            ClipSource::Media { source_start, .. } => *source_start,
        }
    }

    pub fn with_source_start(&self, value: Frame) -> Self {
        match self {
            ClipSource::Media { media_id, .. } => ClipSource::Media {
                media_id: *media_id,
                source_start: value,
            },
        }
    }
}
