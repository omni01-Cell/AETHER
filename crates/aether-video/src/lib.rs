use std::fs;
use std::path::Path;
use ffmpeg_next as ffmpeg;
use blake3;
use aether_core::{AetherError, Ref, Asset, AssetKind};

/// Extracts detailed technical metadata from a video file using ffmpeg-next.
pub fn get_video_metadata<P: AsRef<Path>>(path: P) -> Result<serde_json::Value, AetherError> {
    ffmpeg::init().map_err(|e| AetherError::MediaError(format!("FFmpeg init failed: {}", e)))?;
    let ictx = ffmpeg::format::input(&path)
        .map_err(|e| AetherError::MediaError(format!("Failed to open video format: {}", e)))?;

    let (width, height, duration, fps) = if let Some(stream) = ictx.streams().best(ffmpeg::media::Type::Video) {
        let codec = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .map_err(|e| AetherError::MediaError(format!("Failed to extract video codec parameters: {}", e)))?;
        let video = codec.decoder().video()
            .map_err(|e| AetherError::MediaError(format!("Failed to retrieve video decoder: {}", e)))?;

        let w = video.width();
        let h = video.height();

        let time_base = stream.time_base();
        let dur = stream.duration();
        let d = if dur > 0 {
            (dur as f64 * f64::from(time_base.0) / f64::from(time_base.1)) as f32
        } else {
            (ictx.duration() as f64 / 1_000_000.0) as f32
        };

        let avg_frame_rate = stream.avg_frame_rate();
        let f = if avg_frame_rate.1 > 0 {
            (f64::from(avg_frame_rate.0) / f64::from(avg_frame_rate.1)) as f32
        } else {
            0.0f32
        };

        (w, h, d, f)
    } else {
        return Err(AetherError::MediaError("No video stream found in the source file".to_string()));
    };

    Ok(serde_json::json!({
        "width": width,
        "height": height,
        "duration": duration,
        "fps": fps,
    }))
}


/// Imports a video file using Content-Addressable Storage (CAS) with Blake3 hashing,
/// moving it to the cache directory, and retrieving metadata.
pub fn import_video<P: AsRef<Path>>(
    src_path: P,
    r: Ref,
    cache_dir: &Path,
) -> Result<Asset, AetherError> {
    let src = src_path.as_ref();
    if !src.exists() {
        return Err(AetherError::IoError(
            src.to_string_lossy().to_string(),
            "Source video file does not exist".to_string(),
        ));
    }

    // 1. Calculate Blake3 hash
    let mut file = fs::File::open(src)
        .map_err(|e| AetherError::IoError(src.to_string_lossy().to_string(), e.to_string()))?;
    let mut hasher = blake3::Hasher::new();
    std::io::copy(&mut file, &mut hasher)
        .map_err(|e| AetherError::IoError(src.to_string_lossy().to_string(), e.to_string()))?;
    let hash = hasher.finalize().to_hex().to_string();

    // 2. Fetch video metadata
    let metadata = get_video_metadata(src)?;

    // 3. Move/Copy to the cache directory
    if !cache_dir.exists() {
        fs::create_dir_all(cache_dir)
            .map_err(|e| AetherError::IoError(cache_dir.to_string_lossy().to_string(), e.to_string()))?;
    }
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
    let cache_file_name = format!("{}.{}", hash, ext);
    let cache_file_path = cache_dir.join(cache_file_name);

    if !cache_file_path.exists() {
        fs::copy(src, &cache_file_path)
            .map_err(|e| AetherError::IoError(src.to_string_lossy().to_string(), e.to_string()))?;
    }

    Ok(Asset {
        r,
        kind: AssetKind::Video,
        path: cache_file_path,
        hash,
        metadata,
    })
}

/// Trims a video asset using isolated FFmpeg subprocess re-encoding for high stability.
pub fn trim_video(
    asset: &Asset,
    start: &str,
    end: &str,
    r: Ref,
    cache_dir: &Path,
) -> Result<Asset, AetherError> {
    let ext = asset.path.extension().and_then(|e| e.to_str()).unwrap_or("mp4");

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
                "-c:v", "libx264",
                "-c:a", "aac",
                "-y",
                output_path.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| AetherError::MediaError(format!("Failed to run FFmpeg trim: {}", e)))?;

        if !status.success() {
            return Err(AetherError::MediaError(format!(
                "FFmpeg trim process exited with status {}",
                status
            )));
        }
    }

    let metadata = get_video_metadata(&output_path)?;

    Ok(Asset {
        r,
        kind: AssetKind::Video,
        path: output_path,
        hash: new_hash,
        metadata,
    })
}

