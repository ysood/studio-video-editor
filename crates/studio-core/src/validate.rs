//! Document invariants. Every edit and every load runs [`Project::validate`]; a document
//! that fails is never committed or opened.

use crate::ids::{ClipId, LinkId, MediaId};
use crate::model::{ClipSource, MediaKind, Project, TrackKind, SCHEMA_VERSION};
use crate::time::Frame;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use ts_rs::TS;

pub const MAX_NAME_LEN: usize = 256;
pub const MAX_DIMENSION: u32 = 8192;
pub const SAMPLE_RATES: [u32; 3] = [44_100, 48_000, 96_000];

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, TS)]
#[serde(
    tag = "code",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ValidationError {
    #[error("schema version {found} is not {expected}")]
    SchemaVersion { found: u32, expected: u32 },
    #[error("duplicate id {id}")]
    DuplicateId { id: String },
    #[error("invalid sequence format: {reason}")]
    InvalidFormat { reason: String },
    #[error("name longer than {MAX_NAME_LEN} characters")]
    NameTooLong,
    #[error("clip {clip} has an invalid range")]
    InvalidRange { clip: ClipId },
    #[error("clips {first} and {second} overlap")]
    Overlap { first: ClipId, second: ClipId },
    #[error("clip {clip} references missing media {media}")]
    MissingMedia { clip: ClipId, media: MediaId },
    #[error("clip {clip} does not match its track kind")]
    KindMismatch { clip: ClipId },
    #[error("clip {clip} reads past the end of its media")]
    SourceOutOfRange { clip: ClipId },
    #[error("link {link} is inconsistent")]
    InvalidLink { link: LinkId },
    #[error("media {media} has no usable stream")]
    InvalidMedia { media: MediaId },
}

