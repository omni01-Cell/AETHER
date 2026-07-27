## 2024-05-19 - [Iterators in DSP loops]
**Learning:** Using iterators (`zip`, `iter_mut`) over vector indexing inside tight stateful DSP loops eliminates bounds checking overhead, significantly improving processing performance. This is particularly important for inner loops that process every audio sample.
**Action:** When working on audio DSP components in Rust, prioritize iterator combinations to modify multi-channel buffers simultaneously instead of index-based nested loops.
