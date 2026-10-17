//! Native macOS preview surface (ADR-0003).
//!
//! The webview cannot play our footage: Tauri's asset protocol stalls on high bitrate
//! H.264 on macOS, and a `<video>` element could never composite tracks anyway. Instead
//! the picture comes from AVFoundation, which is VideoToolbox backed and frame accurate.
//!
//! `studio-core` still owns the document. Everything here is derived from a validated
//! project and thrown away on the next sync, so the composition can never be the source
//! of truth for an edit.
//!
//! Threading: AppKit and AVFoundation are main thread only, and `Retained<_>` is not
//! `Send`, so the surface lives in a main thread local. Commands compute plain data on
//! their own thread and hand it to `run_on_main_thread`.

// The only module in the workspace that may use unsafe: every AppKit and AVFoundation
// call is an Objective-C message send, which Rust cannot verify. Kept to this file so the
// rest of the shell, and all of studio-core, stay safe.
#![allow(unsafe_code)]

use std::cell::RefCell;
use std::ffi::c_void;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSView, NSWindow, NSWindowOrderingMode};
use objc2_av_foundation::{
    AVMediaType, AVMediaTypeAudio, AVMediaTypeVideo, AVMutableComposition,
    AVMutableCompositionTrack, AVMutableVideoComposition, AVMutableVideoCompositionInstruction,
    AVMutableVideoCompositionLayerInstruction, AVPlayer, AVPlayerItem, AVPlayerLayer, AVURLAsset,
    AVVideoCompositionLayerInstruction,
};
use objc2_core_foundation::{CGAffineTransform, CGSize};
use objc2_core_media::{CMTime, CMTimeFlags, CMTimeRange};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSSize, NSString, NSURL};

use studio_core::model::{MediaKind, Project, Sequence, TrackKind};
use studio_core::time::Rational;

/// Composition times are built in microseconds, matching `Micros` in the document, so a
/// frame boundary never lands between two representable instants.
const TIMESCALE: i32 = 1_000_000;

thread_local! {
    static SURFACE: RefCell<Option<Surface>> = const { RefCell::new(None) };
}

/// One piece of media on the preview timeline. Plain data so it can cross threads.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub path: String,
    /// Position on the sequence timeline.
    pub start_us: i64,
    pub duration_us: i64,
    /// Offset into the source file.
    pub source_start_us: i64,
    pub audio: bool,
    /// Which composition track this goes on. Audio clips may overlap, video never does.
    pub lane: usize,
}

/// The sequence's picture: the frame the preview renders into.
///
/// The renderer fits every clip into this frame and pads the rest black, so the preview
/// has to do the same or what plays is not what exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub rate: Rational,
}

impl Canvas {
    pub fn of(sequence: &Sequence) -> Self {
        Self {
            width: sequence.format.width,
            height: sequence.format.height,
            rate: sequence.format.frame_rate,
        }
    }
}

/// Flatten one sequence into non-overlapping video segments plus one lane per audio track.
///
/// The sequence is passed in rather than looked up, because the preview shows whichever
/// sequence is open and the document does not record that (ADR-0004).
///
/// Video mirrors `playback::resolve` and `export::render`: the highest enabled video track
/// with an enabled clip wins, so the flattened result has no overlaps and fits one
/// composition track. Audio keeps its lanes because several clips can sound at once.
pub fn segments(project: &Project, sequence: &Sequence) -> Vec<Segment> {
    let rate = sequence.format.frame_rate;
    let mut out = Vec::new();

    let video: Vec<_> = sequence
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Video && t.enabled)
        .collect();
    // Bottom to top, so a higher track's clip overwrites what is already there.
    let mut covered: Vec<(i64, i64)> = Vec::new();
    for track in video.iter().rev() {
        for clip in track.clips.iter().filter(|c| c.enabled) {
            let start = rate.frames_to_micros(clip.start);
            let end = rate.frames_to_micros(clip.start + clip.duration);
            if covered.iter().any(|(s, e)| start < *e && *s < end) {
                continue;
            }
            let Some(media) = project.media(clip.source.media_id()) else {
                continue;
            };
            if media.kind == MediaKind::Image || !media.has_video() {
                continue;
            }
            covered.push((start, end));
            out.push(Segment {
                path: media.path.clone(),
                start_us: start,
                duration_us: end - start,
                source_start_us: rate.frames_to_micros(clip.source.source_start()),
                audio: false,
                lane: 0,
            });
        }
    }

    for (lane, track) in sequence
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Audio && t.enabled)
        .enumerate()
    {
        for clip in track.clips.iter().filter(|c| c.enabled) {
            let Some(media) = project.media(clip.source.media_id()) else {
                continue;
            };
            if !media.has_audio() {
                continue;
            }
            let start = rate.frames_to_micros(clip.start);
            let end = rate.frames_to_micros(clip.start + clip.duration);
            out.push(Segment {
                path: media.path.clone(),
                start_us: start,
                duration_us: end - start,
                source_start_us: rate.frames_to_micros(clip.source.source_start()),
                audio: true,
                lane,
            });
        }
    }
    out
}

