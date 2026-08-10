## 2026-08-10 - Optimizing Stateful DSP Audio Mixing Loops
**Learning:** In Rust audio DSP code, mutable aliasing rules and runtime bounds checks inside tight sample loops can significantly degrade performance. Directly indexing slices in a loop (e.g., `mixed[0][i] += l_sample`) incurs per-sample bounds checks.
**Action:** Always extract invariant variables (loop unswitching), split mutable borrowing manually using `split_at_mut()`, and prefer iterators like `iter_mut().zip()` over direct indexing to elide bounds checks in hot DSP loops.
