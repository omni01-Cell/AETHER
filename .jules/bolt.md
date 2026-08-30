## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.

## 2025-06-03 - Optimizing CPU box blur sliding window accumulator
**Learning:** Naive box blur re-calculated sliding window pixel sums at every position, resulting in $O(W \times H \times r)$ complexity and redundant intermediate buffer clones.
**Action:** Implement an $O(W \times H)$ sliding window moving average accumulator for both horizontal and vertical passes, reducing blur time by $O(r)$ while eliding buffer allocations.
