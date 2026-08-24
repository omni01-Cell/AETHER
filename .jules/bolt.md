## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.
## 2025-06-03 - Optimizing DSP resampler loop
**Learning:** In audio resampling loops, dividing by a ratio and using floating point `.floor()`/`.ceil()` are major bottlenecks.
**Action:** Pre-calculate `1.0 / ratio` for multiplication, use direct integer casts (`as usize`) which act as a fast floor for positive floats, and leverage output iterators to remove runtime bounds checks.
