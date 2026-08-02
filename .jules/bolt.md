## 2025-08-02 - [Audio DSP Loop Optimization]
**Learning:** Extracting conditionals outside loops (loop unswitching) and replacing slice indexing with `iter_mut().zip()` after a `split_at_mut()` is critical for Rust DSP loops to elide bounds checks and mutable aliasing panics, resulting in cleaner, performant code without requiring `unsafe`.
**Action:** Always search for indexed stateful loops (e.g., `samples[ch][i]`) in `aether-audio` and `aether-video`, check if variables are invariant and extract them, and utilize `split_at_mut` + iterators to eliminate runtime overhead.
