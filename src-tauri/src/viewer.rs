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
use objc2_app_kit::{
    NSEvent, NSEventMask, NSEventModifierFlags, NSEventType, NSView, NSWindow, NSWindowOrderingMode,
};
use objc2_av_foundation::{
    AVMediaType, AVMediaTypeAudio, AVMediaTypeVideo, AVMutableComposition,
    AVMutableCompositionTrack, AVMutableVideoComposition, AVMutableVideoCompositionInstruction,
    AVMutableVideoCompositionLayerInstruction, AVPlayer, AVPlayerItem, AVPlayerLayer, AVURLAsset,
    AVVideoCompositionLayerInstruction,
};
use objc2_core_foundation::CFRetained;
use objc2_core_foundation::{CGAffineTransform, CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGColor;
use objc2_core_media::{CMTime, CMTimeFlags, CMTimeRange};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSSize, NSString, NSURL};
use objc2_quartz_core::{CALayer, CATransaction};
use std::ptr::NonNull;

use studio_core::model::{MediaKind, Project, Sequence, TrackKind, Transform};
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
    /// Which composition track this goes on: one per track of the sequence, counting
    /// from the bottom of its kind, so lane 0 is V1 or A1. Clips never overlap within a
    /// track, which is exactly what a composition track requires.
    pub lane: usize,
    /// Where the picture sits in the frame. Identity for audio, which has none.
    pub transform: Transform,
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

/// Spread one sequence over composition lanes: one per enabled track, keeping the stack.
///
/// The sequence is passed in rather than looked up, because the preview shows whichever
/// sequence is open and the document does not record that (ADR-0004).
///
/// Every enabled clip gets a segment. The preview used to keep only the highest video
/// track's clip at each instant, which matched the renderer while the renderer covered
/// the frame with each clip's own black padding. Both composite now, so a clip that is
/// scaled, rotated or faded has to let the layers below it through, and that needs them
/// all present. Clips never overlap within a track, which is what a composition track
/// requires, so one lane per track is the natural fit for video as it already was for
/// audio.
pub fn segments(project: &Project, sequence: &Sequence) -> Vec<Segment> {
    let rate = sequence.format.frame_rate;
    let mut out = Vec::new();

    for kind in [TrackKind::Video, TrackKind::Audio] {
        // Bottom to top within the kind, so lane 0 is V1 or A1.
        for (lane, track) in sequence
            .tracks
            .iter()
            .filter(|t| t.kind == kind && t.enabled)
            .enumerate()
        {
            let audio = kind == TrackKind::Audio;
            for clip in track.clips.iter().filter(|c| c.enabled) {
                let Some(media) = project.media(clip.source.media_id()) else {
                    continue;
                };
                let usable = if audio {
                    media.has_audio()
                } else {
                    media.kind != MediaKind::Image && media.has_video()
                };
                if !usable {
                    continue;
                }
                let start = rate.frames_to_micros(clip.start);
                let end = rate.frames_to_micros(clip.start + clip.duration);
                out.push(Segment {
                    path: media.path.clone(),
                    start_us: start,
                    duration_us: end - start,
                    source_start_us: rate.frames_to_micros(clip.source.source_start()),
                    audio,
                    lane,
                    transform: clip.transform,
                });
            }
        }
    }
    out
}

/// Where the handles go: the clip's own box, and the transform that puts it on screen.
///
/// Both arrive already in the surface's coordinates with y measured from the bottom, the
/// way AppKit measures. The web layer does that conversion, because it is the side that
/// knows the stage's size and already converts the surface's own rect the same way. This
/// module positions what it is given and works nothing out (ADR-0006).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Overlay {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// `a`, `b`, `c`, `d`, `tx`, `ty` of the clip's transform.
    pub transform: [f64; 6],
}

/// A mouse event over the picture, in the surface's own coordinates with y measured from
/// the top, so the web layer reads them the way it reads every other pointer event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pointer {
    pub phase: PointerPhase,
    pub x: f64,
    pub y: f64,
    pub shift: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerPhase {
    Down,
    Moved,
    Up,
}

impl PointerPhase {
    pub fn name(self) -> &'static str {
        match self {
            PointerPhase::Down => "down",
            PointerPhase::Moved => "moved",
            PointerPhase::Up => "up",
        }
    }
}

