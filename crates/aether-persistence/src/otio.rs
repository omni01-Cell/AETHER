use std::fs;
use std::path::Path;
use aether_core::{AetherError, Timeline, TrackKind};

/// Generates a valid JSON representation adhering to OpenTimelineIO (OTIO) schemas and writes it to the specified output path.
pub fn export_otio(timeline: &Timeline, output_path: &str) -> Result<(), AetherError> {
    let otio_json = serde_json::json!({
        "OTIO_SCHEMA": "Timeline.1",
        "metadata": {},
        "name": "AETHER OTIO Export",
        "tracks": timeline.tracks.iter().map(|track| {
            serde_json::json!({
                "OTIO_SCHEMA": "Track.1",
                "kind": match track.kind {
                    TrackKind::Video => "Video",
                    TrackKind::Audio => "Audio",
                },
                "name": track.name,
                "children": track.clips.iter().map(|clip| {
                    serde_json::json!({
                        "OTIO_SCHEMA": "Clip.1",
                        "name": clip.asset_ref.to_string(),
                        "source_range": {
                            "OTIO_SCHEMA": "TimeRange.1",
                            "duration": {
                                "OTIO_SCHEMA": "RationalTime.1",
                                "rate": 30.0,
                                "value": ((clip.out_point_ms - clip.in_point_ms) as f64 * 30.0 / 1000.0)
                            },
                            "start_time": {
                                "OTIO_SCHEMA": "RationalTime.1",
                                "rate": 30.0,
                                "value": (clip.in_point_ms as f64 * 30.0 / 1000.0)
                            }
                        }
                    })
                }).collect::<Vec<_>>()
            })
        }).collect::<Vec<_>>()
    });
    
    let path = Path::new(output_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AetherError::IoError(parent.to_string_lossy().to_string(), e.to_string()))?;
    }
    let formatted_json = serde_json::to_string_pretty(&otio_json)
        .map_err(|e| AetherError::OperationFailed(format!("Failed to format OTIO JSON: {}", e)))?;
    fs::write(path, formatted_json).map_err(|e| AetherError::IoError(output_path.to_string(), e.to_string()))?;
    Ok(())
}
