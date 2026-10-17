//! Everything that leaves the document: interchange files for other editors.
//!
//! Exporters read a validated [`crate::Project`] and never change it. Absolute-positioned
//! clips are converted to the gap-and-clip sequences that interchange formats expect.

pub mod fcp7;
pub mod otio;
pub mod render;
mod url;

pub use url::file_url;

use crate::model::{Clip, Track};
use crate::time::Frame;

/// A track's contents as a run of gaps and clips starting at frame zero.
pub(crate) enum Item<'a> {
    Gap(Frame),
    Clip(&'a Clip),
}

pub(crate) fn sequence_items(track: &Track) -> Vec<Item<'_>> {
    let mut items = Vec::with_capacity(track.clips.len() * 2);
    let mut cursor = 0;
    for clip in &track.clips {
        if clip.start > cursor {
            items.push(Item::Gap(clip.start - cursor));
        }
        items.push(Item::Clip(clip));
        cursor = clip.end();
    }
    items
}
