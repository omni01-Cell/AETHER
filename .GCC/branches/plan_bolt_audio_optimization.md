# Bolt Audio Optimization Plan

## Objective
Optimize the `MultiTrackMixer::mix` function in `crates/aether-audio/src/dsp.rs` by implementing loop unswitching and bounds check elision.

## Steps
1. Create this plan and register branch in `.GCC/main.md`.
2. Extract the `track_ch == 1` condition outside the loop in `MultiTrackMixer::mix`.
3. Pre-calculate invariant volumes/pans.
4. Replace direct slice indexing with `split_at_mut()` and `iter_mut().zip()`.
5. Run workspace tests and linters.
6. Record learnings in `.jules/bolt.md`.
7. Cleanup temporary files and create PR.
