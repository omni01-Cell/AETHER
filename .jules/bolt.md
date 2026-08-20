## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2026-08-20 - Optimizing audio DSP loops
**Learning:** In audio DSP loops, redundant mathematical calculations and array bounds checking cause significant overhead.
**Action:** Extract invariants out of loops and use iterators instead of slice indexing where possible.
