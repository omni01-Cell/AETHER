## 2025-02-18 - Rust DSP Loop Optimizations
**Learning:** In highly stateful audio loops (like `MultiTrackMixer::mix`), direct indexing of parallel mutable slices (e.g., `mixed[0][i]` and `mixed[1][i]`) causes the compiler to conservatively emit bounds checks and struggle with mutable aliasing.
**Action:** Use `split_at_mut` to decouple the mutable regions into distinct slices, and then `.zip()` them with the source iterators. This entirely elides bounds checks in the inner loop and is inherently safe in Rust, resulting in nearly ~2x speedups.
