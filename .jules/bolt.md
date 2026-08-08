## 2024-05-24 - [O(W*H*R) to O(W*H) Box Blur Optimization in aether-image]
**Learning:** Optimizing the `apply_box_blur` function by replacing the nested pixel radius sum loop with a 1D sliding window (moving average) significantly reduced time complexity from O(W*H*R) to O(W*H). Mathematical properties of `.clamp()` on boundaries allowed the sliding window logic to seamlessly mimic the original edge padding behavior without branching.
**Action:** Always check for sliding window optimization opportunities when analyzing nested loops over multidimensional buffers.
