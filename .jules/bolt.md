## 2024-11-20 - Fast DSP Iterators
**Learning:** In audio DSP loops, indexing mutable multidimensional arrays (e.g. `output[ch][i]`) repeatedly causes runtime bounds checks that kill performance.
**Action:** Use zipped iterators (`iter_mut().zip()`) and safe splitting (`split_at_mut`) to sidestep array bounds checking, particularly in hot mixing loops.
