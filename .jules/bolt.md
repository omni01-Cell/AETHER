## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.

## 2025-06-03 - Optimizing CPU Box Blur with Sliding Window Accumulator
**Learning:** Naive box blur implementations iterate over the full kernel radius for every single pixel ($O(W \times H \times R)$), performing repeated boundary clamping, vector cloning, and floating-point divisions in the inner loop.
**Action:** Convert box blur passes into 1D separable sliding-window accumulators ($O(W \times H)$) that update running color sums by adding entering pixels and subtracting leaving pixels, while pre-calculating the inverse window length (`1.0 / window_len`) to replace division with multiplication.