/// How large a corner mark is drawn, in points. Matches `HANDLE_REACH` on the web side
/// closely enough that what you can see is what you can grab.
const HANDLE_SIZE: f64 = 9.0;

const NO_TRANSFORM: CGAffineTransform = CGAffineTransform {
    a: 1.0,
    b: 0.0,
    c: 0.0,
    d: 1.0,
    tx: 0.0,
    ty: 0.0,
};

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
    /// The picture. A sublayer rather than the view's own layer, so the handles have
    /// somewhere to sit above it (ADR-0006).
    picture: Retained<AVPlayerLayer>,
    /// The clip's box and its corner marks, hidden until something is selected.
    overlay: Retained<CALayer>,
    /// Must be held for as long as the observer should fire, and handed back to
    /// `removeTimeObserver` before the player goes away.
    observer: RefCell<Option<Retained<AnyObject>>>,
    /// The mouse monitor, held for as long as it should fire.
    pointer: RefCell<Option<Retained<AnyObject>>>,
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
        let picture = unsafe { AVPlayerLayer::playerLayerWithPlayer(Some(&player)) };
        let overlay = overlay_layer();
        // A plain host layer with the picture and the handles as sublayers, rather than
        // the player layer being the view's own layer. Nothing can be drawn over a
        // view's own layer, and the handles have to sit over the picture (ADR-0006).
        let host = CALayer::layer();
        host.addSublayer(&picture);
        host.addSublayer(&overlay);
        view.setLayer(Some(&host));

        content.addSubview_positioned_relativeTo(&view, NSWindowOrderingMode::Above, None);
        Some(Self {
            view,
            player,
            picture,
            overlay,
            observer: RefCell::new(None),
            pointer: RefCell::new(None),
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
        // A layer hosting view does not lay its sublayers out, so the picture is sized
        // here. Without a transaction Core Animation would animate the resize, which
        // reads as the picture sliding whenever the panel is dragged.
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        self.picture.setFrame(CGRect::new(
            CGPoint::new(0.0, 0.0),
            CGSize::new(width, height),
        ));
        CATransaction::commit();
    }

    /// Put the handles on the clip, or take them away.
    fn set_overlay(&self, overlay: Option<Overlay>) {
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        match overlay {
            None => self.overlay.setHidden(true),
            Some(o) => {
                self.overlay.setHidden(false);
                // A layer's frame means nothing while a transform is in effect, so the
                // box is placed square and the transform goes on afterwards.
                self.overlay.setAffineTransform(NO_TRANSFORM);
                self.overlay.setFrame(CGRect::new(
                    CGPoint::new(o.x, o.y),
                    CGSize::new(o.width, o.height),
                ));
                let [a, b, c, d, tx, ty] = o.transform;
                self.overlay
                    .setAffineTransform(CGAffineTransform { a, b, c, d, tx, ty });
                // The marks ride along with the box, so a turned clip has turned
                // handles, which is how you can tell which corner you are grabbing.
                // Tucked inside the box rather than centred on its corners: a clip
                // that fills the frame has its corners on the edge of the picture, and
                // the web layer insets them the same way for the same reason.
                let (right, bottom) = (o.width - HANDLE_SIZE, o.height - HANDLE_SIZE);
                let corners = [(0.0, 0.0), (right, 0.0), (right, bottom), (0.0, bottom)];
                // Safety: the sublayers are the four marks added in `overlay_layer`,
                // and nothing else ever touches this layer's children.
                if let Some(marks) = unsafe { self.overlay.sublayers() } {
                    for (mark, (cx, cy)) in marks.iter().zip(corners) {
                        mark.setFrame(CGRect::new(
                            CGPoint::new(cx, cy),
                            CGSize::new(HANDLE_SIZE, HANDLE_SIZE),
                        ));
                    }
                }
            }
        }
        CATransaction::commit();
    }

    /// Report mouse events over the picture. Safe to call repeatedly; only the first
    /// takes.
    ///
    /// A local monitor rather than an `NSView` subclass, so no Objective-C class has to
    /// be declared and the responder chain is left exactly as it was: the event is
    /// returned unchanged and goes on to whatever would have had it (ADR-0006).
    fn watch_pointer(&self, on_pointer: impl Fn(Pointer) + 'static) {
        if self.pointer.borrow().is_some() {
            return;
        }
        let view = self.view.clone();
        let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            let event = unsafe { event.as_ref() };
            if !view.isHidden() {
                if let Some(pointer) = pointer_in(&view, event) {
                    on_pointer(pointer);
                }
            }
            // Hand it on: this watches, it does not consume.
            (event as *const NSEvent).cast_mut()
        });
        let mask =
            NSEventMask::LeftMouseDown | NSEventMask::LeftMouseDragged | NSEventMask::LeftMouseUp;
        *self.pointer.borrow_mut() =
            unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &block) };
    }

    fn set_segments(
        &self,
        segments: &[Segment],
        canvas: Canvas,
        at_us: i64,
        mtm: MainThreadMarker,
    ) {
        let composition = unsafe { AVMutableComposition::composition() };

        // A composition track per lane that has something on it. Only the lanes the
        // segments actually use, so an enabled but empty track leaves no empty track in
        // the composition for a layer instruction to point at.
        let open_lanes = |audio: bool| {
            let mut used: Vec<usize> = segments
                .iter()
                .filter(|s| s.audio == audio)
                .map(|s| s.lane)
                .collect();
            used.sort_unstable();
            used.dedup();
            used.into_iter()
                .map(|index| Lane {
                    index,
                    track: add_track(&composition, audio),
                    keys: Vec::new(),
                })
                .collect::<Vec<_>>()
        };
        let mut video = open_lanes(false);
        let mut audio = open_lanes(true);

        for segment in segments {
            let lanes = if segment.audio {
                &mut audio
            } else {
                &mut video
            };
            let Some(at) = lanes.iter().position(|l| l.index == segment.lane) else {
                continue;
            };
            // Cloned so the lane can be written to below; a `Retained` clone is a
            // retain, not a copy of the track.
            let (Some(track), Some(asset)) = (lanes[at].track.clone(), asset(&segment.path)) else {
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
                // The fit needs the source size, which is only known now that the
                // track has been read, so the keyframe is gathered here.
                let natural = unsafe { source.naturalSize() };
                let preferred = unsafe { source.preferredTransform() };
                let fitted = fit(natural, preferred, canvas);
                lanes[at].keys.push(Keyframe {
                    at_us: segment.start_us,
                    transform: then(fitted, placement(segment.transform, canvas)),
                    opacity: segment.transform.opacity_factor(),
                });
            }
        }

        let item = unsafe { AVPlayerItem::playerItemWithAsset(&composition, mtm) };
        if !video.is_empty() {
            let end = segments
                .iter()
                .map(|s| s.start_us + s.duration_us)
                .max()
                .unwrap_or(0);
            let rendered = render_into_canvas(&mut video, canvas, end);
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

/// One lane's transform and opacity from a given instant, which is a clip's start.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Keyframe {
    at_us: i64,
    transform: CGAffineTransform,
    opacity: f64,
}

/// One track of the sequence, as a composition track plus what it does over time.
///
/// Lanes are held in `index` order, counting from the bottom of their kind, so reading
/// them backwards gives the drawing order AVFoundation wants.
struct Lane {
    index: usize,
    track: Option<Retained<AVMutableCompositionTrack>>,
    keys: Vec<Keyframe>,
}

/// Apply `first`, then `second`.
///
/// Written out rather than borrowed from CoreGraphics so the order is stated here rather
/// than remembered: `CGAffineTransformConcat` reads the other way round from the way
/// these are composed.
fn then(first: CGAffineTransform, second: CGAffineTransform) -> CGAffineTransform {
    CGAffineTransform {
        a: second.a * first.a + second.c * first.b,
        b: second.b * first.a + second.d * first.b,
        c: second.a * first.c + second.c * first.d,
        d: second.b * first.c + second.d * first.d,
        tx: second.a * first.tx + second.c * first.ty + second.tx,
        ty: second.b * first.tx + second.d * first.ty + second.ty,
    }
}

/// The clip's own transform, in the frame the fit has already placed it in.
///
/// Scale and rotation turn about the anchor, then the anchor is put where `position`
/// says, which is Premiere's convention and the same arithmetic `export::render` does
/// for FFmpeg. Rotation is clockwise: a video composition measures y downwards, which is
/// why a camera's `preferredTransform` for a quarter turn has a positive `b`.
fn placement(t: Transform, canvas: Canvas) -> CGAffineTransform {
    let (cx, cy) = (
        f64::from(canvas.width) / 2.0,
        f64::from(canvas.height) / 2.0,
    );
    let (ax, ay) = t.anchor_px();
    let (px, py) = t.position_px();
    let (sx, sy) = t.scale_factor();
    let (sin, cos) = t.rotation_radians().sin_cos();

    // The anchor to the origin, scale, rotate, then the anchor to where it belongs.
    let to_origin = shift(-(cx + ax), -(cy + ay));
    let scale = CGAffineTransform {
        a: sx,
        b: 0.0,
        c: 0.0,
        d: sy,
        tx: 0.0,
        ty: 0.0,
    };
    let rotate = CGAffineTransform {
        a: cos,
        b: sin,
        c: -sin,
        d: cos,
        tx: 0.0,
        ty: 0.0,
    };
    then(
        then(then(to_origin, scale), rotate),
        shift(cx + px, cy + py),
    )
}

fn shift(tx: f64, ty: f64) -> CGAffineTransform {
    CGAffineTransform {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        tx,
        ty,
    }
}

/// Render the video lanes into the sequence frame, stacked.
///
/// Without this AVFoundation plays the source at its own size, so a clip that did not
/// match the sequence looked nothing like the export. One layer instruction per lane,
/// and within a lane the transform and opacity change at each clip's start, which is
/// what `setTransform:atTime:` and `setOpacity:atTime:` are for. Both need their times
/// in order, and the lanes are filled track by track rather than by the clock.
///
/// AVFoundation composites the layer instructions front to back, so the array runs from
/// the top video track down, the opposite of the lane order, and the result matches the
/// renderer: a later video track sits over an earlier one.
fn render_into_canvas(
    lanes: &mut [Lane],
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

    let mut instructions: Vec<Retained<AVVideoCompositionLayerInstruction>> = Vec::new();
    // Highest lane first: AVFoundation draws the first layer instruction in front.
    for lane in lanes.iter_mut().rev() {
        let Some(track) = lane.track.as_ref() else {
            continue;
        };
        let layer = unsafe {
            AVMutableVideoCompositionLayerInstruction::videoCompositionLayerInstructionWithAssetTrack(
                track,
            )
        };
        lane.keys.sort_by_key(|k| k.at_us);
        for key in &lane.keys {
            unsafe {
                layer.setTransform_atTime(key.transform, time(key.at_us));
                layer.setOpacity_atTime(key.opacity as f32, time(key.at_us));
            }
        }
        instructions.push(Retained::into_super(layer));
    }

    let instruction =
        unsafe { AVMutableVideoCompositionInstruction::videoCompositionInstruction() };
    unsafe {
        instruction.setTimeRange(CMTimeRange {
            start: time(0),
            duration: time(end_us),
        });
        instruction.setLayerInstructions(&NSArray::from_retained_slice(&instructions));
    }

    let instruction = ProtocolObject::from_retained(instruction);
    unsafe { composition.setInstructions(&NSArray::from_retained_slice(&[instruction])) };
    composition
}

/// The box and its four corner marks, in the accent colour the UI uses for selection.
///
/// Colours are a theme decision everywhere else in the app, but a CALayer cannot read a
/// CSS variable. Kept to the one value the timeline already draws selections in, so the
/// handles match what is on screen beside them.
fn overlay_layer() -> Retained<CALayer> {
    let layer = CALayer::layer();
    let colour = accent();
    layer.setBorderWidth(1.0);
    layer.setBorderColor(Some(&colour));
    layer.setHidden(true);
    for _ in 0..4 {
        let mark = CALayer::layer();
        mark.setBackgroundColor(Some(&colour));
        layer.addSublayer(&mark);
    }
    layer
}

fn accent() -> CFRetained<CGColor> {
    // `color.accent.base` in src/theme/themes/studio.ts (#e5e5e5), which is what the
    // browser preview draws the same box in. It was a red left over from a theme that
    // no longer exists, and red now means the playhead.
    CGColor::new_srgb(0.898, 0.898, 0.898, 1.0)
}

/// An event's position in the surface's own coordinates, or `None` when it is somewhere
/// else. A press outside the picture is not ours; a drag or a release is reported
/// wherever it lands, so a gesture that wanders off the stage still finishes.
fn pointer_in(view: &NSView, event: &NSEvent) -> Option<Pointer> {
    let phase = match event.r#type() {
        NSEventType::LeftMouseDown => PointerPhase::Down,
        NSEventType::LeftMouseDragged => PointerPhase::Moved,
        NSEventType::LeftMouseUp => PointerPhase::Up,
        _ => return None,
    };
    let bounds = view.bounds();
    let point = view.convertPoint_fromView(event.locationInWindow(), None);
    // AppKit measures y from the bottom of the view; the web layer measures it from the
    // top, as CSS does, and so does everything in `framing.ts`.
    let y = bounds.size.height - point.y;
    let inside =
        (0.0..=bounds.size.width).contains(&point.x) && (0.0..=bounds.size.height).contains(&y);
    if phase == PointerPhase::Down && !inside {
        return None;
    }
    Some(Pointer {
        phase,
        x: point.x,
        y,
        shift: event.modifierFlags().contains(NSEventModifierFlags::Shift),
    })
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
        if let Some(monitor) = self.pointer.borrow_mut().take() {
            unsafe { NSEvent::removeMonitor(&monitor) };
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

/// Put the handles on the clip, or take them away (ADR-0006).
pub fn set_overlay(ns_window: *mut c_void, overlay: Option<Overlay>) {
    with_surface(ns_window, |surface, _| surface.set_overlay(overlay));
}

/// Install the mouse reporter. Safe to call repeatedly; only the first takes.
pub fn watch_pointer(ns_window: *mut c_void, on_pointer: impl Fn(Pointer) + 'static) {
    with_surface(ns_window, |surface, _| surface.watch_pointer(on_pointer));
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
    use studio_core::model::{Clip, ClipSource, Offset, Scale, Track, Transform, MILLI};
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

    /// Add a V2 covering the first V1 clip exactly.
    fn with_a_track_on_top(project: &mut Project) {
        let media_id = project.media[0].id;
        let mut over = Track::new(studio_core::model::TrackKind::Video, "V2");
        over.clips.push(Clip {
            id: ClipId::new(),
            name: "over".into(),
            start: 12,
            duration: 48,
            enabled: true,
            link: None,
            transform: Transform::IDENTITY,
            source: ClipSource::Media {
                media_id,
                source_start: 0,
            },
        });
        // Video tracks are stored bottom to top, before the audio tracks.
        project.first_sequence_mut().tracks.insert(1, over);
    }

    #[test]
    fn a_covered_clip_is_kept_on_its_own_lane() {
        // The preview used to drop whatever a higher track covered. It cannot any more:
        // the clip on top may be scaled, turned or faded, and then what is under it has
        // to be there to show through.
        let mut project = sample_edit();
        with_a_track_on_top(&mut project);

        let video: Vec<_> = segments(&project, project.first_sequence())
            .into_iter()
            .filter(|s| !s.audio)
            .collect();
        assert_eq!(video.len(), 3, "both V1 clips and the V2 clip");

        let covered = video
            .iter()
            .find(|s| s.lane == 0 && s.start_us == 500_000)
            .expect("the covered V1 clip survives");
        assert_eq!(covered.source_start_us, 250_000);
        let on_top = video
            .iter()
            .find(|s| s.lane == 1)
            .expect("the V2 clip has its own lane");
        assert_eq!(on_top.source_start_us, 0);
    }

    #[test]
    fn lanes_count_from_the_bottom_of_each_kind() {
        let mut project = sample_edit();
        with_a_track_on_top(&mut project);
        let all = segments(&project, project.first_sequence());
        assert!(
            all.iter().filter(|s| s.audio).all(|s| s.lane == 0),
            "one audio track, one lane"
        );
        let video_lanes: std::collections::BTreeSet<_> =
            all.iter().filter(|s| !s.audio).map(|s| s.lane).collect();
        assert_eq!(video_lanes, [0, 1].into_iter().collect());
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

    /// Where a transform sends a point.
    fn at(t: CGAffineTransform, x: f64, y: f64) -> (f64, f64) {
        (t.a * x + t.c * y + t.tx, t.b * x + t.d * y + t.ty)
    }

    fn close(got: (f64, f64), want: (f64, f64)) {
        assert!(
            (got.0 - want.0).abs() < 1e-6 && (got.1 - want.1).abs() < 1e-6,
            "got {got:?}, want {want:?}"
        );
    }

    /// The middle of a 1920x1080 frame.
    const CENTRE: (f64, f64) = (960.0, 540.0);

    fn moved(transform: Transform) -> CGAffineTransform {
        placement(transform, hd())
    }

    #[test]
    fn then_applies_its_first_argument_first() {
        // Shift right by ten, then double: twenty, not ten.
        let combined = then(
            shift(10.0, 0.0),
            CGAffineTransform {
                a: 2.0,
                b: 0.0,
                c: 0.0,
                d: 2.0,
                tx: 0.0,
                ty: 0.0,
            },
        );
        close(at(combined, 0.0, 0.0), (20.0, 0.0));
    }

    #[test]
    fn an_identity_transform_moves_nothing() {
        let t = moved(Transform::IDENTITY);
        close(at(t, CENTRE.0, CENTRE.1), CENTRE);
        close(at(t, 0.0, 0.0), (0.0, 0.0));
    }

    #[test]
    fn position_slides_the_picture() {
        let t = moved(Transform {
            position: Offset {
                x: 40 * MILLI,
                y: -10 * MILLI,
            },
            ..Transform::IDENTITY
        });
        close(at(t, CENTRE.0, CENTRE.1), (1000.0, 530.0));
    }

    #[test]
    fn scale_grows_about_the_centre_when_the_anchor_is_there() {
        let t = moved(Transform {
            scale: Scale {
                x: MILLI / 2,
                y: MILLI / 2,
            },
            ..Transform::IDENTITY
        });
        close(at(t, CENTRE.0, CENTRE.1), CENTRE);
        // The bottom right corner comes half way in.
        close(at(t, 1920.0, 1080.0), (1440.0, 810.0));
    }

    #[test]
    fn rotation_is_clockwise() {
        // A point to the right of the centre ends up below it, which is clockwise in a
        // render space that measures y downwards.
        let t = moved(Transform {
            rotation: 90 * MILLI,
            ..Transform::IDENTITY
        });
        close(at(t, CENTRE.0 + 100.0, CENTRE.1), (960.0, 640.0));
    }

    #[test]
    fn the_anchor_stays_where_position_puts_it() {
        // Mirrors `rotation_turns_about_the_anchor_not_the_centre` in export::render:
        // whatever the scale and rotation, the anchor lands at the centre plus position.
        for rotation in [0, 30 * MILLI, -90 * MILLI, 200 * MILLI] {
            let transform = Transform {
                scale: Scale {
                    x: MILLI / 2,
                    y: MILLI / 2,
                },
                rotation,
                anchor: Offset {
                    x: 100 * MILLI,
                    y: 0,
                },
                ..Transform::IDENTITY
            };
            // The anchor is 100 px right of the fitted picture's centre.
            close(at(moved(transform), CENTRE.0 + 100.0, CENTRE.1), CENTRE);
        }
    }

    #[test]
    fn an_anchor_alone_slides_the_picture() {
        // Premiere's convention, and what export::render does: position places the
        // anchor, so pulling the anchor right slides the picture left.
        let t = moved(Transform {
            anchor: Offset {
                x: 100 * MILLI,
                y: 0,
            },
            ..Transform::IDENTITY
        });
        close(at(t, CENTRE.0, CENTRE.1), (860.0, 540.0));
    }

    #[test]
    fn a_fitted_clip_is_still_centred_after_it_is_scaled() {
        // Fit and placement together: a matching clip scaled by half keeps its middle in
        // the middle of the frame, and its own corner lands half way in.
        let fitted = fit(CGSize::new(1920.0, 1080.0), UPRIGHT, hd());
        let t = then(
            fitted,
            moved(Transform {
                scale: Scale {
                    x: MILLI / 2,
                    y: MILLI / 2,
                },
                ..Transform::IDENTITY
            }),
        );
        close(at(t, 960.0, 540.0), CENTRE);
        close(at(t, 0.0, 0.0), (480.0, 270.0));
    }

    #[test]
    fn a_track_with_no_size_is_not_divided_by_zero() {
        let t = fit(CGSize::new(0.0, 0.0), UPRIGHT, hd());
        assert!(t.a.is_finite() && t.tx.is_finite());
    }
}