impl Project {
    /// Check every invariant in `docs/schema.md`. Returns the first violation found.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ValidationError::SchemaVersion {
                found: self.schema_version,
                expected: SCHEMA_VERSION,
            });
        }
        check_name(&self.name)?;
        check_name(&self.sequence.name)?;
        self.validate_format()?;
        self.validate_unique_ids()?;
        self.validate_media()?;
        self.validate_tracks()?;
        self.validate_links()
    }

    fn validate_format(&self) -> Result<(), ValidationError> {
        let f = &self.sequence.format;
        let fail = |reason: &str| {
            Err(ValidationError::InvalidFormat {
                reason: reason.into(),
            })
        };
        if !f.frame_rate.is_valid() || f.frame_rate.as_f64() > 240.0 {
            return fail("frame rate must be positive and at most 240");
        }
        // H.264 and ProRes 4:2:2 need even dimensions.
        if f.width == 0
            || f.height == 0
            || !f.width.is_multiple_of(2)
            || !f.height.is_multiple_of(2)
        {
            return fail("width and height must be positive and even");
        }
        if f.width > MAX_DIMENSION || f.height > MAX_DIMENSION {
            return fail("width and height must be at most 8192");
        }
        if !SAMPLE_RATES.contains(&f.sample_rate) {
            return fail("sample rate must be 44100, 48000 or 96000");
        }
        Ok(())
    }

    fn validate_unique_ids(&self) -> Result<(), ValidationError> {
        let mut seen = HashSet::new();
        let ids = std::iter::once(self.id.0)
            .chain(std::iter::once(self.sequence.id.0))
            .chain(self.media.iter().map(|m| m.id.0))
            .chain(self.sequence.tracks.iter().map(|t| t.id.0))
            .chain(self.all_clips().map(|(_, c)| c.id.0));
        for id in ids {
            if !seen.insert(id) {
                return Err(ValidationError::DuplicateId { id: id.to_string() });
            }
        }
        Ok(())
    }

    fn validate_media(&self) -> Result<(), ValidationError> {
        for m in &self.media {
            check_name(&m.name)?;
            let usable = match m.kind {
                MediaKind::Video | MediaKind::Image => m.video.is_some(),
                MediaKind::Audio => m.audio.is_some() && m.video.is_none(),
            };
            let timed = m.kind == MediaKind::Image || m.duration_us.is_some_and(|d| d > 0);
            if !usable || !timed || m.path.is_empty() {
                return Err(ValidationError::InvalidMedia { media: m.id });
            }
        }
        Ok(())
    }

    fn validate_tracks(&self) -> Result<(), ValidationError> {
        let rate = self.sequence.format.frame_rate;
        for track in &self.sequence.tracks {
            check_name(&track.name)?;
            let mut previous_end: Option<(ClipId, Frame)> = None;
            for clip in &track.clips {
                check_name(&clip.name)?;
                if clip.start < 0 || clip.duration < 1 {
                    return Err(ValidationError::InvalidRange { clip: clip.id });
                }
                if let Some((prev_id, prev_end)) = previous_end {
                    if clip.start < prev_end {
                        return Err(ValidationError::Overlap {
                            first: prev_id,
                            second: clip.id,
                        });
                    }
                }
                previous_end = Some((clip.id, clip.end()));

                let ClipSource::Media {
                    media_id,
                    source_start,
                } = clip.source;
                let media = self.media(media_id).ok_or(ValidationError::MissingMedia {
                    clip: clip.id,
                    media: media_id,
                })?;
                let kind_ok = match track.kind {
                    TrackKind::Video => media.has_video(),
                    TrackKind::Audio => media.has_audio(),
                };
                if !kind_ok {
                    return Err(ValidationError::KindMismatch { clip: clip.id });
                }
                if source_start < 0 {
                    return Err(ValidationError::InvalidRange { clip: clip.id });
                }
                if let Some(available) = media.frames_at(rate) {
                    if source_start + clip.duration > available {
                        return Err(ValidationError::SourceOutOfRange { clip: clip.id });
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_links(&self) -> Result<(), ValidationError> {
        let mut groups: HashMap<LinkId, Vec<_>> = HashMap::new();
        for (track, clip) in self.all_clips() {
            if let Some(link) = clip.link {
                groups.entry(link).or_default().push((track.id, clip));
            }
        }
        for (link, members) in groups {
            let first = members[0].1;
            let mut tracks = HashSet::new();
            let consistent = members.len() >= 2
                && members.iter().all(|(track_id, c)| {
                    tracks.insert(*track_id)
                        && c.start == first.start
                        && c.duration == first.duration
                        && c.source == first.source
                });
            if !consistent {
                return Err(ValidationError::InvalidLink { link });
            }
        }
        Ok(())
    }
}

fn check_name(name: &str) -> Result<(), ValidationError> {
    if name.chars().count() > MAX_NAME_LEN {
        Err(ValidationError::NameTooLong)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Clip, SequenceFormat};
    use crate::testing::{media_with_av, project_with, still_image};

    fn clip_on(project: &mut Project, track_index: usize, start: Frame, duration: Frame) -> ClipId {
        let media_id = project.media[0].id;
        let clip = Clip {
            id: ClipId::new(),
            name: "c".into(),
            start,
            duration,
            enabled: true,
            link: None,
            source: ClipSource::Media {
                media_id,
                source_start: 0,
            },
        };
        let id = clip.id;
        let clips = &mut project.sequence.tracks[track_index].clips;
        clips.push(clip);
        clips.sort_by_key(|c| c.start);
        id
    }

    #[test]
    fn new_project_is_valid() {
        Project::new("Untitled", SequenceFormat::default())
            .validate()
            .unwrap();
    }

    #[test]
    fn rejects_overlapping_clips() {
        let mut p = project_with(vec![media_with_av(10)]);
        clip_on(&mut p, 0, 0, 48);
        clip_on(&mut p, 0, 24, 48);
        assert!(matches!(p.validate(), Err(ValidationError::Overlap { .. })));
    }

    #[test]
    fn rejects_clip_past_media_end() {
        let mut p = project_with(vec![media_with_av(2)]);
        clip_on(&mut p, 0, 0, 49); // 2 s at 24 fps is 48 frames
        assert!(matches!(
            p.validate(),
            Err(ValidationError::SourceOutOfRange { .. })
        ));
    }

    #[test]
    fn rejects_odd_dimensions() {
        let mut p = project_with(vec![]);
        p.sequence.format.width = 1921;
        assert!(matches!(
            p.validate(),
            Err(ValidationError::InvalidFormat { .. })
        ));
    }

    #[test]
    fn rejects_singleton_link() {
        let mut p = project_with(vec![media_with_av(10)]);
        clip_on(&mut p, 0, 0, 24);
        p.sequence.tracks[0].clips[0].link = Some(LinkId::new());
        assert!(matches!(
            p.validate(),
            Err(ValidationError::InvalidLink { .. })
        ));
    }

    #[test]
    fn rejects_audio_only_media_on_video_track() {
        let mut audio = media_with_av(10);
        audio.video = None;
        audio.kind = MediaKind::Audio;
        let mut p = project_with(vec![audio]);
        clip_on(&mut p, 0, 0, 24);
        assert!(matches!(
            p.validate(),
            Err(ValidationError::KindMismatch { .. })
        ));
    }

    #[test]
    fn still_image_has_no_length_limit() {
        let mut p = project_with(vec![still_image()]);
        clip_on(&mut p, 0, 0, 24 * 600);
        p.validate().unwrap();
    }
}
