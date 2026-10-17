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

#[cfg(test)]
mod tests {
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
