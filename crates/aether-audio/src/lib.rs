use std::fs;
use std::path::Path;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use blake3;
use aether_core::{AetherError, Ref, Asset, AssetKind};

/// Extracts detailed technical metadata from an audio file using symphonia,
/// falling back to a quick FFmpeg probe if duration is not available in the container headers.
pub fn get_audio_metadata<P: AsRef<Path>>(path: P) -> Result<serde_json::Value, AetherError> {
    let p = path.as_ref();
    if !p.exists() {
        return Err(AetherError::IoError(p.to_string_lossy().to_string(), "Audio file does not exist".to_string()));
    }

    let file = fs::File::open(p)
        .map_err(|e| AetherError::IoError(p.to_string_lossy().to_string(), e.to_string()))?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let mut sample_rate = 44100;
    let mut channels = 2;
    let mut duration = 0.0f32;

    if let Ok(probed) = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
    {
        let format = probed.format;
        if let Some(track) = format.tracks().first() {
            let params = &track.codec_params;
            sample_rate = params.sample_rate.unwrap_or(44100);
            channels = params.channels.map(|c| c.count()).unwrap_or(2);
            if let Some(n_frames) = params.n_frames {
                duration = n_frames as f32 / sample_rate as f32;
            }
        }
    }


    // Fallback to FFmpeg probe if Symphonia was unable to extract a positive duration
    if duration <= 0.0 {
        let output = std::process::Command::new("ffprobe")
            .args([
                "-v", "error",
                "-show_entries", "format=duration",
                "-of", "default=noprint_wrappers=1:nokey=1",
                p.to_str().unwrap(),
            ])
            .output()
            .map_err(|e| AetherError::MediaError(format!("Failed to run ffprobe: {}", e)))?;

        if output.status.success() {
            let dur_str = String::from_utf8_lossy(&output.stdout);
            if let Ok(dur) = dur_str.trim().parse::<f32>() {
                duration = dur;
            }
        }
    }

    Ok(serde_json::json!({
        "sample_rate": sample_rate,
        "channels": channels,
        "duration": duration,
    }))
}

/// Imports an audio file using Content-Addressable Storage (CAS) with Blake3 hashing,
/// copying it to the cache directory, and retrieving metadata.
pub fn import_audio<P: AsRef<Path>>(
    src_path: P,
    r: Ref,
    cache_dir: &Path,
) -> Result<Asset, AetherError> {
    let src = src_path.as_ref();
    if !src.exists() {
        return Err(AetherError::IoError(
            src.to_string_lossy().to_string(),
            "Source audio file does not exist".to_string(),
        ));
    }

    // 1. Calculate Blake3 hash
    let mut file = fs::File::open(src)
        .map_err(|e| AetherError::IoError(src.to_string_lossy().to_string(), e.to_string()))?;
    let mut hasher = blake3::Hasher::new();
    std::io::copy(&mut file, &mut hasher)
        .map_err(|e| AetherError::IoError(src.to_string_lossy().to_string(), e.to_string()))?;
    let hash = hasher.finalize().to_hex().to_string();

    // 2. Fetch audio metadata
    let metadata = get_audio_metadata(src)?;

    // 3. Move/Copy to the cache directory
    if !cache_dir.exists() {
        fs::create_dir_all(cache_dir)
            .map_err(|e| AetherError::IoError(cache_dir.to_string_lossy().to_string(), e.to_string()))?;
    }
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("wav");
    let cache_file_name = format!("{}.{}", hash, ext);
    let cache_file_path = cache_dir.join(cache_file_name);

    if !cache_file_path.exists() {
        fs::copy(src, &cache_file_path)
            .map_err(|e| AetherError::IoError(src.to_string_lossy().to_string(), e.to_string()))?;
    }

    Ok(Asset {
        r,
        kind: AssetKind::Audio,
        path: cache_file_path,
        hash,
        metadata,
    })
}

/// Trims an audio asset using isolated FFmpeg subprocess.
pub fn trim_audio(
    asset: &Asset,
    start: &str,
    end: &str,
    r: Ref,
    cache_dir: &Path,
) -> Result<Asset, AetherError> {
    let ext = asset.path.extension().and_then(|e| e.to_str()).unwrap_or("wav");

    // Calculate unique output hash based on command inputs
    let mut hasher = blake3::Hasher::new();
    hasher.update(asset.hash.as_bytes());
    hasher.update(start.as_bytes());
    hasher.update(end.as_bytes());
    let new_hash = hasher.finalize().to_hex().to_string();
    let output_path = cache_dir.join(format!("{}.{}", new_hash, ext));

    if !output_path.exists() {
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-ss", start,
                "-to", end,
                "-i", asset.path.to_str().unwrap(),
                "-c:a", "pcm_s16le",
                "-y",
                output_path.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| AetherError::MediaError(format!("Failed to run FFmpeg trim audio: {}", e)))?;

        if !status.success() {
            return Err(AetherError::MediaError(format!(
                "FFmpeg trim audio process exited with status {}",
                status
            )));
        }
    }

    let metadata = get_audio_metadata(&output_path)?;

    Ok(Asset {
        r,
        kind: AssetKind::Audio,
        path: output_path,
        hash: new_hash,
        metadata,
    })
}

