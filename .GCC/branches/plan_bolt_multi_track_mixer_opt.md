# Plan: Optimize MultiTrackMixer loop
- Replace explicit index-based sample loop in `MultiTrackMixer::mix` with iterators (`iter_mut().zip()`) and slice splitting (`split_at_mut()`) to elide bounds checks.
