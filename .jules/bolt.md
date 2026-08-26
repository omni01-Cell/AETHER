## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.

## 2025-06-04 - Optimizing CpuBackend image box blur with sliding window
**Learning:** The CPU box blur was recomputing pixel sums across the entire window for every pixel and making 3 full vector clones of image buffers, leading to O(W * H * R) time complexity.
**Action:** Replace full window re-computation with a sliding-window accumulator (subtract outgoing pixel, add incoming pixel) reducing complexity to O(W * H), and allocate a single reusable buffer for intermediate passes.