/// Normalizes an audio asset based on ITU-R BS.1770 LUFS target using FFmpeg loudnorm filter.
pub fn normalize_audio(
    asset: &Asset,
    lufs: f32,
    true_peak: f32,
    r: Ref,
    cache_dir: &Path,
) -> Result<Asset, AetherError> {
    let ext = asset.path.extension().and_then(|e| e.to_str()).unwrap_or("wav");

    // Calculate unique output hash based on command inputs
    let mut hasher = blake3::Hasher::new();
    hasher.update(asset.hash.as_bytes());
    hasher.update(format!("{:.1}", lufs).as_bytes());
    hasher.update(format!("{:.1}", true_peak).as_bytes());
    let new_hash = hasher.finalize().to_hex().to_string();
    let output_path = cache_dir.join(format!("{}.{}", new_hash, ext));

    if !output_path.exists() {
        let sample_rate = asset.metadata["sample_rate"].as_u64().unwrap_or(44100);
        let filter_str = format!("loudnorm=I={:.1}:TP={:.1}", lufs, true_peak);
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-i", asset.path.to_str().unwrap(),
                "-filter:a", &filter_str,
                "-ar", &sample_rate.to_string(),
                "-c:a", "pcm_s16le",
                "-y",
                output_path.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| AetherError::MediaError(format!("Failed to run FFmpeg loudnorm audio: {}", e)))?;

        if !status.success() {
            return Err(AetherError::MediaError(format!(
                "FFmpeg loudnorm audio process exited with status {}",
                status
            )));
        }
    }


    let metadata = get_audio_metadata(&output_path)?;

    Ok(Asset {
        r,
        kind: AssetKind::Audio,
        path: output_path,
        hash: new_hash,
        metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::RefKind;
    use std::path::PathBuf;

    fn temp_test_dir() -> PathBuf {
        let unique_dir = format!("test_audio_{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos());
        std::env::temp_dir().join(unique_dir)
    }

    fn generate_synthetic_wav(output_path: &Path) {
        if let Some(parent) = output_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-f", "lavfi",
                "-i", "sine=duration=2:frequency=1000",
                "-c:a", "pcm_s16le",
                "-y",
                output_path.to_str().unwrap(),
            ])
            .status()
            .expect("Failed to run FFmpeg synthetic audio generator");
        assert!(status.success(), "FFmpeg synthetic audio generator failed");
    }

    #[test]
    fn test_audio_metadata_extraction() {
        let dir = temp_test_dir();
        let audio_path = dir.join("synthetic.wav");
        generate_synthetic_wav(&audio_path);

        let metadata = get_audio_metadata(&audio_path).unwrap();
        assert_eq!(metadata["sample_rate"].as_u64().unwrap(), 44100);
        assert!((metadata["duration"].as_f64().unwrap() - 2.0).abs() < 0.1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_audio_import_and_trim() {
        let dir = temp_test_dir();
        let audio_path = dir.join("synthetic.wav");
        generate_synthetic_wav(&audio_path);

        let cache_dir = dir.join("cache");
        let r1 = Ref { kind: RefKind::Audio, id: 1 };
        let asset1 = import_audio(&audio_path, r1, &cache_dir).unwrap();

        assert_eq!(asset1.r, r1);
        assert_eq!(asset1.kind, AssetKind::Audio);
        assert!(asset1.path.exists());
        assert_eq!(asset1.metadata["sample_rate"].as_u64().unwrap(), 44100);

        // Trim from 0.5s to 1.5s
        let r2 = Ref { kind: RefKind::Audio, id: 2 };
        let asset2 = trim_audio(&asset1, "0.5", "1.5", r2, &cache_dir).unwrap();

        assert_eq!(asset2.r, r2);
        assert!(asset2.path.exists());
        assert!((asset2.metadata["duration"].as_f64().unwrap() - 1.0).abs() < 0.1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_audio_normalization() {
        let dir = temp_test_dir();
        let audio_path = dir.join("synthetic.wav");
        generate_synthetic_wav(&audio_path);

        let cache_dir = dir.join("cache");
        let r1 = Ref { kind: RefKind::Audio, id: 1 };
        let asset1 = import_audio(&audio_path, r1, &cache_dir).unwrap();

        // Normalize to -14 LUFS, -1.0 True Peak
        let r2 = Ref { kind: RefKind::Audio, id: 2 };
        let asset2 = normalize_audio(&asset1, -14.0, -1.0, r2, &cache_dir).unwrap();

        assert_eq!(asset2.r, r2);
        assert!(asset2.path.exists());
        assert_eq!(asset2.metadata["sample_rate"].as_u64().unwrap(), 44100);

        let _ = fs::remove_dir_all(&dir);
    }
}
