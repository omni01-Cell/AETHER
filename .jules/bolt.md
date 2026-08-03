## 2024-05-19 - [Audio Resampler Math]
**Learning:** When attempting to separate a DSP loop into bounds-checked vs unchecked bounds paths (like `(input_len as f64 - 1.0) * ratio`), verify edge case up-sampling panic regressions before pushing.
**Action:** Always double-check mathematically computed safe loop bounds through short script compilation before applying bounds optimization to hot DSP loops.
