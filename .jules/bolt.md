## 2026-08-15 - Audio Mixer Loop Optimization
**Learning:** In audio processing loops, conditionals and direct slice indexing inside hot loops cause branch prediction failures and redundant bounds checking.
**Action:** Extract loop-invariant conditionals (loop unswitching), pre-calculate variables outside the loop, and use `.iter_mut().zip()` combined with `.split_at_mut()` to safely elide runtime bounds checks for parallel buffer access.
