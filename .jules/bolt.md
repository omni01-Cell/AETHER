## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.

## 2025-06-03 - Optimizing Box Blur filter pass
**Learning:** Naive box blur implementations re-sample the full radius kernel at every pixel leading to $O(W \cdot H \cdot R)$ complexity, alongside expensive float channel casts and redundant vector allocations per pass.
**Action:** Use an $O(W \cdot H)$ 1D sliding window with integer accumulators (`u32`) for running channel sums, and allocate a single temporary buffer to store intermediate pass results.
