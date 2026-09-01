## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.
## 2025-06-03 - Optimizing CPU image filters with Look-Up Tables (LUTs)
**Learning:** Per-pixel floating point calculations (normalized conversions, scaling, clamping) in image processing inner loops (e.g. brightness and contrast filters) scale linearly with image pixel count (millions of operations per frame).
**Action:** Precompute a 256-entry Look-Up Table (`[u8; 256]`) for 8-bit channel transformations outside the pixel loop to replace float arithmetic with constant-time array index lookups.
