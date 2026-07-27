1.  *Refactor loop indexing in `crates/aether-audio/src/dsp.rs` and `crates/aether-audio/src/lib.rs`*
    - I will replace index-based loops in `dsp.rs` (lines 44-52, 88-121, 202-214) and `lib.rs` (lines 313-318) with iterator-based loops (`iter_mut`, `zip`, `enumerate`). This aligns with the memory instruction: "When optimizing Rust loops in this repository, prefer using iterators (e.g., `iter_mut().enumerate()`, `iter_mut().zip()`) over array/vector indexing. This is a highly effective optimization pattern that eliminates runtime bounds checks and improves performance, particularly in stateful DSP loops."
    - In `dsp.rs` around line 44:
      ```rust
      for (filter, channel_samples) in self.filters.iter_mut().zip(samples.iter_mut()) {
          for sample in channel_samples.iter_mut() {
              *sample = filter.run(*sample);
          }
      }
      ```
    - In `dsp.rs` around line 88:
      ```rust
      let channels = samples.len();
      if self.envelope.len() < channels {
          self.envelope.resize(channels, 0.0);
      }
      for (env, channel_samples) in self.envelope.iter_mut().zip(samples.iter_mut()) {
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
      ```
    - In `dsp.rs` around line 202:
      ```rust
      for (ch, channel_output) in output.iter_mut().enumerate() {
          for (i, out_sample) in channel_output.iter_mut().enumerate() {
              let src_idx = i as f64 / ratio;
              let low = src_idx.floor() as usize;
              let high = src_idx.ceil() as usize;
              let frac = src_idx - low as f64;

              if low < input_len && high < input_len {
                  let sample_low = track[ch][low];
                  let sample_high = track[ch][high];
                  *out_sample = sample_low + (sample_high - sample_low) * frac as f32;
              } else if low < input_len {
                  *out_sample = track[ch][low];
              }
          }
      }
      ```
    - In `lib.rs` around line 313:
      ```rust
      for i in 0..len {
          for channel_samples in samples.iter() {
              let sample = channel_samples.get(i).copied().unwrap_or(0.0);
              let sample_i16 = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
              writer.write_sample(sample_i16)
                  .map_err(|e| AetherError::IoError(path.to_string_lossy().to_string(), e.to_string()))?;
          }
      }
      ```
    - I will make these changes using the `replace_with_git_merge_diff` tool.
2. *Run tests and linting*
   - Verify that all tests pass using the `run_in_bash_session` tool to execute `cargo test --workspace` and `cargo clippy --all-targets --workspace`.
3. *Complete pre-commit steps*
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
4. *Submit the change.*
    - Submit the PR with the required Bolt format description using the `submit` tool.
