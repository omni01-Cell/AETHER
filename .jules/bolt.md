## 2024-05-18 - Audio DSP Loop Optimization
**Learning:** In highly stateful audio and DSP processing loops, array bounds checking overhead within the inner-most loops across parallel slices is a significant bottleneck, and Rust cannot always infer bounds elimination for parallel indexed access on slices in Vecs.
**Action:** Always replace parallel array indexing inside DSP hot paths (like `left[idx]` and `right[idx]`) with iterators using `iter_mut().zip()`, pre-calculating variants outside the loop, and hoisting conditions (like `channel == 1`) to outside the fast-path loop to elide checks.
