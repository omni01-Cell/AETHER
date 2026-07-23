# Plan for perf/audio-dsp-zip

1. **Optimize `resample_track` in `crates/aether-audio/src/dsp.rs`**
   - Refactored inner loops to use `iter_mut().enumerate()` rather than indexing.
2. **Optimize `mix` in `crates/aether-audio/src/dsp.rs`**
   - Used `split_at_mut` and `iter_mut().zip()` for mono and stereo loops to avoid bounds checks.
3. **Tests & Linting**
   - Verified changes with `cargo test --workspace`.