/// Zero tolerance, so a seek lands on the frame asked for.
const EXACT: CMTime = CMTime {
    value: 0,
    timescale: 1,
    flags: CMTimeFlags::Valid,
    epoch: 0,
};

fn time(us: i64) -> CMTime {
    CMTime {
        value: us,
        timescale: TIMESCALE,
        flags: CMTimeFlags::Valid,
        epoch: 0,
    }
}

struct Surface {
    view: Retained<NSView>,
    player: Retained<AVPlayer>,
    /// Must be held for as long as the observer should fire, and handed back to
    /// `removeTimeObserver` before the player goes away.
    observer: RefCell<Option<Retained<AnyObject>>>,
}

impl Surface {
    /// Add a layer backed view above the webview. The viewer's matte is a plain rectangle
    /// with no web content over it, so ordering above avoids depending on webview
    /// transparency, which is unreliable on recent macOS (wry#1867).
    fn attach(window: &NSWindow, mtm: MainThreadMarker) -> Option<Self> {
        let content = window.contentView()?;
        let view = NSView::initWithFrame(NSView::alloc(mtm), NSRect::ZERO);
        view.setWantsLayer(true);

        let player = unsafe { AVPlayer::new(mtm) };
        let layer = unsafe { AVPlayerLayer::playerLayerWithPlayer(Some(&player)) };
        view.setLayer(Some(&layer));

        content.addSubview_positioned_relativeTo(&view, NSWindowOrderingMode::Above, None);
        Some(Self {
            view,
            player,
            observer: RefCell::new(None),
        })
    }

    /// Position the surface. The rect arrives in CSS pixels with a top left origin, which
    /// match AppKit points one to one because the webview fills the content view, but
    /// AppKit measures y from the bottom.
    fn set_rect(&self, x: f64, y: f64, width: f64, height: f64) {
        let Some(parent) = (unsafe { self.view.superview() }) else {
            return;
        };
        let bounds = parent.bounds();
        let flipped = bounds.size.height - y - height;
        self.view.setFrame(NSRect::new(
            NSPoint::new(x, flipped),
            NSSize::new(width, height),
        ));
    }

