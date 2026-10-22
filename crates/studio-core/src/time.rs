//! Time in the project document.
//!
//! Every timeline position, duration and source offset is an integer [`Frame`] at the
//! sequence frame rate. Floats appear only at tool boundaries (FFmpeg arguments, OTIO
//! rates) and are derived from these integers, never stored.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A count of frames at the sequence frame rate. Signed so that deltas are expressible,
/// but positions and durations in a valid document are never negative.
pub type Frame = i64;

/// Microseconds, used only for media metadata reported by ffprobe.
pub type Micros = i64;

/// An exact frame rate such as 24/1 or 24000/1001.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Rational {
    pub num: u32,
    pub den: u32,
}

impl Rational {
    pub const fn new(num: u32, den: u32) -> Self {
        Self { num, den }
    }

    pub fn is_valid(self) -> bool {
        self.num > 0 && self.den > 0
    }

    pub fn as_f64(self) -> f64 {
        f64::from(self.num) / f64::from(self.den)
    }

    /// Seconds for a frame count. Only for tool boundaries such as FFmpeg arguments.
    pub fn frames_to_seconds(self, frames: Frame) -> f64 {
        frames as f64 * f64::from(self.den) / f64::from(self.num)
    }

    /// Microseconds at the start of a frame, rounded up when the boundary falls between
    /// two microseconds. Seeking to the result lands on that frame rather than the one
    /// before it, which `micros_to_frames_floor` would otherwise report. Exact rates such
    /// as 24/1 divide evenly and are unaffected; 24000/1001 is the case that needs it.
    pub fn frames_to_micros(self, frames: Frame) -> Micros {
        let total = i128::from(frames) * i128::from(self.den) * 1_000_000;
        let num = i128::from(self.num);
        let value = total.div_euclid(num) + i128::from(total.rem_euclid(num) != 0);
        clamp_i128(value)
    }

    /// Whole frames contained in a span of microseconds, rounded down so a clip built
    /// from the result never reads past the end of its media.
    pub fn micros_to_frames_floor(self, micros: Micros) -> Frame {
        let value = i128::from(micros) * i128::from(self.num) / (i128::from(self.den) * 1_000_000);
        clamp_i128(value)
    }

    /// Frames in `seconds` whole seconds, for defaults such as still image duration.
    pub fn frames_in_seconds(self, seconds: u32) -> Frame {
        let value = i128::from(seconds) * i128::from(self.num) / i128::from(self.den);
        clamp_i128(value)
    }

    /// Nearest integer frames per second, the "timebase" used by FCP7 XML.
    pub fn timebase(self) -> u32 {
        let num = u64::from(self.num);
        let den = u64::from(self.den);
        u32::try_from((num + den / 2) / den).unwrap_or(u32::MAX)
    }

    /// True for NTSC-style rates such as 24000/1001 and 30000/1001.
    pub fn is_ntsc(self) -> bool {
        self.den == 1001
    }

    /// Parse ffprobe's `"24000/1001"` form. Returns `None` for `"0/0"` or garbage.
    pub fn parse_ratio(text: &str) -> Option<Self> {
        let (num, den) = text.split_once('/')?;
        let rate = Self::new(num.trim().parse().ok()?, den.trim().parse().ok()?);
        rate.is_valid().then_some(rate)
    }
}

fn clamp_i128(value: i128) -> Frame {
    Frame::try_from(value).unwrap_or(if value < 0 { Frame::MIN } else { Frame::MAX })
}

/// `HH:MM:SS:FF` non-drop-frame timecode for display and interchange.
pub fn timecode(frame: Frame, rate: Rational) -> String {
    let fps = Frame::from(rate.timebase().max(1));
    let sign = if frame < 0 { "-" } else { "" };
    let frame = frame.abs();
    let ff = frame % fps;
    let total_seconds = frame / fps;
    let ss = total_seconds % 60;
    let mm = (total_seconds / 60) % 60;
    let hh = total_seconds / 3600;
    format!("{sign}{hh:02}:{mm:02}:{ss:02}:{ff:02}")
}

