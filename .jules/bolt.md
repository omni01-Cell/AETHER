## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.

## 2025-06-04 - Optimizing Box Blur with O(1) Sliding Window Algorithm
**Learning:** Naively computing box blur sums per pixel iterates `2r + 1` times per pixel per channel, leading to O(W * H * r) complexity and heavy inner-loop floating-point operations.
**Action:** Use an O(1) sliding window accumulator with integer (`u32`) channel sums. Add entering pixels and subtract leaving pixels per step to reduce blur time complexity to O(W * H), independent of blur radius `r`.
