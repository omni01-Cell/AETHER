use biquad::{Coefficients, DirectForm1, Type, ToHertz, Biquad};

pub struct BiquadFilter {
    filters: Vec<DirectForm1<f32>>,
}

impl BiquadFilter {
    pub fn new(
        filter_type: &str,
        freq_hz: f32,
        gain_db: f32,
        q: f32,
        sample_rate: u32,
        channels: usize,
    ) -> Result<Self, String> {
        let fs = (sample_rate as f32).hz();
        let f0 = freq_hz.hz();
        
        let t = match filter_type {
            "LowPass" => Type::LowPass,
            "HighPass" => Type::HighPass,
            "BandPass" => Type::BandPass,
            "PeakingEQ" => Type::PeakingEQ(gain_db),
            "LowShelf" => Type::LowShelf(gain_db),
            "HighShelf" => Type::HighShelf(gain_db),
            "Notch" => Type::Notch,
            "AllPass" => Type::AllPass,
            _ => return Err(format!("Unknown filter type: {}", filter_type)),
        };
        
        let coeffs = Coefficients::<f32>::from_params(t, fs, f0, q)
            .map_err(|e| format!("Failed to compute biquad coefficients: {:?}", e))?;
            
        let mut filters = Vec::with_capacity(channels);
        for _ in 0..channels {
            filters.push(DirectForm1::<f32>::new(coeffs));
        }
        
        Ok(BiquadFilter { filters })
    }
    
    pub fn process(&mut self, samples: &mut [Vec<f32>]) {
        // OPTIMIZATION: Replacing index-based iteration (e.g., `samples[ch]`)
        // with `iter_mut().enumerate()` over the channels slice.
        // Impact: Eliminates runtime bounds checking for every channel iteration,
        // which can marginally improve throughput for multi-channel processing paths.
        for (ch, channel_samples) in samples.iter_mut().enumerate() {
            if ch < self.filters.len() {
                let filter = &mut self.filters[ch];
                for sample in channel_samples.iter_mut() {
                    *sample = filter.run(*sample);
                }
            }
        }
    }
}

pub struct DynamicCompressor {
    threshold_db: f32,
    ratio: f32,
    attack_s: f32,
    release_s: f32,
    sample_rate: f32,
    envelope: Vec<f32>,
}

impl DynamicCompressor {
    pub fn new(
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        sample_rate: u32,
        channels: usize,
    ) -> Self {
        DynamicCompressor {
            threshold_db,
            ratio,
            attack_s: attack_ms / 1000.0,
            release_s: release_ms / 1000.0,
            sample_rate: sample_rate as f32,
            envelope: vec![0.0; channels],
        }
    }
    
    pub fn process(&mut self, samples: &mut [Vec<f32>]) {
        let att_coef = (-1.0 / (self.sample_rate * self.attack_s)).exp();
        let rel_coef = (-1.0 / (self.sample_rate * self.release_s)).exp();
        
        // OPTIMIZATION: Using `iter_mut().enumerate()` over `samples` slice instead of
        // indexing (`samples[ch]`).
        // Impact: Eliminates runtime bounds checking for every channel iteration,
        // which can marginally improve throughput for multi-channel processing paths.
        for (ch, channel_samples) in samples.iter_mut().enumerate() {
            if ch >= self.envelope.len() {
                self.envelope.push(0.0);
            }
            let env = &mut self.envelope[ch];
            
            for sample in channel_samples.iter_mut() {
                let input_mag = sample.abs();
                
                if input_mag > *env {
                    *env = att_coef * (*env) + (1.0 - att_coef) * input_mag;
                } else {
                    *env = rel_coef * (*env) + (1.0 - rel_coef) * input_mag;
                }
                
                let env_db = if *env > 1e-5 {
                    20.0 * env.log10()
                } else {
                    -100.0
                };
                
                let gain_reduction_db = if env_db > self.threshold_db {
                    let overshoot = env_db - self.threshold_db;
                    let target_gain_db = self.threshold_db + overshoot / self.ratio;
                    target_gain_db - env_db
                } else {
                    0.0
                };
                
                let gain_linear = 10.0f32.powf(gain_reduction_db / 20.0);
                *sample *= gain_linear;
            }
        }
    }
}

pub struct MultiTrackMixer;

impl MultiTrackMixer {
    pub fn mix(
        tracks: &[Vec<Vec<f32>>],
        sample_rates: &[u32],
        volumes: &[f32],
        pans: &[f32],
        target_sample_rate: u32,
    ) -> Result<Vec<Vec<f32>>, String> {
        if tracks.is_empty() {
            return Ok(vec![Vec::new(), Vec::new()]);
        }
        
        let mut resampled_tracks = Vec::new();
        for (i, track) in tracks.iter().enumerate() {
            let sr = sample_rates[i];
            if sr == target_sample_rate {
                resampled_tracks.push(track.clone());
            } else {
                let resampled = Self::resample_track(track, sr, target_sample_rate)?;
                resampled_tracks.push(resampled);
            }
        }
        
        let max_len = resampled_tracks.iter().map(|t| if t.is_empty() { 0 } else { t[0].len() }).max().unwrap_or(0);
        
        let mut mixed = vec![vec![0.0; max_len], vec![0.0; max_len]];
        
        for (i, track) in resampled_tracks.iter().enumerate() {
            if track.is_empty() {
                continue;
            }
            let vol = volumes.get(i).copied().unwrap_or(1.0);
            let pan = pans.get(i).copied().unwrap_or(0.0).clamp(-1.0, 1.0);
            
            let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
            let left_pan = angle.cos();
            let right_pan = angle.sin();
            
            let track_ch = track.len();
            let len = track[0].len();
            
            for sample_idx in 0..len {
                let (l_sample, r_sample) = if track_ch == 1 {
                    let s = track[0][sample_idx];
                    (s, s)
                } else {
                    let l = track[0][sample_idx];
                    let r = track[1][sample_idx];
                    (l, r)
                };
                
                mixed[0][sample_idx] += l_sample * vol * left_pan;
                mixed[1][sample_idx] += r_sample * vol * right_pan;
            }
        }
        
        Ok(mixed)
    }
    
    fn resample_track(
        track: &[Vec<f32>],
        from_rate: u32,
        to_rate: u32,
    ) -> Result<Vec<Vec<f32>>, String> {
        if track.is_empty() {
            return Ok(Vec::new());
        }
        
        let ratio = to_rate as f64 / from_rate as f64;
        let channels = track.len();
        let input_len = track[0].len();
        let output_len = (input_len as f64 * ratio).round() as usize;
        
        let mut output = vec![vec![0.0; output_len]; channels];
        
        for ch in 0..channels {
            for i in 0..output_len {
                let src_idx = i as f64 / ratio;
                let low = src_idx.floor() as usize;
                let high = src_idx.ceil() as usize;
                let frac = src_idx - low as f64;
                
                if low < input_len && high < input_len {
                    let sample_low = track[ch][low];
                    let sample_high = track[ch][high];
                    output[ch][i] = sample_low + (sample_high - sample_low) * frac as f32;
                } else if low < input_len {
                    output[ch][i] = track[ch][low];
                }
            }
        }
        
        Ok(output)
    }
}
