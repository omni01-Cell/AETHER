## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.

## 2025-06-03 - Optimizing CPU box blur filter allocations and pixel inner loops
**Learning:** Cloned pixel buffer vectors (`pixels.to_vec()`, `temp.clone()`) and per-pixel floating-point math in image filter loops create heavy GC/heap allocation overhead and high CPU cycle costs.
**Action:** Allocate a single intermediate scratch buffer for 2-pass box blur, write vertical blur pass directly to target mutable pixel slice, and use integer accumulator arithmetic for sample sums.
