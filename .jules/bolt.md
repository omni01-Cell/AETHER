## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.

## 2025-06-03 - Optimizing CPU box blur pixel allocations
**Learning:** 2D image processing passes like box blurs can inadvertently create multiple intermediate heap vector allocations (`.to_vec()`, `.clone()`) when converting between immutable and mutable pixel slice views.
**Action:** Pre-allocate a single intermediate pixel buffer for separable filtering passes, reading directly from `pixmap.pixels()` into `temp` during the first pass and writing into `pixmap.pixels_mut()` in the final pass.
