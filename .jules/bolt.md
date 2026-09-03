## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.

## 2025-06-04 - Optimizing KeyframeTrack interpolation
**Learning:** `KeyframeTrack::interpolate` previously performed a linear scan `O(N)` over keyframes for every frame evaluation. Since keyframes are maintained in sorted order by timestamp, binary search (`partition_point`) eliminates the linear overhead.
**Action:** Use `partition_point(|k| k.time_ms <= time_ms)` on sorted keyframe tracks to evaluate keyframe intervals in `O(log N)` time.
