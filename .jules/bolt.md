## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.

## 2025-06-03 - Optimizing MultiTrackMixer resampling math
**Learning:** The resample_track function was performing expensive division (`/`) and floating-point `.floor()`/`.ceil()` calls for every single sample in the inner DSP loop.
**Action:** Pre-calculate the inverse ratio outside the loop to substitute division for multiplication, and replace floating-point rounding with safe integer casts (`as usize`), carefully matching exact integer boundary behavior.
