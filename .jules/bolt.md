## 2024-05-18 - [Rust DSP Optimization: Loop Unswitching and Zipping Iterators]
**Learning:** In audio DSP processing loops (e.g. `MultiTrackMixer::mix` for `aether-audio`), having conditionals inside the inner loop and using direct slice indexing causes overhead from both branch prediction and bounds checking.
**Action:** Always extract static conditionals out of the tight loops (loop unswitching) and prefer using iterator methods like `.zip()` and `split_at_mut()` for concurrent buffer mutation. This enables vectorization and safely elides bounds checking and mutable aliasing errors.
