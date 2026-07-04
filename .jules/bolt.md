## 2024-07-04 - Keyframe Binary Search Optimization
**Learning:** Found that keyframe interpolation for f32 tracks previously used an O(N) linear scan, which can be easily changed to O(log N) since `insert_keyframe` naturally keeps `keyframes` sorted.
**Action:** Always check if sorted data structures are being linearly searched in hot paths (like audio/video rendering).