    fn set_segments(
        &self,
        segments: &[Segment],
        canvas: Canvas,
        at_us: i64,
        mtm: MainThreadMarker,
    ) {
        let composition = unsafe { AVMutableComposition::composition() };
        let lanes = segments.iter().filter(|s| s.audio).map(|s| s.lane).max();

        let video = (!segments.iter().all(|s| s.audio))
            .then(|| add_track(&composition, false))
            .flatten();
        let audio: Vec<_> = (0..=lanes.unwrap_or(0))
            .map(|_| add_track(&composition, true))
            .collect();

        // Where each clip's fit transform takes effect. Gathered as the clips go in,
        // because the source size is only known once its track has been read.
        let mut fits: Vec<(i64, CGAffineTransform)> = Vec::new();

        for segment in segments {
            let track = if segment.audio {
                audio.get(segment.lane).and_then(|t| t.as_ref())
            } else {
                video.as_ref()
            };
            let (Some(track), Some(asset)) = (track, asset(&segment.path)) else {
                continue;
            };
            // Deprecated in favour of loadTracksWithMediaType:completionHandler:, which
            // is asynchronous. These are local files, so the synchronous read does not
            // block on IO, and staying synchronous keeps this a single main thread hop.
            #[allow(deprecated)]
            let source = unsafe { asset.tracksWithMediaType(media_type(segment.audio)) };
            let Some(source) = source.firstObject() else {
                continue;
            };
            let range = CMTimeRange {
                start: time(segment.source_start_us),
                duration: time(segment.duration_us),
            };
            // A failed insert leaves a gap rather than killing the whole preview.
            let _ = unsafe {
                track.insertTimeRange_ofTrack_atTime_error(range, &source, time(segment.start_us))
            };
            if !segment.audio {
                let natural = unsafe { source.naturalSize() };
                let preferred = unsafe { source.preferredTransform() };
                fits.push((segment.start_us, fit(natural, preferred, canvas)));
            }
        }

        let item = unsafe { AVPlayerItem::playerItemWithAsset(&composition, mtm) };
        if let Some(track) = video.as_ref() {
            let end = segments
                .iter()
                .map(|s| s.start_us + s.duration_us)
                .max()
                .unwrap_or(0);
            let rendered = render_into_canvas(track, &mut fits, canvas, end);
            unsafe { item.setVideoComposition(Some(&rendered)) };
        }
        unsafe { self.player.replaceCurrentItemWithPlayerItem(Some(&item)) };
        // Replacing the item rewinds the player to zero. Put it back under the playhead,
        // here rather than from the UI, so the seek cannot race the swap.
        unsafe {
            self.player
                .seekToTime_toleranceBefore_toleranceAfter(time(at_us), EXACT, EXACT)
        };
    }
}

/// Put a source frame inside the sequence frame the way the renderer does: scaled to fit
/// without cropping, centred, the rest left black.
///
/// `preferred` is the track's own display transform, which is how a camera records that it
/// was held sideways. Applying it first means a portrait clip is fitted by the size it is
/// shown at rather than the size it is stored at.
fn fit(natural: CGSize, preferred: CGAffineTransform, canvas: Canvas) -> CGAffineTransform {
    // The displayed size is the source rect under `preferred`, which for the quarter turns
    // AVFoundation reports swaps width and height.
    let shown_w = (preferred.a * natural.width).abs() + (preferred.c * natural.height).abs();
    let shown_h = (preferred.b * natural.width).abs() + (preferred.d * natural.height).abs();
    let frame_w = f64::from(canvas.width);
    let frame_h = f64::from(canvas.height);
    if shown_w <= 0.0 || shown_h <= 0.0 {
        return preferred;
    }
    let scale = (frame_w / shown_w).min(frame_h / shown_h);
    CGAffineTransform {
        a: preferred.a * scale,
        b: preferred.b * scale,
        c: preferred.c * scale,
        d: preferred.d * scale,
        // Centring is symmetric about the middle of the frame, so it comes out the same
        // whichever way the render space measures y.
        tx: preferred.tx * scale + (frame_w - shown_w * scale) / 2.0,
        ty: preferred.ty * scale + (frame_h - shown_h * scale) / 2.0,
    }
}

