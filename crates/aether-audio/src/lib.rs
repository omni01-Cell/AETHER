pub mod dsp;

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
        let ffmpeg_res = std::process::Command::new("ffmpeg")
            .args([
                "-ss", start,
                "-to", end,
                "-i", asset.path.to_str().unwrap(),
                "-c:a", "pcm_s16le",
                "-y",
                output_path.to_str().unwrap(),
            ])
            .status();

        let success = match ffmpeg_res {
            Ok(status) => status.success(),
            Err(_) => false,
        };

        if !success {
            let (samples, sample_rate) = decode_audio_file(&asset.path)?;
            let start_sec: f32 = start.parse().unwrap_or(0.0);
            let end_sec: f32 = end.parse().unwrap_or(0.0);
            let start_idx = (start_sec * sample_rate as f32) as usize;
            let end_idx = if end_sec > 0.0 {
                (end_sec * sample_rate as f32) as usize
            } else {
                samples.first().map(|ch| ch.len()).unwrap_or(0)
            };

            let mut trimmed_samples = Vec::new();
            for ch in &samples {
                let s_idx = start_idx.min(ch.len());
                let e_idx = end_idx.min(ch.len()).max(s_idx);
                trimmed_samples.push(ch[s_idx..e_idx].to_vec());
            }

            encode_audio_file(&trimmed_samples, sample_rate, &output_path)?;
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
        let ffmpeg_res = std::process::Command::new("ffmpeg")
            .args([
                "-i", asset.path.to_str().unwrap(),
                "-filter:a", &filter_str,
                "-ar", &sample_rate.to_string(),
                "-c:a", "pcm_s16le",
                "-y",
                output_path.to_str().unwrap(),
            ])
            .status();

        let success = match ffmpeg_res {
            Ok(status) => status.success(),
            Err(_) => false,
        };

        if !success {
            let (mut samples, sr) = decode_audio_file(&asset.path)?;
            let target_linear = 10.0f32.powf(true_peak / 20.0);
            let mut current_peak = 0.0f32;
            for ch in &samples {
                for &s in ch {
                    if s.abs() > current_peak {
                        current_peak = s.abs();
                    }
                }
            }
            if current_peak > 0.0 {
                let gain = target_linear / current_peak;
                for ch in &mut samples {
                    for s in ch {
                        *s *= gain;
                    }
                }
            }
            encode_audio_file(&samples, sr, &output_path)?;
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

/// Decodes an entire audio file into separate channel buffers of f32 samples using Symphonia.
pub fn decode_audio_file<P: AsRef<Path>>(path: P) -> Result<(Vec<Vec<f32>>, u32), AetherError> {
    let p = path.as_ref();
    let file = std::fs::File::open(p)
        .map_err(|e| AetherError::IoError(p.to_string_lossy().to_string(), e.to_string()))?;
    let mss = symphonia::core::io::MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = symphonia::core::probe::Hint::new();
    if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &symphonia::core::formats::FormatOptions::default(), &symphonia::core::meta::MetadataOptions::default())
        .map_err(|e| AetherError::MediaError(format!("Failed to probe audio: {}", e)))?;
        
    let mut format = probed.format;
    let track = format.tracks().first()
        .ok_or_else(|| AetherError::MediaError("No audio tracks found".to_string()))?;
        
    let codec_params = &track.codec_params;
    let sample_rate = codec_params.sample_rate.unwrap_or(44100);
    let channels = codec_params.channels.map(|c| c.count()).unwrap_or(2);
    
    let mut decoder = symphonia::default::get_codecs()
        .make(codec_params, &symphonia::core::codecs::DecoderOptions::default())
        .map_err(|e| AetherError::MediaError(format!("Failed to create decoder: {}", e)))?;
        
    let track_id = track.id;
    let mut channel_samples = vec![Vec::new(); channels];
    
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => {
                continue;
            }
            Err(e) => return Err(AetherError::MediaError(format!("Failed to read packet: {}", e))),
        };
        
        if packet.track_id() != track_id {
            continue;
        }
        
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::DecodeError(e)) => {
                tracing::warn!("Decode error: {}, skipping packet", e);
                continue;
            }
            Err(e) => return Err(AetherError::MediaError(format!("Decoder error: {}", e))),
        };
        
        let mut sample_buf = symphonia::core::audio::SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
        sample_buf.copy_interleaved_ref(decoded);
        let samples = sample_buf.samples();
        
        for (i, &sample) in samples.iter().enumerate() {
            let ch = i % channels;
            channel_samples[ch].push(sample);
        }
    }
    
    Ok((channel_samples, sample_rate))
}

