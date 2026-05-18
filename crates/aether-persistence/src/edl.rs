use std::fs;
use std::path::Path;
use aether_core::{AetherError, Timeline, RefRegistry};

/// Helper to format timecode from frames and frame rate.
pub fn format_timecode(frames: u64, fps: f64) -> String {
    let fps_u = fps as u64;
    let total_secs = frames / fps_u;
    let f = frames % fps_u;
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    format!("{:02}:{:02}:{:02}:{:02}", h, m, s, f)
}

/// Generates a standard CMX 3600 Edit Decision List (EDL) text representing the timeline, and writes it to the specified output path.
pub fn export_edl(timeline: &Timeline, registry: &RefRegistry, output_path: &str) -> Result<(), AetherError> {
    let mut edl = String::new();
    edl.push_str("TITLE: AETHER Project\nFCM: NON-DROP FRAME\n\n");
    let mut event_count = 1;
    for track in &timeline.tracks {
        for clip in &track.clips {
            let asset = registry.resolve(&clip.asset_ref)?;
            let name = asset.path.file_name().and_then(|n| n.to_str()).unwrap_or("UNKNOWN");
            
            let in_frames = (clip.in_point_ms as f64 * 30.0 / 1000.0) as u64;
            let out_frames = (clip.out_point_ms as f64 * 30.0 / 1000.0) as u64;
            let offset_frames = (clip.track_offset_ms as f64 * 30.0 / 1000.0) as u64;
            let duration_frames = out_frames - in_frames;
            
            let in_tc = format_timecode(in_frames, 30.0);
            let out_tc = format_timecode(out_frames, 30.0);
            let start_tc = format_timecode(offset_frames, 30.0);
            let end_tc = format_timecode(offset_frames + duration_frames, 30.0);
            
            edl.push_str(&format!(
                "{:03}  AX       V     C        {} {} {} {}\n* FROM CLIP NAME: {}\n\n",
                event_count, in_tc, out_tc, start_tc, end_tc, name
            ));
            event_count += 1;
        }
    }
    
    let path = Path::new(output_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AetherError::IoError(parent.to_string_lossy().to_string(), e.to_string()))?;
    }
    fs::write(path, edl).map_err(|e| AetherError::IoError(output_path.to_string(), e.to_string()))?;
    Ok(())
}
