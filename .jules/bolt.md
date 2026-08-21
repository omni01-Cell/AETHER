## 2025-06-03 - Optimizing DynamicCompressor decibel conversions
**Learning:** The DynamicCompressor was unconditionally performing expensive `log10` and `powf` operations on every single sample, even when the signal was below the compression threshold.
**Action:** Pre-calculate the linear threshold outside the inner processing loop and use a fast linear comparison (`if *env > threshold_linear`) to elide expensive decibel-space conversions.
## 2025-06-03 - Optimizing MultiTrackMixer audio mixing loop
**Learning:** Explicit indexing in inner DSP loops (`mixed[0][sample_idx] += ...`) forces Rust to perform redundant runtime bounds checks, creating a significant performance bottleneck.
**Action:** Use iterators (e.g., `iter_mut().zip()`) and slice splitting (`split_at_mut()`) on parallel buffers to completely elide bounds checks and safely handle mutable aliasing.

## 2025-06-03 - Optimizing Contrast and Brightness filters with Lookup Tables (LUT)
**Learning:** Per-pixel floating-point division, multiplication, clamping, and conversions in 8-bit color space filters force redundant arithmetic across millions of pixels per frame.
**Action:** Precompute a 256-element lookup table (`[u8; 256]`) before pixel iteration for any 8-bit channel transformation, turning expensive floating-point arithmetic into O(1) table indexing.