/// Encodes separate channel buffers of f32 samples into a standard 16-bit PCM WAV file using Hound.
pub fn encode_audio_file(samples: &[Vec<f32>], sample_rate: u32, path: &Path) -> Result<(), AetherError> {
    let channels = samples.len();
    if channels == 0 {
        return Err(AetherError::MediaError("Cannot write audio with 0 channels".to_string()));
    }
    
    let spec = hound::WavSpec {
        channels: channels as u16,
        sample_rate: sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    
    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| AetherError::IoError(path.to_string_lossy().to_string(), e.to_string()))?;
        
    let len = samples[0].len();
    for i in 0..len {
        for ch in 0..channels {
            let sample = samples[ch].get(i).copied().unwrap_or(0.0);
            let sample_i16 = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            writer.write_sample(sample_i16)
                .map_err(|e| AetherError::IoError(path.to_string_lossy().to_string(), e.to_string()))?;
        }
    }
    
    writer.finalize()
        .map_err(|e| AetherError::IoError(path.to_string_lossy().to_string(), e.to_string()))?;
        
    Ok(())
}

/// Applies EQ (Biquad Filter) to an audio asset and saves the result as a new asset.
pub fn apply_eq(
    asset: &Asset,
    filter_type: &str,
    freq_hz: f32,
    gain_db: f32,
    q: f32,
    r: Ref,
    cache_dir: &Path,
) -> Result<Asset, AetherError> {
    let ext = asset.path.extension().and_then(|e| e.to_str()).unwrap_or("wav");
    
    let mut hasher = blake3::Hasher::new();
    hasher.update(asset.hash.as_bytes());
    hasher.update(filter_type.as_bytes());
    hasher.update(format!("{:.1}", freq_hz).as_bytes());
    hasher.update(format!("{:.1}", gain_db).as_bytes());
    hasher.update(format!("{:.1}", q).as_bytes());
    let new_hash = hasher.finalize().to_hex().to_string();
    let output_path = cache_dir.join(format!("{}.{}", new_hash, ext));
    
    if !output_path.exists() {
        let (mut samples, sample_rate) = decode_audio_file(&asset.path)?;
        let channels = samples.len();
        
        let mut filter = dsp::BiquadFilter::new(filter_type, freq_hz, gain_db, q, sample_rate, channels)
            .map_err(|e| AetherError::MediaError(e))?;
        filter.process(&mut samples);
        
        encode_audio_file(&samples, sample_rate, &output_path)?;
    }
    
    let metadata = get_audio_metadata(&output_path)?;
    
    Ok(Asset {
        r,
        kind: AssetKind::Audio,
        path: output_path,
        hash: new_hash,
        metadata: serde_json::json!({
            "duration": metadata["duration"].as_f64().unwrap_or(0.0),
            "eq": {
                "filter_type": filter_type,
                "freq_hz": freq_hz,
                "gain_db": gain_db,
                "q": q,
            },
            "parent_ref": asset.r.to_string(),
        }),
    })
}

/// Applies dynamic compression to an audio asset and saves the result as a new asset.
pub fn apply_compressor(
    asset: &Asset,
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    r: Ref,
    cache_dir: &Path,
) -> Result<Asset, AetherError> {
    let ext = asset.path.extension().and_then(|e| e.to_str()).unwrap_or("wav");
    
    let mut hasher = blake3::Hasher::new();
    hasher.update(asset.hash.as_bytes());
    hasher.update(format!("{:.1}", threshold_db).as_bytes());
    hasher.update(format!("{:.1}", ratio).as_bytes());
    hasher.update(format!("{:.1}", attack_ms).as_bytes());
    hasher.update(format!("{:.1}", release_ms).as_bytes());
    let new_hash = hasher.finalize().to_hex().to_string();
    let output_path = cache_dir.join(format!("{}.{}", new_hash, ext));
    
    if !output_path.exists() {
        let (mut samples, sample_rate) = decode_audio_file(&asset.path)?;
        let channels = samples.len();
        
        let mut compressor = dsp::DynamicCompressor::new(threshold_db, ratio, attack_ms, release_ms, sample_rate, channels);
        compressor.process(&mut samples);
        
        encode_audio_file(&samples, sample_rate, &output_path)?;
    }
    
    let metadata = get_audio_metadata(&output_path)?;
    
    Ok(Asset {
        r,
        kind: AssetKind::Audio,
        path: output_path,
        hash: new_hash,
        metadata: serde_json::json!({
            "duration": metadata["duration"].as_f64().unwrap_or(0.0),
            "compressor": {
                "threshold_db": threshold_db,
                "ratio": ratio,
                "attack_ms": attack_ms,
                "release_ms": release_ms,
            },
            "parent_ref": asset.r.to_string(),
        }),
    })
}

