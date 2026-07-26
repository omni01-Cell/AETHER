## 2025-01-20 - [Iterators in DSP Loops]
**Learning:** In Rust DSP code, heavy inner loops running per-sample should use zip iterators (`.iter_mut().zip()`) instead of indexing to eliminate bounds checks. This is a common performance pitfall in audio code handling channels/samples.
**Action:** When finding loops like `for ch in 0..channels { let env = &mut self.envelope[ch]; for sample in &mut samples[ch] { ... } }`, refactor to `for (sample_ch, env) in samples.iter_mut().zip(self.envelope.iter_mut()) { for sample in sample_ch { ... } }` and similarly for `BiquadFilter`.
