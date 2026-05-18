use std::path::{Path, PathBuf};
use tiny_skia::{Pixmap, PixmapPaint, Transform, Color};
use hound::WavReader;
use aether_core::{AetherError, Ref};

pub struct ObservationPacket {
    pub asset_ref: Ref,
    pub contact_sheet: Option<PathBuf>,
    pub audio_rms: Option<Vec<f32>>,
    pub anomalies: Vec<String>,
}

pub fn extract_keyframes(
    video_path: &Path,
    times: &[f64],
    output_dir: &Path,
) -> Result<Vec<PathBuf>, AetherError> {
    let mut paths = Vec::new();
    if !video_path.exists() {
        return Err(AetherError::IoError(
            video_path.to_string_lossy().to_string(),
            "Source video file does not exist".to_string(),
        ));
    }

    if let Err(e) = std::fs::create_dir_all(output_dir) {
        return Err(AetherError::IoError(
            output_dir.to_string_lossy().to_string(),
            e.to_string(),
        ));
    }

    for (i, &t) in times.iter().enumerate() {
        let out_path = output_dir.join(format!("frame_{}.png", i));
        
        let status = std::process::Command::new("ffmpeg")
            .arg("-ss")
            .arg(t.to_string())
            .arg("-i")
            .arg(video_path)
            .arg("-vframes")
            .arg("1")
            .arg("-f")
            .arg("image2")
            .arg(&out_path)
            .arg("-y")
            .status();

        match status {
            Ok(s) if s.success() && out_path.exists() => {
                paths.push(out_path);
            }
            _ => {
                // Graceful fallback: create a robust dummy color frame using tiny-skia
                let mut pixmap = Pixmap::new(320, 240)
                    .ok_or_else(|| AetherError::MediaError("Failed to allocate dummy keyframe".to_string()))?;
                pixmap.fill(Color::from_rgba8(20, 20, 20, 255));
                
                pixmap.save_png(&out_path)
                    .map_err(|e| AetherError::MediaError(format!("Failed to save dummy keyframe: {}", e)))?;
                paths.push(out_path);
            }
        }
    }
    
    Ok(paths)
}

pub fn generate_contact_sheet(
    frames: &[PathBuf],
    cols: u32,
    rows: u32,
    output_path: &Path,
) -> Result<(), AetherError> {
    if frames.is_empty() {
        return Err(AetherError::OperationFailed("Cannot generate contact sheet with zero frames".to_string()));
    }

    let frame_w = 160;
    let frame_h = 120;
    let width = frame_w * cols;
    let height = frame_h * rows;

    let mut sheet = Pixmap::new(width, height)
        .ok_or_else(|| AetherError::MediaError(format!("Failed to allocate contact sheet pixmap {}x{}", width, height)))?;

    sheet.fill(Color::from_rgba8(15, 15, 15, 255));

    let paint = PixmapPaint::default();
    let mut idx = 0;
    for r in 0..rows {
        for c in 0..cols {
            if idx >= frames.len() {
                break;
            }
            let frame_path = &frames[idx];
            if let Ok(frame_pixmap) = Pixmap::load_png(frame_path) {
                let scale_x = frame_w as f32 / frame_pixmap.width() as f32;
                let scale_y = frame_h as f32 / frame_pixmap.height() as f32;
                let x = c * frame_w;
                let y = r * frame_h;
                
                sheet.draw_pixmap(
                    x as i32,
                    y as i32,
                    frame_pixmap.as_ref(),
                    &paint,
                    Transform::from_scale(scale_x, scale_y),
                    None,
                );
            }
            idx += 1;
        }
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AetherError::IoError(parent.to_string_lossy().to_string(), e.to_string()))?;
    }

    sheet.save_png(output_path)
        .map_err(|e| AetherError::MediaError(format!("Failed to save contact sheet PNG: {}", e)))?;

    Ok(())
}