/// Render the flattened video track into the sequence frame.
///
/// Without this AVFoundation plays the source at its own size, so a clip that did not
/// match the sequence looked nothing like the export. Every clip shares the one flattened
/// video track, so one layer instruction covers the timeline and the transform changes at
/// each clip's start, which is what `setTransform:atTime:` is for. That needs the times in
/// order, and the flattening walks tracks rather than the clock.
fn render_into_canvas(
    track: &AVMutableCompositionTrack,
    fits: &mut [(i64, CGAffineTransform)],
    canvas: Canvas,
    end_us: i64,
) -> Retained<AVMutableVideoComposition> {
    let composition = unsafe { AVMutableVideoComposition::videoComposition() };
    unsafe {
        composition.setRenderSize(CGSize::new(
            f64::from(canvas.width),
            f64::from(canvas.height),
        ));
        composition.setFrameDuration(CMTime {
            value: i64::from(canvas.rate.den),
            timescale: i32::try_from(canvas.rate.num).unwrap_or(TIMESCALE),
            flags: CMTimeFlags::Valid,
            epoch: 0,
        });
    }

    let layer = unsafe {
        AVMutableVideoCompositionLayerInstruction::videoCompositionLayerInstructionWithAssetTrack(
            track,
        )
    };
    fits.sort_by_key(|(at_us, _)| *at_us);
    for (at_us, transform) in fits.iter() {
        unsafe { layer.setTransform_atTime(*transform, time(*at_us)) };
    }
    let layers: Retained<AVVideoCompositionLayerInstruction> = Retained::into_super(layer);

    let instruction =
        unsafe { AVMutableVideoCompositionInstruction::videoCompositionInstruction() };
    unsafe {
        instruction.setTimeRange(CMTimeRange {
            start: time(0),
            duration: time(end_us),
        });
        instruction.setLayerInstructions(&NSArray::from_retained_slice(&[layers]));
    }

    let instruction = ProtocolObject::from_retained(instruction);
    unsafe { composition.setInstructions(&NSArray::from_retained_slice(&[instruction])) };
    composition
}

/// The AVFoundation media type constants. They are declared optional because the symbol
/// could in principle be missing, but on any macOS that runs this app they are present.
fn media_type(audio: bool) -> &'static AVMediaType {
    let value = if audio {
        unsafe { AVMediaTypeAudio }
    } else {
        unsafe { AVMediaTypeVideo }
    };
    value.expect("AVFoundation media type constant missing")
}

impl Drop for Surface {
    fn drop(&mut self) {
        if let Some(observer) = self.observer.borrow_mut().take() {
            unsafe { self.player.removeTimeObserver(&observer) };
        }
    }
}

fn add_track(
    composition: &AVMutableComposition,
    audio: bool,
) -> Option<Retained<AVMutableCompositionTrack>> {
    unsafe {
        composition.addMutableTrackWithMediaType_preferredTrackID(
            media_type(audio),
            // kCMPersistentTrackID_Invalid: let AVFoundation pick the id.
            0,
        )
    }
}

fn asset(path: &str) -> Option<Retained<AVURLAsset>> {
    let path = NSString::from_str(path);
    let url = NSURL::fileURLWithPath(&path);
    Some(unsafe { AVURLAsset::URLAssetWithURL_options(&url, None) })
}

/// How often the player reports its time. Half a frame at 60 fps is smooth for the
/// playhead and timecode without flooding the webview with events.
const OBSERVE_HZ: i32 = 30;

impl Surface {
    /// Report the player's own clock. AVFoundation is the only timekeeper: the UI used
    /// to advance the playhead from wall time, which drifted from the picture and
    /// disagreed with it about where playback restarts.
    fn observe(&self, on_time: impl Fn(i64) + 'static) {
        if self.observer.borrow().is_some() {
            return;
        }
        let block = RcBlock::new(move |time: CMTime| {
            if !time.flags.contains(CMTimeFlags::Valid) || time.timescale <= 0 {
                return;
            }
            let micros = i128::from(time.value) * 1_000_000 / i128::from(time.timescale);
            on_time(i64::try_from(micros).unwrap_or(0));
        });
        let interval = CMTime {
            value: 1,
            timescale: OBSERVE_HZ,
            flags: CMTimeFlags::Valid,
            epoch: 0,
        };
        // A null queue means the main queue, which is where the UI needs these anyway.
        let token = unsafe {
            self.player
                .addPeriodicTimeObserverForInterval_queue_usingBlock(interval, None, &block)
        };
        *self.observer.borrow_mut() = Some(token);
    }
}