/// Mixes multiple track assets with given volumes and pans into a single stereo output wav.
pub fn mix_tracks(
    assets: &[Asset],
    volumes: &[f32],
    pans: &[f32],
    r: Ref,
    cache_dir: &Path,
) -> Result<Asset, AetherError> {
    let mut hasher = blake3::Hasher::new();
    for (i, asset) in assets.iter().enumerate() {
        hasher.update(asset.hash.as_bytes());
        let vol = volumes.get(i).copied().unwrap_or(1.0);
        let pan = pans.get(i).copied().unwrap_or(0.0);
        hasher.update(format!("{:.1}", vol).as_bytes());
        hasher.update(format!("{:.1}", pan).as_bytes());
    }
    let new_hash = hasher.finalize().to_hex().to_string();
    let output_path = cache_dir.join(format!("{}.wav", new_hash));
    
    if !output_path.exists() {
        let mut tracks = Vec::new();
        let mut sample_rates = Vec::new();
        
        for asset in assets {
            let (samples, sr) = decode_audio_file(&asset.path)?;
            tracks.push(samples);
            sample_rates.push(sr);
        }
        
        let mixed = dsp::MultiTrackMixer::mix(&tracks, &sample_rates, volumes, pans, 44100)
            .map_err(|e| AetherError::MediaError(e))?;
            
        encode_audio_file(&mixed, 44100, &output_path)?;
    }
    
    let metadata = get_audio_metadata(&output_path)?;
    let mut inputs = Vec::new();
    for (i, asset) in assets.iter().enumerate() {
        let vol = volumes.get(i).copied().unwrap_or(1.0);
        let pan = pans.get(i).copied().unwrap_or(0.0);
        inputs.push(serde_json::json!({
            "ref": asset.r.to_string(),
            "volume": vol,
            "pan": pan,
        }));
    }
    
    Ok(Asset {
        r,
        kind: AssetKind::Audio,
        path: output_path,
        hash: new_hash,
        metadata: serde_json::json!({
            "duration": metadata["duration"].as_f64().unwrap_or(0.0),
            "inputs": inputs,
        }),
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
        let sample_rate = 44100;
        let num_samples = sample_rate * 2;
        let mut samples = vec![vec![0.0f32; num_samples]];
        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            samples[0][i] = (2.0 * std::f32::consts::PI * 1000.0 * t).sin();
        }
        encode_audio_file(&samples, sample_rate as u32, output_path)
            .expect("Failed to write synthetic wav");
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

    #[test]
    fn test_sinus_highpass_attenuation() {
        let sample_rate = 44100;
        let freq = 1000.0;
        let mut samples = vec![vec![0.0; 44100]];
        for i in 0..44100 {
            let t = i as f32 / sample_rate as f32;
            samples[0][i] = (2.0 * std::f32::consts::PI * freq * t).sin();
        }
        
        let mut filter = dsp::BiquadFilter::new("HighPass", 2000.0, 0.0, 0.707, sample_rate, 1).unwrap();
        filter.process(&mut samples);
        
        let mut max_val = 0.0f32;
        for i in 2000..44100 {
            let val = samples[0][i].abs();
            if val > max_val {
                max_val = val;
            }
        }
        
        assert!(max_val > 0.22 && max_val < 0.28, "Expected attenuation ~12dB (amplitude ~0.25), got: {}", max_val);
    }

    #[test]
    fn test_compressor_gain_reduction() {
        let sample_rate = 44100;
        let mut samples = vec![vec![2.0; 44100]];
        
        let mut compressor = dsp::DynamicCompressor::new(-6.0, 4.0, 1.0, 100.0, sample_rate, 1);
        compressor.process(&mut samples);
        
        let stabilized_val = samples[0][40000];
        
        assert!(stabilized_val > 0.65 && stabilized_val < 0.75, "Expected compressed level ~0.707, got: {}", stabilized_val);
    }

    #[test]
    fn test_multitrack_mixing_pan() {
        let track1 = vec![vec![1.0; 1000]];
        let track2 = vec![vec![2.0; 1000]];
        let tracks = vec![track1, track2];
        let sample_rates = vec![44100, 44100];
        let volumes = vec![1.0, 1.0];
        let pans = vec![-1.0, 1.0];
        
        let mixed = dsp::MultiTrackMixer::mix(&tracks, &sample_rates, &volumes, &pans, 44100).unwrap();
        
        assert_eq!(mixed.len(), 2);
        assert_eq!(mixed[0].len(), 1000);
        assert_eq!(mixed[1].len(), 1000);
        
        assert!((mixed[0][0] - 1.0).abs() < 1e-4);
        assert!((mixed[1][0] - 2.0).abs() < 1e-4);
    }
}
