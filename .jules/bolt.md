## 2024-06-25 - [Binary search for Keyframe tracks]
**Learning:** For time-series data like keyframes that are always kept sorted, a simple loop linear scan (`O(n)`) can become a bottleneck when evaluating complex animations at 60fps.
**Action:** Use Rust's built-in `partition_point` method for sorted collections to efficiently locate indexes with `O(log n)` binary search. E.g., `let idx = self.keyframes.partition_point(|k| k.time_ms <= time_ms) - 1;`
