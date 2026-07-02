use aether_core::{AetherError, Timeline, TrackKind};
use std::fs;
use std::path::Path;

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
        fs::create_dir_all(parent).map_err(|e| {
            AetherError::IoError(parent.to_string_lossy().to_string(), e.to_string())
        })?;
    }
    let formatted_json = serde_json::to_string_pretty(&otio_json)
        .map_err(|e| AetherError::OperationFailed(format!("Failed to format OTIO JSON: {}", e)))?;
    fs::write(path, formatted_json)
        .map_err(|e| AetherError::IoError(output_path.to_string(), e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::{Clip, Ref, RefKind, Track};
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        let unique_dir = format!(
            "test_otio_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        std::env::temp_dir().join(unique_dir)
    }

    #[test]
    fn test_export_otio_success() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        let output_path = dir.join("test.otio");

        let r1 = Ref {
            kind: RefKind::Video,
            id: 1,
        };
        let r2 = Ref {
            kind: RefKind::Audio,
            id: 2,
        };

        let clip1 = Clip {
            asset_ref: r1,
            in_point_ms: 1000,  // 1 second
            out_point_ms: 3000, // 3 seconds (duration 2s)
            track_offset_ms: 0,
            transition: None,
        };

        let clip2 = Clip {
            asset_ref: r2,
            in_point_ms: 500,   // 0.5 seconds
            out_point_ms: 2500, // 2.5 seconds (duration 2s)
            track_offset_ms: 0,
            transition: None,
        };

        let track1 = Track {
            name: "V1".to_string(),
            kind: TrackKind::Video,
            clips: vec![clip1],
        };

        let track2 = Track {
            name: "A1".to_string(),
            kind: TrackKind::Audio,
            clips: vec![clip2],
        };

        let timeline = Timeline {
            tracks: vec![track1, track2],
        };

        let result = export_otio(&timeline, output_path.to_str().unwrap());
        assert!(result.is_ok());

        // Verify the file was created
        assert!(output_path.exists());

        // Parse and check structure
        let content = fs::read_to_string(&output_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(parsed["OTIO_SCHEMA"], "Timeline.1");
        assert_eq!(parsed["name"], "AETHER OTIO Export");

        let tracks = parsed["tracks"].as_array().unwrap();
        assert_eq!(tracks.len(), 2);

        // Check first track (Video)
        assert_eq!(tracks[0]["OTIO_SCHEMA"], "Track.1");
        assert_eq!(tracks[0]["kind"], "Video");
        assert_eq!(tracks[0]["name"], "V1");

        let t1_clips = tracks[0]["children"].as_array().unwrap();
        assert_eq!(t1_clips.len(), 1);
        assert_eq!(t1_clips[0]["OTIO_SCHEMA"], "Clip.1");
        // start time = 1.0s * 30.0 = 30.0
        assert_eq!(t1_clips[0]["source_range"]["start_time"]["value"], 30.0);
        // duration = 2.0s * 30.0 = 60.0
        assert_eq!(t1_clips[0]["source_range"]["duration"]["value"], 60.0);

        // Check second track (Audio)
        assert_eq!(tracks[1]["OTIO_SCHEMA"], "Track.1");
        assert_eq!(tracks[1]["kind"], "Audio");
        assert_eq!(tracks[1]["name"], "A1");

        let t2_clips = tracks[1]["children"].as_array().unwrap();
        assert_eq!(t2_clips.len(), 1);
        assert_eq!(t2_clips[0]["OTIO_SCHEMA"], "Clip.1");
        // start time = 0.5s * 30.0 = 15.0
        assert_eq!(t2_clips[0]["source_range"]["start_time"]["value"], 15.0);
        // duration = 2.0s * 30.0 = 60.0
        assert_eq!(t2_clips[0]["source_range"]["duration"]["value"], 60.0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_export_otio_invalid_path() {
        let timeline = Timeline { tracks: vec![] };

        // Use a path that is a directory and cannot be written as a file, or root etc.
        // Actually /dev/null/foo is a good cross-platformish failure for trying to create a dir.
        #[cfg(unix)]
        let invalid_path = "/dev/null/impossible.otio";
        #[cfg(windows)]
        let invalid_path = "Z:\\impossible_dir\\impossible.otio";

        let result = export_otio(&timeline, invalid_path);
        assert!(result.is_err());
        match result.unwrap_err() {
            AetherError::IoError(..) => {} // expected
            _ => panic!("Expected IoError"),
        }
    }
}
