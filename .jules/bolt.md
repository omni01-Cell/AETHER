## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.

## 2025-06-03 - Optimizing KeyframeTrack interpolation search
**Learning:** `KeyframeTrack::interpolate` was linearly scanning keyframe intervals in $O(N)$ time per lookup. On dense animation tracks or per-frame timeline evaluation, linear search scales poorly with keyframe count.
**Action:** Use `partition_point(|k| k.time_ms <= time_ms) - 1` on sorted keyframe tracks to locate interval bounding indices in $O(\log N)$ time.
