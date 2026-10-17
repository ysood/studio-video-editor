//! Fixtures shared by unit tests across modules.

use crate::ids::MediaId;
use crate::model::{
    AudioInfo, Fingerprint, MediaAsset, MediaKind, Project, SequenceFormat, VideoInfo,
};
use crate::time::Rational;

/// Video with stereo audio, 1920x1080 at 24 fps, `seconds` long.
pub fn media_with_av(seconds: i64) -> MediaAsset {
    MediaAsset {
        id: MediaId::new(),
        name: "shot.mov".into(),
        path: "/footage/shot.mov".into(),
        kind: MediaKind::Video,
        duration_us: Some(seconds * 1_000_000),
        video: Some(VideoInfo {
            width: 1920,
            height: 1080,
            frame_rate: Rational::new(24, 1),
            codec: "prores".into(),
        }),
        audio: Some(AudioInfo {
            sample_rate: 48_000,
            channels: 2,
            codec: "pcm_s24le".into(),
        }),
        fingerprint: Fingerprint {
            size_bytes: 1,
            modified_unix_ms: 0,
        },
    }
}

pub fn still_image() -> MediaAsset {
    MediaAsset {
        name: "title.png".into(),
        path: "/footage/title.png".into(),
        kind: MediaKind::Image,
        duration_us: None,
        audio: None,
        ..media_with_av(1)
    }
}

/// A 24 fps 1080p project (V1 and A1) containing `media`.
pub fn project_with(media: Vec<MediaAsset>) -> Project {
    let mut project = Project::new("Test", SequenceFormat::default());
    project.media = media;
    project
}

/// Two linked video+audio clips: frames 12..60 (source from 6) and 100..124 (source 0),
/// so V1 and A1 each read gap, clip, gap, clip.
pub fn sample_edit() -> Project {
    use crate::edit::{apply, EditCommand};
    let mut project = project_with(vec![media_with_av(10)]);
    let media_id = project.media[0].id;
    let sequence_id = project.first_sequence().id;
    let (v1, a1) = (
        project.first_sequence().tracks[0].id,
        project.first_sequence().tracks[1].id,
    );
    for (start, source_start, duration) in [(12, 6, 48), (100, 0, 24)] {
        let command = EditCommand::InsertMedia {
            sequence_id,
            media_id,
            start,
            source_start,
            duration: Some(duration),
            video_track: Some(v1),
            audio_track: Some(a1),
        };
        project = apply(&project, &command).unwrap().0;
    }
    project
}
