## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.
## 2026-09-02 - Optimizing audio resampling inner loop
**Learning:** Inner DSP loops (like in `MultiTrackMixer::resample_track`) performing divisions and expensive floating point operations like `floor()` and `ceil()` on every sample iteration create major bottlenecks.
**Action:** Pre-calculate invariant mathematical variables (like substituting division for multiplication by an inverse ratio), replace expensive floating-point operations (`.floor()`, `.ceil()`) with safe integer casts.
