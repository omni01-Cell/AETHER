## ⚡ Bolt Optimization Plan
- Optimize `MultiTrackMixer::mix` in `crates/aether-audio/src/dsp.rs`
- Extract loop-invariant calculations
- Apply loop unswitching
- Replace direct slice indexing with iterators
- Add inline comments explaining optimizations
- Ensure all tests pass