/// Concatenates multiple video assets using a stable FFmpeg filter_complex concat structure.
pub fn concat_video(
    assets: &[Asset],
    r: Ref,
    cache_dir: &Path,
) -> Result<Asset, AetherError> {
    if assets.is_empty() {
        return Err(AetherError::MediaError("Cannot concatenate an empty list of video assets".to_string()));
    }
    if assets.len() == 1 {
        return Ok(assets[0].clone());
    }

    let mut hasher = blake3::Hasher::new();
    for asset in assets {
        hasher.update(asset.hash.as_bytes());
    }
    let new_hash = hasher.finalize().to_hex().to_string();
    let output_path = cache_dir.join(format!("{}.mp4", new_hash));

    if !output_path.exists() {
        let mut cmd = std::process::Command::new("ffmpeg");
        for asset in assets {
            cmd.arg("-i").arg(asset.path.to_str().unwrap());
        }

        let mut filter_complex = String::new();
        for i in 0..assets.len() {
            filter_complex.push_str(&format!("[{}:v][{}:a]", i, i));
        }
        filter_complex.push_str(&format!("concat=n={}:v=1:a=1[v][a]", assets.len()));

        let status = cmd
            .args([
                "-filter_complex", &filter_complex,
                "-map", "[v]",
                "-map", "[a]",
                "-c:v", "libx264",
                "-c:a", "aac",
                "-y",
                output_path.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| AetherError::MediaError(format!("Failed to run FFmpeg concat: {}", e)))?;

        if !status.success() {
            return Err(AetherError::MediaError(format!(
                "FFmpeg concat process exited with status {}",
                status
            )));
        }
    }

    let metadata = get_video_metadata(&output_path)?;

    Ok(Asset {
        r,
        kind: AssetKind::Video,
        path: output_path,
        hash: new_hash,
        metadata,
    })
}

/// Renders/exports a timeline video into a specific output path with codec and quality settings.
pub fn render_video(
    timeline_asset: &Asset,
    format: &str,
    codec: &str,
    quality: &str,
    output_path: &Path,
) -> Result<(), AetherError> {
    let crf = match quality {
        "high" => "18",
        "medium" => "23",
        "low" => "28",
        other => other,
    };

    let v_codec = match codec {
        "h264" => "libx264",
        "hevc" => "libx265",
        "vp9" => "libvpx-vp9",
        other => other,
    };

    // Make sure parent directory of output exists
    if let Some(parent) = output_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| AetherError::IoError(parent.to_string_lossy().to_string(), e.to_string()))?;
        }
    }

    let status = std::process::Command::new("ffmpeg")
        .args([
            "-i", timeline_asset.path.to_str().unwrap(),
            "-c:v", v_codec,
            "-crf", crf,
            "-c:a", "aac",
            "-f", format,
            "-y",
            output_path.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| AetherError::MediaError(format!("Failed to run FFmpeg render: {}", e)))?;

    if !status.success() {
        return Err(AetherError::MediaError(format!(
            "FFmpeg render process exited with status {}",
            status
        )));
    }

    Ok(())
}

