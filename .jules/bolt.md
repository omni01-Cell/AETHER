## 2024-05-24 - Rust Iterator Optimization in DSP Loops
**Learning:** Using manual indexing in hot DSP loops (like `for ch in 0..channels { for i in 0..len { ... track[ch][i] ... } }`) forces Rust to perform runtime bounds checking on every sample, which degrades performance.
**Action:** When optimizing Rust loops in this repository, prefer using iterators (e.g., `iter_mut().enumerate()`, `iter_mut().zip()`) over array/vector indexing. This is a highly effective optimization pattern that eliminates runtime bounds checks and improves performance, particularly in stateful DSP loops.
