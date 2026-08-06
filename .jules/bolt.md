## 2025-02-13 - [Stateful DSP Loops Mutable Aliasing]
**Learning:** Rust's mutable aliasing rules prevent directly mutating separate parallel buffer arrays using standard slice indexing within DSP loops without invoking runtime bounds checks, which causes performance regressions.
**Action:** When optimizing stateful DSP inner loops containing parallel buffers, combine `.split_at_mut()` with multiple `.zip()` iterators over the splits. This structure satisfies the borrow checker while eliding the runtime bounds checks.
