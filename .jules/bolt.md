## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.
## 2026-09-03 - Optimizing Resample track audio loops
**Learning:** In audio resampling algorithms, running expensive math operations like division (i.e. 'src_idx / ratio') and float-rounding ('.floor()', '.ceil()') on every single sample iteration drastically limits throughput.
**Action:** Pre-calculate invariant inverse mathematical variables (e.g. '1.0 / ratio' multiplier instead of division), elide boundary checks using output '.iter_mut()', and replace float rounding with equivalent integer casts (e.g., 'src_idx as usize').
