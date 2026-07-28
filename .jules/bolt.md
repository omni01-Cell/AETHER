## 2024-05-18 - [Rust DSP Iterators Optimization]
**Learning:** In stateful DSP processing loops, directly indexing arrays or vectors inside tight loops incurs overhead from bounds checking and potential mutable aliasing issues.
**Action:** When optimizing audio or signal processing hot paths in Rust, extract conditionals outside of loops (loop unswitching), pre-calculate invariant mathematical variables, and replace direct slice indexing with `iter_mut().zip(...)` paired with `split_at_mut()` when operating on parallel buffers to ensurebounds checking is elided at compile time.
