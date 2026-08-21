use std::path::Path;
use aether_core::{AetherError, Ref, Asset, AssetKind};
use blake3;

fn get_asset_duration(asset: &Asset) -> f32 {
    asset.metadata.get("duration")
        .and_then(|v| v.as_f64())
        .map(|d| d as f32)
        .unwrap_or(5.0)
}

fn has_audio_stream(asset: &Asset) -> bool {
    asset.metadata.get("has_audio")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn run_xfade(
    v1: &Asset,
    v2: &Asset,
    transition_name: &str,
    duration_sec: f32,
    output_path: &Path,
) -> Result<(), AetherError> {
    let dur1 = get_asset_duration(v1);
    let offset = (dur1 - duration_sec).max(0.0);
    
    let has_audio1 = has_audio_stream(v1);
    let has_audio2 = has_audio_stream(v2);
    
    let mut args = vec![
        "-i".to_string(),
        v1.path.to_string_lossy().to_string(),
        "-i".to_string(),
        v2.path.to_string_lossy().to_string(),
    ];
    
    let filter_str = if has_audio1 && has_audio2 {
        format!(
            "[0:v][1:v]xfade=transition={}:duration={:.2}:offset={:.2}[v];[0:a][1:a]acrossfade=d={:.2}[a]",
            transition_name, duration_sec, offset, duration_sec
        )
    } else {
        format!(
            "[0:v][1:v]xfade=transition={}:duration={:.2}:offset={:.2}[v]",
            transition_name, duration_sec, offset
        )
    };
    
    args.push("-filter_complex".to_string());
    args.push(filter_str);
    args.push("-map".to_string());
    args.push("[v]".to_string());
    
    if has_audio1 && has_audio2 {
        args.push("-map".to_string());
        args.push("[a]".to_string());
    } else if has_audio1 {
        args.push("-map".to_string());
        args.push("0:a".to_string());
    } else if has_audio2 {
        args.push("-map".to_string());
        args.push("1:a".to_string());
    }
    
    args.push("-c:v".to_string());
    args.push("libx264".to_string());
    args.push("-c:a".to_string());
    args.push("aac".to_string());
    args.push("-y".to_string());
    args.push(output_path.to_string_lossy().to_string());
    
    let status = std::process::Command::new("ffmpeg")
        .args(&args)
        .status()
        .map_err(|e| AetherError::MediaError(format!("Failed to run FFmpeg xfade: {}", e)))?;
        
    if !status.success() {
        return Err(AetherError::MediaError(format!(
            "FFmpeg xfade ({}) process failed with status {}",
            transition_name, status
        )));
    }
    
    Ok(())
}

pub fn render_crossfade(
    v1: &Asset,
    v2: &Asset,
    duration_sec: f32,
    output_path: &Path,
) -> Result<(), AetherError> {
    run_xfade(v1, v2, "fade", duration_sec, output_path)
}

pub fn render_wipe(
    v1: &Asset,
    v2: &Asset,
    direction: &str,
    duration_sec: f32,
    output_path: &Path,
) -> Result<(), AetherError> {
    let trans = match direction.to_lowercase().as_str() {
        "left" => "wipeleft",
        "right" => "wiperight",
        "up" => "wipeup",
        "down" => "wipedown",
        _ => "wipeleft",
    };
    run_xfade(v1, v2, trans, duration_sec, output_path)
}

pub fn render_dissolve(
    v1: &Asset,
    v2: &Asset,
    duration_sec: f32,
    output_path: &Path,
) -> Result<(), AetherError> {
    run_xfade(v1, v2, "dissolve", duration_sec, output_path)
}

pub fn render_slide(
    v1: &Asset,
    v2: &Asset,
    direction: &str,
    duration_sec: f32,
    output_path: &Path,
) -> Result<(), AetherError> {
    let trans = match direction.to_lowercase().as_str() {
        "left" => "slideleft",
        "right" => "slideright",
        "up" => "slideup",
        "down" => "slidedown",
        _ => "slideleft",
    };
    run_xfade(v1, v2, trans, duration_sec, output_path)
}

pub fn render_glitch(
    v1: &Asset,
    v2: &Asset,
    duration_sec: f32,
    output_path: &Path,
) -> Result<(), AetherError> {
    run_xfade(v1, v2, "hlslice", duration_sec, output_path)
}

pub fn render_blur_transition(
    v1: &Asset,
    v2: &Asset,
    duration_sec: f32,
    output_path: &Path,
) -> Result<(), AetherError> {
    run_xfade(v1, v2, "zoomwarp", duration_sec, output_path)
}

pub fn render_zoom_transition(
    v1: &Asset,
    v2: &Asset,
    duration_sec: f32,
    output_path: &Path,
) -> Result<(), AetherError> {
    run_xfade(v1, v2, "crosszoom", duration_sec, output_path)
}

pub fn change_speed(
    asset: &Asset,
    factor: f32,
    r: Ref,
    cache_dir: &Path,
) -> Result<Asset, AetherError> {
    let ext = asset.path.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
    
    let mut hasher = blake3::Hasher::new();
    hasher.update(asset.hash.as_bytes());
    hasher.update(format!("{:.2}", factor).as_bytes());
    let new_hash = hasher.finalize().to_hex().to_string();
    let output_path = cache_dir.join(format!("{}.{}", new_hash, ext));
    
    if !output_path.exists() {
        let video_filter = format!("setpts={}*PTS", 1.0 / factor);
        
        let mut audio_filter = String::new();
        let mut current_factor = factor;
        while current_factor > 2.0 {
            if !audio_filter.is_empty() { audio_filter.push(','); }
            audio_filter.push_str("atempo=2.0");
            current_factor /= 2.0;
        }
        while current_factor < 0.5 {
            if !audio_filter.is_empty() { audio_filter.push(','); }
            audio_filter.push_str("atempo=0.5");
            current_factor /= 0.5;
        }
        if current_factor != 1.0 {
            if !audio_filter.is_empty() { audio_filter.push(','); }
            audio_filter.push_str(&format!("atempo={:.2}", current_factor));
        }
        
        let has_audio = has_audio_stream(asset);
        
        let mut args = vec![
            "-i".to_string(),
            asset.path.to_string_lossy().to_string(),
            "-filter:v".to_string(),
            video_filter,
        ];
        
        if has_audio && !audio_filter.is_empty() {
            args.push("-filter:a".to_string());
            args.push(audio_filter);
        }
        
        args.push("-c:v".to_string());
        args.push("libx264".to_string());
        
        if has_audio {
            args.push("-c:a".to_string());
            args.push("aac".to_string());
        }
        
        args.push("-y".to_string());
        args.push(output_path.to_string_lossy().to_string());
        
        let status = std::process::Command::new("ffmpeg")
            .args(&args)
            .status()
            .map_err(|e| AetherError::MediaError(format!("Failed to run FFmpeg change_speed: {}", e)))?;
            
        if !status.success() {
            return Err(AetherError::MediaError(format!(
                "FFmpeg change_speed process exited with status {}",
                status
            )));
        }
    }
    
    let mut metadata = crate::get_video_metadata(&output_path)?;
    if let Some(obj) = metadata.as_object_mut() {
        obj.insert("speed_factor".to_string(), serde_json::json!(factor));
        obj.insert("parent_ref".to_string(), serde_json::json!(asset.r.to_string()));
    }
    
    Ok(Asset {
        r,
        kind: AssetKind::Video,
        path: output_path,
        hash: new_hash,
        metadata,
    })
}
