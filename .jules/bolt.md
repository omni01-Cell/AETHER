## 2025-02-17 - [Optimizing Rust loops in stateful DSP loops in audio/signal processing]
**Learning:** Extracting conditionals outside of loops (loop unswitching), pre-calculating invariant mathematical variables, and replacing direct slice indexing with iterators (e.g., `iter_mut().zip()`, paired with `split_at_mut()` for parallel buffers) elides runtime bounds checks and avoids mutable aliasing issues, leading to significant performance improvements.
**Action:** Always pre-calculate invariants and use iterators instead of indices when dealing with audio buffers in Rust.