/// Composites an overlay asset onto a base video asset at a specific time and position.
pub fn composite_video(
    base: &Asset,
    overlay: &Asset,
    at: &str,
    x: i32,
    y: i32,
    r: Ref,
    cache_dir: &Path,
) -> Result<Asset, AetherError> {
    let ext = base.path.extension().and_then(|e| e.to_str()).unwrap_or("mp4");

    // Calculate unique output hash based on inputs
    let mut hasher = blake3::Hasher::new();
    hasher.update(base.hash.as_bytes());
    hasher.update(overlay.hash.as_bytes());
    hasher.update(at.as_bytes());
    hasher.update(&x.to_le_bytes());
    hasher.update(&y.to_le_bytes());
    let new_hash = hasher.finalize().to_hex().to_string();
    let output_path = cache_dir.join(format!("{}.{}", new_hash, ext));

    if !output_path.exists() {
        let enable_clause = if at.is_empty() || at == "0" {
            "".to_string()
        } else if at.contains('(') || at.contains('=') {
            format!(":enable='{}'", at)
        } else {
            format!(":enable='gte(t,{})'", at)
        };

        let filter_str = format!("[0:v][1:v]overlay=x={}:y={}{}[v]", x, y, enable_clause);

        let status = std::process::Command::new("ffmpeg")
            .args([
                "-i", base.path.to_str().unwrap(),
                "-i", overlay.path.to_str().unwrap(),
                "-filter_complex", &filter_str,
                "-map", "[v]",
                "-map", "0:a?",
                "-c:v", "libx264",
                "-c:a", "aac",
                "-y",
                output_path.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| AetherError::MediaError(format!("Failed to run FFmpeg composite: {}", e)))?;

        if !status.success() {
            return Err(AetherError::MediaError(format!(
                "FFmpeg composite process exited with status {}",
                status
            )));
        }
    }

    let metadata = get_video_metadata(&output_path)?;

    Ok(Asset {
        r,
        kind: AssetKind::Video,
        path: output_path,
        hash: new_hash,
        metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use aether_core::RefKind;

    fn temp_test_dir() -> PathBuf {
        let unique_dir = format!("test_video_{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos());
        std::env::temp_dir().join(unique_dir)
    }

    fn generate_synthetic_mp4(output_path: &Path) {
        if let Some(parent) = output_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-f", "lavfi",
                "-i", "testsrc=duration=2:size=320x240:rate=30",
                "-f", "lavfi",
                "-i", "sine=duration=2:frequency=1000",
                "-c:v", "libx264",
                "-c:a", "aac",
                "-pix_fmt", "yuv420p",
                "-y",
                output_path.to_str().unwrap(),
            ])
            .status()
            .expect("Failed to run FFmpeg synthetic source generator");
        assert!(status.success(), "FFmpeg synthetic source generator failed");
    }

    #[test]
    fn test_metadata_extraction() {
        let dir = temp_test_dir();
        let video_path = dir.join("synthetic.mp4");
        generate_synthetic_mp4(&video_path);

        let metadata = get_video_metadata(&video_path).unwrap();
        assert_eq!(metadata["width"].as_u64().unwrap(), 320);
        assert_eq!(metadata["height"].as_u64().unwrap(), 240);
        assert!((metadata["duration"].as_f64().unwrap() - 2.0).abs() < 0.2);
        assert!((metadata["fps"].as_f64().unwrap() - 30.0).abs() < 0.1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_import_and_trim_video() {
        let dir = temp_test_dir();
        let video_path = dir.join("synthetic.mp4");
        generate_synthetic_mp4(&video_path);

        let cache_dir = dir.join("cache");
        let r1 = Ref { kind: RefKind::Video, id: 1 };
        let asset1 = import_video(&video_path, r1, &cache_dir).unwrap();

        assert_eq!(asset1.r, r1);
        assert_eq!(asset1.kind, AssetKind::Video);
        assert!(asset1.path.exists());
        assert_eq!(asset1.metadata["width"].as_u64().unwrap(), 320);

        // Trim from 0.5s to 1.5s
        let r2 = Ref { kind: RefKind::Video, id: 2 };
        let asset2 = trim_video(&asset1, "0.5", "1.5", r2, &cache_dir).unwrap();

        assert_eq!(asset2.r, r2);
        assert!(asset2.path.exists());
        assert!((asset2.metadata["duration"].as_f64().unwrap() - 1.0).abs() < 0.25);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_concat_and_render_video() {
        let dir = temp_test_dir();
        let video_path = dir.join("synthetic.mp4");
        generate_synthetic_mp4(&video_path);

        let cache_dir = dir.join("cache");
        let r1 = Ref { kind: RefKind::Video, id: 1 };
        let asset1 = import_video(&video_path, r1, &cache_dir).unwrap();

        // Concat copy with itself
        let r2 = Ref { kind: RefKind::Video, id: 2 };
        let asset2 = concat_video(&[asset1.clone(), asset1], r2, &cache_dir).unwrap();

        assert_eq!(asset2.r, r2);
        assert!(asset2.path.exists());
        assert!((asset2.metadata["duration"].as_f64().unwrap() - 4.0).abs() < 0.35);

        // Test compositing
        let r3 = Ref { kind: RefKind::Video, id: 3 };
        let comp_asset = composite_video(&asset2, &asset2, "1.0", 10, 20, r3, &cache_dir).unwrap();
        assert_eq!(comp_asset.r, r3);
        assert!(comp_asset.path.exists());

        // Export render
        let render_path = dir.join("output.mp4");
        render_video(&comp_asset, "mp4", "h264", "high", &render_path).unwrap();
        assert!(render_path.exists());

        let _ = fs::remove_dir_all(&dir);
    }
}

