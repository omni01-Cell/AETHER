## 2024-05-15 - DSP Loop Optimization
**Learning:** Direct slice indexing in Rust's inner loops during DSP tasks causes severe bounds-checking overhead; extracting conditionals (loop unswitching) and using `.iter_mut().zip()` with `split_at_mut()` is necessary to avoid this penalty and mutable aliasing errors.
**Action:** Always pre-calculate invariant mathematical variables and replace direct slice indexing with iterators for stateful DSP loops to elide bounds checks.
