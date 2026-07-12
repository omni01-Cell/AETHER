## 2024-05-15 - Finding Keyframe Interpolation Index Faster
**Learning:** `KeyframeTrack::interpolate` uses a linear search to find the correct keyframe index `O(n)`. We can use binary search `O(log n)` using `partition_point` or `binary_search_by_key` since keyframes are kept sorted.
**Action:** Replace the linear search loop in `KeyframeTrack::interpolate` with `partition_point` for faster lookup.
