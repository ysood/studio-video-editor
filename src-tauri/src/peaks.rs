//! Audio peaks for the timeline, so a sound clip shows its shape rather than a colour.
//!
//! Peaks are derived from a file, like a thumbnail would be: they say nothing the media
//! does not already say, and recomputing them changes no decision anyone made. So they
//! are not document data, they never reach `Project`, and they live in a cache beside
//! `recents.json` rather than in a project file.
//!
//! A file is decoded once. Its samples are bucketed, and each bucket keeps the loudest
//! sample in it, which is what makes a drum hit stay a spike: an average would flatten
//! every transient into the noise floor and the waveform would stop being useful for
//! finding an edit. One byte per bucket, not a min and a max, because the row is thirty
//! pixels tall and a waveform mirrored about its centre is indistinguishable there from
//! a true asymmetric one at a quarter of the size.
//!
//! The cache is keyed by the file's path and the fingerprint taken at import, so a file
//! replaced on disk misses and is decoded again without anyone having to invalidate
//! anything.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use base64::Engine;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;

use crate::error::{CommandError, CommandResult, ErrorCode};
use crate::state::AppState;
use studio_core::model::Fingerprint;
use studio_core::MediaId;

/// Buckets per second of audio.
///
/// One bucket is one vertical line, so this is the resolution the waveform can ever be
/// drawn at. At 200 and the usual zoom a bucket is about a pixel wide; at the maximum
/// zoom it is a few, which reads as a slight squaring of the shape rather than as a
/// defect. It also sets the size: an hour of audio is 720 KB, small enough to hold
/// every clip's peaks in memory and draw synchronously.
const BUCKETS_PER_SECOND: u32 = 200;

/// Decode rate. A divisor of nothing in particular except that it divides evenly by
/// `BUCKETS_PER_SECOND`, which keeps bucket boundaries exact, and it is low enough that
/// decoding is bound by reading the file rather than by resampling.
const DECODE_RATE: u32 = 24_000;

const SAMPLES_PER_BUCKET: u32 = DECODE_RATE / BUCKETS_PER_SECOND;

/// Cache format. The version is in the magic so a later format is told apart rather
/// than misread, and a file that does not match is treated as absent.
const MAGIC: &[u8; 4] = b"SPK1";

const HEADER: usize = 12;

/// The shape of one clip's sound.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Peaks {
    pub buckets_per_second: u32,
    /// One byte per bucket: the loudest sample in it, 0 silent and 255 full scale.
    /// Base64 rather than an array of numbers, which for an hour of audio would be
    /// several megabytes of JSON digits to say the same thing.
    pub data: String,
}