/// Run `f` against the surface, creating it on first use.
///
/// `ns_window` comes from Tauri's `PlatformWebview::ns_window`, which hands out a raw
/// pointer. The cast is confined here so no other module needs unsafe.
fn with_surface<F: FnOnce(&Surface, MainThreadMarker)>(ns_window: *mut c_void, f: F) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    if ns_window.is_null() {
        return;
    }
    // Safe: Tauri gives us the window it owns, and we only touch it on the main thread.
    let window: &NSWindow = unsafe { &*ns_window.cast::<NSWindow>() };
    SURFACE.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Surface::attach(window, mtm);
        }
        if let Some(surface) = slot.as_ref() {
            f(surface, mtm);
        }
    });
}

pub fn set_rect(ns_window: *mut c_void, x: f64, y: f64, width: f64, height: f64) {
    with_surface(ns_window, |surface, _| {
        surface.set_rect(x, y, width, height)
    });
}

/// Take the surface out of the picture, or put it back.
///
/// The view sits above the webview, so it covers anything the web layer draws in the same
/// place. Hiding it is how a dialog or a menu over the viewer gets to be seen whole.
pub fn set_hidden(ns_window: *mut c_void, hidden: bool) {
    with_surface(ns_window, |surface, _| surface.view.setHidden(hidden));
}

pub fn set_segments(ns_window: *mut c_void, segments: &[Segment], canvas: Canvas, at_us: i64) {
    with_surface(ns_window, |surface, mtm| {
        surface.set_segments(segments, canvas, at_us, mtm)
    });
}

/// Install the player clock reporter. Safe to call repeatedly; only the first takes.
pub fn observe_time(ns_window: *mut c_void, on_time: impl Fn(i64) + 'static) {
    with_surface(ns_window, |surface, _| surface.observe(on_time));
}

pub fn play(ns_window: *mut c_void) {
    with_surface(ns_window, |surface, _| unsafe { surface.player.play() });
}

pub fn pause(ns_window: *mut c_void) {
    with_surface(ns_window, |surface, _| unsafe { surface.player.pause() });
}