pub fn analyze_audio_rms(audio_path: &Path) -> Result<Vec<f32>, AetherError> {
    if !audio_path.exists() {
        return Err(AetherError::IoError(
            audio_path.to_string_lossy().to_string(),
            "Source audio file does not exist".to_string(),
        ));
    }

    // Try extracting mono audio to WAV via ffmpeg for non-WAV assets or video assets
    let temp_wav = std::env::temp_dir().join(format!(
        "aether_rms_{}.wav",
        blake3::hash(audio_path.to_string_lossy().as_bytes()).to_hex()
    ));

    let status = std::process::Command::new("ffmpeg")
        .arg("-i")
        .arg(audio_path)
        .arg("-vn")
        .arg("-acodec")
        .arg("pcm_s16le")
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg("16000")
        .arg(&temp_wav)
        .arg("-y")
        .status();

    let mut samples = Vec::new();
    let got_ffmpeg_wav = match status {
        Ok(s) if s.success() && temp_wav.exists() => {
            if let Ok(mut reader) = WavReader::open(&temp_wav) {
                for sample in reader.samples::<i16>() {
                    if let Ok(val) = sample {
                        samples.push(val as f32 / 32768.0);
                    }
                }
                true
            } else {
                false
            }
        }
        _ => false,
    };

    if temp_wav.exists() {
        let _ = std::fs::remove_file(temp_wav);
    }

    // Direct WAV fallback
    if !got_ffmpeg_wav {
        if let Ok(mut reader) = WavReader::open(audio_path) {
            for sample in reader.samples::<i16>() {
                if let Ok(val) = sample {
                    samples.push(val as f32 / 32768.0);
                }
            }
        }
    }

    if samples.is_empty() {
        return Ok(Vec::new());
    }

    // 100ms blocks at 16kHz is 1600 samples
    let block_size = 1600;
    let mut rms_vals = Vec::new();
    for chunk in samples.chunks(block_size) {
        let mut sum_sq = 0.0;
        for &s in chunk {
            sum_sq += s * s;
        }
        let rms = (sum_sq / chunk.len() as f32).sqrt();
        rms_vals.push(rms);
    }

    Ok(rms_vals)
}

pub fn detect_anomalies(
    rms: &[f32],
    video_frames: &[PathBuf],
) -> Result<Vec<String>, AetherError> {
    let mut anomalies = Vec::new();

    // 1. Audio Silence Detection
    if !rms.is_empty() {
        let silent_blocks = rms.iter().filter(|&&v| v < 0.001).count();
        let silence_ratio = silent_blocks as f32 / rms.len() as f32;
        if silence_ratio > 0.9 {
            anomalies.push(format!(
                "Audio Silence Detected: {:.1}% of audio is silent (RMS < 0.001)",
                silence_ratio * 100.0
            ));
        }

        // 2. Audio Clipping Detection
        let clipping_blocks = rms.iter().filter(|&&v| v > 0.9).count();
        if clipping_blocks > 0 {
            anomalies.push(format!(
                "Audio Clipping/Overdrive Detected: {} blocks exceeded 0.9 RMS limit",
                clipping_blocks
            ));
        }
    }

    // 3. Video Black Frame Detection
    let mut black_frames = 0;
    for frame_path in video_frames {
        if is_black_frame(frame_path) {
            black_frames += 1;
        }
    }

    if black_frames > 0 {
        anomalies.push(format!(
            "Black Frame Detected: {} keyframes are completely black",
            black_frames
        ));
    }

    Ok(anomalies)
}

fn is_black_frame(path: &Path) -> bool {
    if let Ok(pixmap) = Pixmap::load_png(path) {
        let mut dark_pixels = 0;
        let total_pixels = pixmap.width() * pixmap.height();
        if total_pixels == 0 {
            return false;
        }
        for p in pixmap.pixels() {
            let r = p.red() as f32 / 255.0;
            let g = p.green() as f32 / 255.0;
            let b = p.blue() as f32 / 255.0;
            // Standard relative luminance formula
            let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            if lum < 0.05 {
                dark_pixels += 1;
            }
        }
        return (dark_pixels as f32 / total_pixels as f32) > 0.98;
    }
    false
}