/// The peaks for one asset in the bin, generating them if they are not cached.
///
/// Takes a media id and not a path: the path then comes from the document, which was
/// validated when the file was imported, so nothing here has to decide whether a path
/// from the window may be read.
#[tauri::command]
pub async fn media_peaks(
    media_id: MediaId,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<Option<Peaks>> {
    // The session lock is taken and dropped before any work, so decoding a long file
    // never holds the document against an edit.
    let found = {
        let slot = state.session();
        let session = slot
            .as_ref()
            .ok_or_else(|| CommandError::new(ErrorCode::NoProject, "no project is open"))?;
        let asset = session.editor.project().media(media_id).ok_or_else(|| {
            CommandError::new(ErrorCode::NotFound, "that media is not in the bin")
        })?;
        asset
            .has_audio()
            .then(|| (asset.path.clone(), asset.fingerprint))
    };
    // Silent by nature, not a failure: a still image or a video with no sound has no
    // shape to draw, and the timeline falls back to the flat fill.
    let Some((path, fingerprint)) = found else {
        return Ok(None);
    };

    let dir = cache_dir(&app);
    if let Some(cached) = dir
        .as_ref()
        .and_then(|dir| read_cache(&file(dir, &path, fingerprint)))
    {
        return Ok(Some(encode(cached)));
    }

    let ffmpeg = state.ffmpeg().ok_or_else(|| {
        CommandError::new(
            ErrorCode::Tool,
            "ffmpeg was not found. Install FFmpeg (brew install ffmpeg).",
        )
    })?;
    let decoded = tauri::async_runtime::spawn_blocking(move || generate(&ffmpeg, Path::new(&path)))
        .await
        .map_err(|_| CommandError::new(ErrorCode::Tool, "the decode thread panicked"))??;

    // A cache that cannot be written is not an error: the peaks are already in hand and
    // the only cost is decoding again next time.
    if let Some(dir) = dir {
        write_cache(
            &dir,
            &file(&dir, &decoded.path, fingerprint),
            &decoded.buckets,
        );
    }
    Ok(Some(encode(decoded.buckets)))
}

struct Decoded {
    path: String,
    buckets: Vec<u8>,
}

fn encode(buckets: Vec<u8>) -> Peaks {
    Peaks {
        buckets_per_second: BUCKETS_PER_SECOND,
        data: base64::engine::general_purpose::STANDARD.encode(buckets),
    }
}

/// Decode one file to peaks.
///
/// FFmpeg is asked for one mono stream of raw samples and nothing else: no container to
/// parse at this end, no shell, and a pipe that is read as it arrives so a long file
/// never has to fit anywhere.
fn generate(ffmpeg: &Path, path: &Path) -> CommandResult<Decoded> {
    let mut child = Command::new(ffmpeg)
        .args(["-hide_banner", "-nostdin", "-v", "error", "-i"])
        .arg(format!("file:{}", path.display()))
        .args([
            // The first audio stream only, and no picture: decoding video to throw it
            // away is most of the cost of this on a camera file.
            "-map",
            "0:a:0",
            "-vn",
            "-ac",
            "1",
            "-ar",
            &DECODE_RATE.to_string(),
            "-f",
            "s16le",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdout = child.stdout.take().expect("stdout was piped");

    let mut buckets: Vec<u8> = Vec::new();
    let mut buffer = [0u8; 64 * 1024];
    // A sample is two bytes and a read can stop between them.
    let mut odd: Option<u8> = None;
    let mut loudest: u16 = 0;
    let mut counted: u32 = 0;
    loop {
        let read = stdout.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let mut rest = &buffer[..read];
        if let Some(low) = odd.take() {
            if let Some((high, tail)) = rest.split_first() {
                take(
                    i16::from_le_bytes([low, *high]),
                    &mut loudest,
                    &mut counted,
                    &mut buckets,
                );
                rest = tail;
            } else {
                odd = Some(low);
                continue;
            }
        }
        let pairs = rest.chunks_exact(2);
        odd = pairs.remainder().first().copied();
        for pair in pairs {
            take(
                i16::from_le_bytes([pair[0], pair[1]]),
                &mut loudest,
                &mut counted,
                &mut buckets,
            );
        }
    }
    // Whatever is left over is a real bucket, just a short one: dropping it would lose
    // up to the last 5 ms, which at the end of a clip is exactly where a trim looks.
    if counted > 0 {
        buckets.push(level(loudest));
    }

    let status = child.wait()?;
    if !status.success() {
        return Err(CommandError::new(
            ErrorCode::Tool,
            "ffmpeg could not decode this file's audio",
        ));
    }
    Ok(Decoded {
        path: path.display().to_string(),
        buckets,
    })
}

/// Fold one sample into the bucket being filled.
fn take(sample: i16, loudest: &mut u16, counted: &mut u32, buckets: &mut Vec<u8>) {
    *loudest = (*loudest).max(sample.unsigned_abs());
    *counted += 1;
    if *counted == SAMPLES_PER_BUCKET {
        buckets.push(level(*loudest));
        *loudest = 0;
        *counted = 0;
    }
}

/// A sample magnitude as a byte. `i16::MIN` has no positive counterpart, so the top of
/// the range is clamped rather than allowed to wrap to silence.
fn level(loudest: u16) -> u8 {
    (loudest.min(i16::MAX as u16) >> 7) as u8
}

fn cache_dir(app: &AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_cache_dir().ok()?.join("peaks");
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Where one file's peaks are cached.
///
/// The path and the fingerprint together, so the same footage used by two projects is
/// decoded once and a file replaced in place simply misses.
fn file(dir: &Path, path: &str, fingerprint: Fingerprint) -> PathBuf {
    let mut hash = Sha256::new();
    hash.update(path.as_bytes());
    hash.update(fingerprint.size_bytes.to_le_bytes());
    hash.update(fingerprint.modified_unix_ms.to_le_bytes());
    hash.update(BUCKETS_PER_SECOND.to_le_bytes());
    let digest = hash.finalize();
    let mut name = String::with_capacity(32);
    for byte in &digest[..16] {
        name.push_str(&format!("{byte:02x}"));
    }
    dir.join(format!("{name}.peaks"))
}

fn read_cache(path: &Path) -> Option<Vec<u8>> {
    let bytes = fs::read(path).ok()?;
    if bytes.len() < HEADER || &bytes[..4] != MAGIC {
        return None;
    }
    let rate = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    let count = u32::from_le_bytes(bytes[8..12].try_into().ok()?) as usize;
    // A cache written at another resolution is not wrong, it is just not what the
    // drawing code is expecting; decode again rather than stretch it.
    if rate != BUCKETS_PER_SECOND || bytes.len() != HEADER + count {
        return None;
    }
    Some(bytes[HEADER..].to_vec())
}

fn write_cache(dir: &Path, path: &Path, buckets: &[u8]) {
    let mut bytes = Vec::with_capacity(HEADER + buckets.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&BUCKETS_PER_SECOND.to_le_bytes());
    bytes.extend_from_slice(&(buckets.len() as u32).to_le_bytes());
    bytes.extend_from_slice(buckets);
    // Written beside the target and renamed, so a cache read never sees half a file.
    let temporary = dir.join(format!(
        "{}.part",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    if fs::write(&temporary, &bytes).is_ok() && fs::rename(&temporary, path).is_err() {
        let _ = fs::remove_file(&temporary);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_bucket_is_pushed_and_the_running_maximum_resets() {
        let mut buckets = Vec::new();
        let mut loudest = 0;
        let mut counted = 0;
        for i in 0..SAMPLES_PER_BUCKET {
            // One loud sample among quiet ones: the bucket keeps the loud one.
            let sample = if i == 3 { 20_000 } else { 10 };
            take(sample, &mut loudest, &mut counted, &mut buckets);
        }
        assert_eq!(buckets, vec![level(20_000)]);
        assert_eq!((loudest, counted), (0, 0));
    }

    #[test]
    fn the_quietest_and_loudest_samples_reach_the_ends_of_the_range() {
        assert_eq!(level(0), 0);
        assert_eq!(level(i16::MAX as u16), 255);
        // i16::MIN's magnitude is one past i16::MAX and must not wrap to silence.
        assert_eq!(level(i16::MIN.unsigned_abs()), 255);
    }

    /// Decoding a signal whose shape is known in advance.
    ///
    /// Skipped rather than failed without FFmpeg, the way the render tests are: this is
    /// the one test here that needs the tool, and a machine without it should still be
    /// able to run the suite.
    #[test]
    fn a_known_tone_decodes_to_the_peaks_it_should() {
        let Some(ffmpeg) = crate::tools::locate("ffmpeg") else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tone.wav");
        // Two seconds: a full scale tone for the first, silence for the second.
        let built = Command::new(&ffmpeg)
            .args(["-hide_banner", "-v", "error", "-y", "-f", "lavfi", "-i"])
            .arg("aevalsrc='0.99*sin(2*PI*1000*t)*lt(t,1)':d=2:s=48000")
            .arg(&path)
            .status()
            .unwrap();
        assert!(built.success(), "could not build the test tone");

        let peaks = generate(&ffmpeg, &path).unwrap().buckets;
        // 200 buckets a second for two seconds, give or take the last short one.
        assert!(
            (399..=401).contains(&peaks.len()),
            "expected about 400 buckets, got {}",
            peaks.len()
        );
        // Inside the tone, every bucket is near full scale: 0.99 of i16::MAX is 253.
        assert!(peaks[50] > 240, "tone read as {}", peaks[50]);
        assert!(peaks[150] > 240, "tone read as {}", peaks[150]);
        // Inside the silence, nothing at all.
        assert_eq!(peaks[250], 0);
        assert_eq!(peaks[350], 0);
    }

    #[test]
    fn a_cache_file_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.peaks");
        let buckets = vec![0, 7, 255, 128];
        write_cache(dir.path(), &path, &buckets);
        assert_eq!(read_cache(&path), Some(buckets));
    }

    #[test]
    fn a_cache_file_that_is_not_ours_is_ignored_rather_than_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.peaks");
        fs::write(&path, b"not peaks at all").unwrap();
        assert_eq!(read_cache(&path), None);
        assert_eq!(read_cache(&dir.path().join("missing.peaks")), None);
    }

    #[test]
    fn a_truncated_cache_file_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.peaks");
        write_cache(dir.path(), &path, &[1, 2, 3, 4]);
        let mut bytes = fs::read(&path).unwrap();
        bytes.truncate(bytes.len() - 1);
        fs::write(&path, &bytes).unwrap();
        assert_eq!(read_cache(&path), None);
    }

    #[test]
    fn the_cache_name_follows_the_file_and_its_fingerprint() {
        let dir = Path::new("/cache");
        let print = Fingerprint {
            size_bytes: 10,
            modified_unix_ms: 20,
        };
        let moved = Fingerprint {
            size_bytes: 10,
            modified_unix_ms: 21,
        };
        assert_eq!(file(dir, "/a.wav", print), file(dir, "/a.wav", print));
        assert_ne!(file(dir, "/a.wav", print), file(dir, "/b.wav", print));
        // A file replaced in place keeps its path and must still miss.
        assert_ne!(file(dir, "/a.wav", print), file(dir, "/a.wav", moved));
    }
}