/// Seeking with zero tolerance costs more than the default, which is free to land on the
/// nearest keyframe. On long GOP footage that default is seconds away from the frame
/// asked for, which showed up as playback resuming before where it was paused.
pub fn seek(ns_window: *mut c_void, us: i64) {
    with_surface(ns_window, |surface, _| unsafe {
        surface
            .player
            .seekToTime_toleranceBefore_toleranceAfter(time(us), EXACT, EXACT)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_core::ids::ClipId;
    use studio_core::model::{Clip, ClipSource, Track};
    use studio_core::testing::sample_edit;

    /// `sample_edit` puts linked clips at frames 12..60 (source from 6) and 100..124 at
    /// 24 fps, so the expected microseconds are exact.
    #[test]
    fn flattens_linked_clips_into_video_and_audio_lanes() {
        let project = sample_edit();
        let all = segments(&project, project.first_sequence());
        let video: Vec<_> = all.iter().filter(|s| !s.audio).collect();
        let audio: Vec<_> = all.iter().filter(|s| s.audio).collect();

        assert_eq!(video.len(), 2, "one segment per video clip");
        assert_eq!(audio.len(), 2, "one segment per audio clip");
        assert_eq!(video[0].start_us, 500_000);
        assert_eq!(video[0].duration_us, 2_000_000);
        assert_eq!(video[0].source_start_us, 250_000);
        assert_eq!(video[1].start_us, 4_166_667);
        assert_eq!(video[1].source_start_us, 0);
        assert!(
            audio.iter().all(|s| s.lane == 0),
            "one audio track, one lane"
        );
    }

    #[test]
    fn a_higher_video_track_hides_what_is_under_it() {
        let mut project = sample_edit();
        let media_id = project.media[0].id;
        let mut over = Track::new(studio_core::model::TrackKind::Video, "V2");
        over.clips.push(Clip {
            id: ClipId::new(),
            name: "over".into(),
            start: 12,
            duration: 48,
            enabled: true,
            link: None,
            source: ClipSource::Media {
                media_id,
                source_start: 0,
            },
        });
        // Video tracks are stored bottom to top, before the audio tracks.
        project.first_sequence_mut().tracks.insert(1, over);

        let video: Vec<_> = segments(&project, project.first_sequence())
            .into_iter()
            .filter(|s| !s.audio)
            .collect();
        assert_eq!(
            video.len(),
            2,
            "the covered V1 clip is dropped, not stacked"
        );
        assert_eq!(
            video[0].source_start_us, 0,
            "the V2 clip wins the shared range"
        );
        assert_eq!(
            video[1].start_us, 4_166_667,
            "the uncovered V1 clip survives"
        );
    }

    #[test]
    fn a_disabled_track_contributes_nothing() {
        let mut project = sample_edit();
        for track in &mut project.first_sequence_mut().tracks {
            track.enabled = false;
        }
        assert!(segments(&project, project.first_sequence()).is_empty());
    }

    /// What AVFoundation reports for footage that was not rotated.
    const UPRIGHT: CGAffineTransform = CGAffineTransform {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    fn hd() -> Canvas {
        Canvas {
            width: 1920,
            height: 1080,
            rate: Rational::new(24, 1),
        }
    }

    #[test]
    fn a_matching_clip_is_left_alone() {
        let t = fit(CGSize::new(1920.0, 1080.0), UPRIGHT, hd());
        assert_eq!((t.a, t.d), (1.0, 1.0));
        assert_eq!((t.tx, t.ty), (0.0, 0.0));
    }

    #[test]
    fn a_larger_clip_is_scaled_down_to_the_frame() {
        let t = fit(CGSize::new(3840.0, 2160.0), UPRIGHT, hd());
        assert_eq!((t.a, t.d), (0.5, 0.5));
        assert_eq!((t.tx, t.ty), (0.0, 0.0), "same aspect, so no bars");
    }

    #[test]
    fn a_smaller_clip_is_scaled_up_the_way_the_renderer_does() {
        // ffmpeg's force_original_aspect_ratio=decrease fits to the frame in both
        // directions, so a small clip grows rather than sitting in the middle.
        let t = fit(CGSize::new(640.0, 360.0), UPRIGHT, hd());
        assert_eq!((t.a, t.d), (3.0, 3.0));
    }

    #[test]
    fn a_portrait_clip_is_centred_between_black_bars() {
        let t = fit(CGSize::new(1080.0, 1920.0), UPRIGHT, hd());
        let scale = 1080.0 / 1920.0;
        assert!((t.a - scale).abs() < 1e-9);
        // 1080 wide at that scale is 607.5, leaving 656.25 either side.
        assert!((t.tx - (1920.0 - 1080.0 * scale) / 2.0).abs() < 1e-9);
        assert_eq!(t.ty, 0.0, "full height, so no bars above or below");
    }

    #[test]
    fn a_sideways_clip_is_fitted_by_the_size_it_is_shown_at() {
        // A phone held upright stores 1920x1080 and asks for a quarter turn. Fitting the
        // stored size would fill the frame and show the picture on its side.
        let quarter_turn = CGAffineTransform {
            a: 0.0,
            b: 1.0,
            c: -1.0,
            d: 0.0,
            tx: 1080.0,
            ty: 0.0,
        };
        let t = fit(CGSize::new(1920.0, 1080.0), quarter_turn, hd());
        let scale = 1080.0 / 1920.0;
        assert!((t.b - scale).abs() < 1e-9, "the turn is kept");
        assert!((t.c + scale).abs() < 1e-9);
        // Shown at 1080x1920, scaled to 607.5x1080, so it is inset horizontally.
        assert!((t.tx - (1080.0 * scale + (1920.0 - 1080.0 * scale) / 2.0)).abs() < 1e-9);
    }

    #[test]
    fn a_track_with_no_size_is_not_divided_by_zero() {
        let t = fit(CGSize::new(0.0, 0.0), UPRIGHT, hd());
        assert!(t.a.is_finite() && t.tx.is_finite());
    }
}
