## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.
## 2025-06-03 - Optimizing MultiTrackMixer track resampling
**Learning:** Using `floor()` and `ceil()` along with division in an inner DSP resampling loop introduces unnecessary floating-point overhead, and direct multidimensional slice indexing prevents the compiler from eliding bounds checks.
**Action:** Pre-calculate inverse ratios to use multiplication, replace float rounding functions with direct integer casts (`as usize`), and use `iter_mut()` over the inner slice to elide runtime bounds checks.
