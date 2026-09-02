## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.

## 2026-09-02 - Precomputed Look-Up Tables (LUTs) for 8-bit image filters
**Learning:** Performing per-pixel float conversions (`u8` -> `f32`), division by 255.0, math operations, and multiplications on millions of pixels per frame creates massive CPU overhead for 8-bit channel image filters.
**Action:** For discrete 8-bit channel domains (`0..=255`), precalculate a 256-entry lookup table `[u8; 256]` outside the pixel loop and map channel values via `lut[channel as usize]`.
