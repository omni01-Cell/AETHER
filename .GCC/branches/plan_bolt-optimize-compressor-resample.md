# Execution Plan: Bolt Optimize Compressor and Resample

## 📋 Target Invariant & Pre-requisites
- **Target Invariant**: Audio DSP logic maintains exact audio output equivalence while performing significantly faster by avoiding redundant computations and floating point conversions.
- **Pre-requisites**: Aether audio test suite passes.

## 🛠️ Step-by-Step Sequence

### Step 1: Optimize DynamicCompressor and Resampler
- [ ] **Action**: Modified `crates/aether-audio/src/dsp.rs` to precompute `threshold_linear` in `DynamicCompressor::new()` and replace `.floor()`/`.ceil()` with integer casts in `resample_track()`.
- [ ] **Verify**: `cargo test --workspace`
- **Verification Proof**:
```text
running 6 tests
test tests::test_compressor_gain_reduction ... ok
test tests::test_multitrack_mixing_pan ... ok
test tests::test_sinus_highpass_attenuation ... ok
test tests::test_audio_metadata_extraction ... ok
test tests::test_audio_import_and_trim ... ok
test tests::test_audio_normalization ... ok
```
