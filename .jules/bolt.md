## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.

## 2026-08-20 - Unswitching inner sample loops and pre-calculating reciprocal ratios
**Learning:** `MultiTrackMixer` was evaluating per-sample channel branch checks (`track_ch == 1`) and per-sample float divisions and `.floor()` / `.ceil()` calls inside inner audio loops.
**Action:** Lift channel branch checks outside the sample loop (`split_at_mut(1)` for disjoint slices enabling SIMD auto-vectorization) and pre-calculate reciprocal ratio `inv_ratio` outside resampling loops.
