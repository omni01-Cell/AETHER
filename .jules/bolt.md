## 2024-07-18 - Optimized box blur using sliding window
**Learning:** Found a performance bottleneck in `crates/aether-image/src/cpu.rs` where the `apply_box_blur` function used a nested loop approach with O(n^2) complexity for filter operations.
**Action:** Replace the nested loop with an O(n) sliding window approach. It drastically improved performance (execution time dropped from ~790ms to ~42ms). When making image filtering algorithm changes, prefer utilizing moving window sums instead of recalculating overlapping pixel neighborhoods.
