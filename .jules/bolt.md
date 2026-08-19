## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.

## 2025-06-03 - Hoisting track gains and branches in MultiTrackMixer
**Learning:** In audio multi-track mixing, evaluating mono/stereo channel conditions (`if track_ch == 1`) and multiplying track volume/pan gains per sample inside inner loops prevents LLVM auto-vectorization and adds per-sample bounds checking overhead.
**Action:** Pre-compute track gains (`vol * left_pan`, `vol * right_pan`) and split output buffers into mutable slices (`split_at_mut`) outside the inner loop to create tight, branchless loops that auto-vectorize with SIMD.
