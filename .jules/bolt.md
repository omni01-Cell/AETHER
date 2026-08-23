## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.
## 2025-06-03 - Optimizing audio resampling loop
**Learning:** The `resample_track` function in DSP audio processing contained an inner loop executing division, `.floor()`, and `.ceil()` operations per sample. Floating-point division and rounding functions are significantly slower than multiplication and integer casting.
**Action:** Pre-calculate an inverse ratio and substitute multiplication for division. Replace `.floor()` with a safe `as usize` cast. Refactor the loop to use iterators instead of array indexing to bypass bounds checking.
