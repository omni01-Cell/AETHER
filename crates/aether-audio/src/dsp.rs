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
        let channels = samples.len();
        for ch in 0..channels {
            if ch < self.filters.len() {
                let filter = &mut self.filters[ch];
                for sample in &mut samples[ch] {
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
        let channels = samples.len();
        let att_coef = (-1.0 / (self.sample_rate * self.attack_s)).exp();
        let rel_coef = (-1.0 / (self.sample_rate * self.release_s)).exp();
        
        // Optimization (Bolt): Pre-calculate threshold and slope in linear space
        // to avoid costly log10() and powf() per sample.
        let threshold_lin = 10.0f32.powf(self.threshold_db / 20.0);
        let slope = 1.0 / self.ratio - 1.0;

        for ch in 0..channels {
            if ch >= self.envelope.len() {
                self.envelope.push(0.0);
            }
            let env = &mut self.envelope[ch];
            
            for sample in &mut samples[ch] {
                let input_mag = sample.abs();
                
                if input_mag > *env {
                    *env = att_coef * (*env) + (1.0 - att_coef) * input_mag;
                } else {
                    *env = rel_coef * (*env) + (1.0 - rel_coef) * input_mag;
                }
                
                // Optimization (Bolt): Apply gain reduction in linear domain
                let gain_linear = if *env > threshold_lin {
                    (*env / threshold_lin).powf(slope)
                } else {
                    1.0
                };
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
            
            // Optimization (Bolt): Pre-calculate multipliers, loop unswitching for track_ch,
            // and use split_at_mut/zip to elide runtime bounds checks.
            let l_mult = vol * left_pan;
            let r_mult = vol * right_pan;
            let (mix_l, mix_r) = mixed.split_at_mut(1);

            if track_ch == 1 {
                for (idx, &s) in track[0].iter().enumerate() {
                    mix_l[0][idx] += s * l_mult;
                    mix_r[0][idx] += s * r_mult;
                }
            } else {
                for (idx, (&l, &r)) in track[0].iter().zip(track[1].iter()).enumerate() {
                    mix_l[0][idx] += l * l_mult;
                    mix_r[0][idx] += r * r_mult;
                }
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
        
        // Optimization (Bolt): Precalculate inverse ratio, use iter_mut to elide bounds checks,
        // and partition loop into fast/slow paths to avoid branches in hot loop.
        let inv_ratio = 1.0 / ratio;
        let safe_output_len = if input_len > 1 {
            ((input_len as f64 - 1.0) * ratio).ceil() as usize
        } else {
            0
        };
        let safe_len = safe_output_len.min(output_len);

        for ch in 0..channels {
            let ch_track = &track[ch];
            let ch_out = &mut output[ch];

            let mut i = 0;

            // Fast path (no bounds checks on high)
            for out_sample in ch_out.iter_mut().take(safe_len) {
                let src_idx = i as f64 * inv_ratio;
                let low = src_idx as usize;
                let high = low + 1;
                let frac = (src_idx - low as f64) as f32;

                let sample_low = ch_track[low];
                let sample_high = ch_track[high];
                *out_sample = sample_low + (sample_high - sample_low) * frac;
                i += 1;
            }

            // Slow path (handles edge cases at end of track)
            for out_sample in ch_out.iter_mut().skip(safe_len) {
                let src_idx = i as f64 * inv_ratio;
                let low = src_idx as usize;
                let high = low + 1;
                let frac = (src_idx - low as f64) as f32;
                
                if high < input_len {
                    let sample_low = ch_track[low];
                    let sample_high = ch_track[high];
                    *out_sample = sample_low + (sample_high - sample_low) * frac;
                } else if low < input_len {
                    *out_sample = ch_track[low];
                }
                i += 1;
            }
        }
        
        Ok(output)
    }
}
