## 2026-07-19 - Rust Indexed Loops vs Iterators
**Learning:** In audio processing, manually indexing into vectors (like `output[ch][i]`) inside hot inner loops triggers runtime bounds checks that severely degrade performance.
**Action:** Always replace indexed loops with `iter().enumerate()` or `iter_mut().enumerate()` in data-intensive areas like audio/video DSP blocks to let the compiler elide bounds checking.