/// Read back what [`timecode`] writes.
///
/// Non-drop-frame, like its inverse, so the two agree exactly: the frame count is read
/// against `rate.timebase()`, the nearest whole frames per second, and a rate of
/// 24000/1001 counts twenty four frames to the second here just as it does there.
///
/// Returns `None` rather than clamping when a field is out of range, because "00:00:00:30"
/// at 24 fps is a caller that has misunderstood something, and rounding it to the next
/// second would hide that.
pub fn parse_timecode(text: &str, rate: Rational) -> Option<Frame> {
    let text = text.trim();
    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, text),
    };
    let mut parts = digits.split(':');
    let mut next = || parts.next()?.parse::<Frame>().ok().filter(|n| *n >= 0);
    let (hh, mm, ss, ff) = (next()?, next()?, next()?, next()?);
    if parts.next().is_some() {
        return None;
    }
    let fps = Frame::from(rate.timebase().max(1));
    if mm > 59 || ss > 59 || ff >= fps {
        return None;
    }
    Some(sign * (((hh * 3600 + mm * 60 + ss) * fps) + ff))
}

#[cfg(test)]
mod tests {
    #[test]
    fn frames_to_micros_round_trips_through_floor() {
        for rate in [
            Rational::new(24, 1),
            Rational::new(25, 1),
            Rational::new(24000, 1001),
        ] {
            for frame in [0, 1, 135, 141, 90_000] {
                let us = rate.frames_to_micros(frame);
                assert_eq!(
                    rate.micros_to_frames_floor(us),
                    frame,
                    "{rate:?} frame {frame}"
                );
            }
        }
    }

    use super::*;

    #[test]
    fn ntsc_rate_converts_without_drift() {
        let rate = Rational::new(24000, 1001);
        // One hour of NTSC film is 86_313.6 frames; floor keeps us inside the media.
        assert_eq!(rate.micros_to_frames_floor(3_600_000_000), 86_313);
        assert_eq!(rate.timebase(), 24);
        assert!(rate.is_ntsc());
    }

    #[test]
    fn integer_rate_round_trips_seconds() {
        let rate = Rational::new(25, 1);
        assert_eq!(rate.frames_in_seconds(5), 125);
        assert!((rate.frames_to_seconds(125) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn parses_ffprobe_ratios() {
        assert_eq!(
            Rational::parse_ratio("30000/1001"),
            Some(Rational::new(30000, 1001))
        );
        assert_eq!(Rational::parse_ratio("0/0"), None);
        assert_eq!(Rational::parse_ratio("abc"), None);
    }

    #[test]
    fn formats_timecode() {
        let rate = Rational::new(24, 1);
        assert_eq!(timecode(0, rate), "00:00:00:00");
        assert_eq!(timecode(24 * 3661 + 5, rate), "01:01:01:05");
    }
}

#[cfg(test)]
mod timecode_parsing {
    use super::*;

    #[test]
    fn round_trips_against_the_formatter() {
        for rate in [
            Rational::new(24, 1),
            Rational::new(25, 1),
            Rational::new(30, 1),
            Rational::new(24000, 1001),
            Rational::new(30000, 1001),
        ] {
            for frame in [0, 1, 23, 24, 100, 1439, 86_399, -1, -4321] {
                let text = timecode(frame, rate);
                assert_eq!(
                    parse_timecode(&text, rate),
                    Some(frame),
                    "{rate:?} frame {frame} wrote {text}"
                );
            }
        }
    }

    #[test]
    fn rejects_a_frame_the_rate_cannot_hold() {
        let rate = Rational::new(24, 1);
        assert_eq!(parse_timecode("00:00:00:23", rate), Some(23));
        assert_eq!(
            parse_timecode("00:00:00:24", rate),
            None,
            "24 fps has no frame 24"
        );
        assert_eq!(
            parse_timecode("00:00:00:24", Rational::new(25, 1)),
            Some(24)
        );
    }

    #[test]
    fn rejects_what_is_not_a_timecode() {
        let rate = Rational::new(24, 1);
        for text in [
            "",
            "1:2:3",
            "00:00:00:00:00",
            "aa:bb:cc:dd",
            "00:60:00:00",
            "00:00:60:00",
            "12",
        ] {
            assert_eq!(parse_timecode(text, rate), None, "{text} should not parse");
        }
    }

    #[test]
    fn tolerates_surrounding_space() {
        assert_eq!(
            parse_timecode("  00:00:01:00  ", Rational::new(24, 1)),
            Some(24)
        );
    }
}
