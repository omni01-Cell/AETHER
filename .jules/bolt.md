## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.
## 2025-06-03 - Optimizing MultiTrackMixer resample_track
**Learning:** The resample loop was performing expensive floating-point division, `.floor()`, and `.ceil()` for every output sample, and explicit indexing incurred output bounds checks.
**Action:** Pre-calculate the inverse ratio for multiplication, use fast integer casts instead of floor/ceil (handling the exact semantic boundaries), and use `.iter_mut().enumerate()` to elide output bounds checks.
