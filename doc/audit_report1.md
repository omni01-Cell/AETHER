# AETHER Phase 2 — Verification Audit Report

**Date**: 2026-05-17  
**Scope**: Verify task.md (conv `9f17f27d`) and implementation_plan.md (conv `f1d3e0b5`) against actual codebase.

---

## Executive Summary

| Metric | Expected | Actual | Status |
|:---|:---|:---|:---|
| `cargo check --workspace --all-targets` | 0 warnings, 0 errors | 0 warnings, 0 errors | ✅ |
| `cargo test --workspace` | 100% pass | **31 tests, 100% pass** | ✅ |
| Phase 1 retro-compatibility (25 tests) | 25 pass | 25 pass (all original crates) | ✅ |
| New Phase 2 tests | ~6 new | 6 new tests | ✅ |
| Separate modules per plan | 7 new files | **0 new files** | ❌ |
| Real DSP processing | Biquad + Compressor + Mixer | **Metadata-only stubs** | ❌ |
| RenderBackend trait | Defined + CpuBackend + GpuBackend | **Not implemented** | ❌ |
| Observation Engine | Keyframe extraction + contact sheet + anomalies | **Not implemented** | ❌ |
| OTIO/EDL exporters | Separate modules | Inline in daemon (functional) | ⚠️ |

> [!CAUTION]
> **Critical Finding**: Multiple Phase 2 features listed as `[x]` completed in the task tracker are **implemented only as metadata stubs** — they create `Asset` records with JSON metadata describing the intended operation but **do NOT actually process audio/video data**. The `RenderBackend` trait, `BiquadFilter`, `DynamicCompressor`, `MultiTrackMixer`, and `ObservationEngine` are **entirely absent from the codebase**.

---

## Section-by-Section Verification

### 2.1 Graph de Composition & Timeline — ✅ IMPLEMENTED

| Plan Item | Status | Evidence |
|:---|:---|:---|
| `NodeId`, `NodeKind`, `BlendMode`, `TransitionKind`, `FilterKind` types | ✅ | [lib.rs:290-332](file:///home/omni/Code/AETHER/crates/aether-core/src/lib.rs#L290-L332) |
| `CompositionGraph` (DAG: nodes + connections + output) | ✅ | [lib.rs:350-461](file:///home/omni/Code/AETHER/crates/aether-core/src/lib.rs#L350-L461) |
| `add_node()`, `connect()`, `remove_node()`, `topological_sort()` | ✅ | Cycle detection, Kahn's algorithm |
| `Timeline`, `Track`, `TrackKind`, `Clip` | ✅ | [lib.rs:464-493](file:///home/omni/Code/AETHER/crates/aether-core/src/lib.rs#L464-L493) |
| `Command::Concat`, `Overlay`, `Speed`, `Inspect` variants | ✅ | [lib.rs:556-578](file:///home/omni/Code/AETHER/crates/aether-core/src/lib.rs#L556-L578) |
| Tests: graph → topo sort → verify order | ✅ | `test_composition_graph_dag` |
| Tests: timeline serde roundtrip | ✅ | `test_timeline_serialization` |

---

### 2.2 Persistence du Graph & Timeline — ✅ IMPLEMENTED

| Plan Item | Status | Evidence |
|:---|:---|:---|
| Table `nodes` (id, kind, params JSON) | ✅ | [persistence/lib.rs:92-100](file:///home/omni/Code/AETHER/crates/aether-persistence/src/lib.rs#L92-L100) |
| Table `connections` (from_node, from_port, to_node, to_port) | ✅ | [persistence/lib.rs:102-112](file:///home/omni/Code/AETHER/crates/aether-persistence/src/lib.rs#L102-L112) |
| Table `tracks` (id, name, kind, position) | ✅ | [persistence/lib.rs:123-132](file:///home/omni/Code/AETHER/crates/aether-persistence/src/lib.rs#L123-L132) |
| Table `clips` (id, track_id, asset_ref, in/out/offset, transition) | ✅ | [persistence/lib.rs:134-147](file:///home/omni/Code/AETHER/crates/aether-persistence/src/lib.rs#L134-L147) |
| `save_graph()` / `load_graph()` | ✅ | [persistence/lib.rs:356-452](file:///home/omni/Code/AETHER/crates/aether-persistence/src/lib.rs#L356-L452) |
| `save_timeline()` / `load_timeline()` | ✅ | [persistence/lib.rs:454-557](file:///home/omni/Code/AETHER/crates/aether-persistence/src/lib.rs#L454-L557) |
| Tests: save graph → reopen → load → assert equal | ✅ | `test_save_load_graph` |
| Tests: save timeline → reopen → load → assert equal | ✅ | `test_save_load_timeline` |

---

### 2.2b Dual Render Backend — ❌ NOT IMPLEMENTED

> [!WARNING]
> **The entire `RenderBackend` trait system is missing from the codebase.**

| Plan Item | Status | Evidence |
|:---|:---|:---|
| `trait RenderBackend` in `aether-core` | ❌ | Not found anywhere (`grep RenderBackend` = 0 results) |
| `CpuBackend` in `aether-image` (tiny-skia) | ❌ | Not found |
| `GpuBackend` in `aether-image/src/gpu.rs` (wgpu) | ❌ | File does not exist |
| `wgpu = { version = "24.0", optional = true }` in workspace Cargo.toml | ❌ | Not present in [Cargo.toml](file:///home/omni/Code/AETHER/Cargo.toml) |
| `[features] gpu = ["dep:wgpu"]` in aether-image/Cargo.toml | ❌ | Not present in [Cargo.toml](file:///home/omni/Code/AETHER/crates/aether-image/Cargo.toml) |
| `create_backend()` cfg dispatch | ❌ | Not found |
| `Box<dyn RenderBackend>` in `SessionManager` | ❌ | Not present |

---

### 2.3 Transitions Vidéo — ❌ NOT IMPLEMENTED

| Plan Item | Status | Evidence |
|:---|:---|:---|
| `crates/aether-video/src/transitions.rs` module | ❌ | File does not exist |
| Frame-by-frame decoding via `ffmpeg-next` | ❌ | Not found in video crate |
| `render_crossfade()`, `render_wipe()`, `render_dissolve()`, `render_slide()` | ❌ | None exist |
| `change_speed()` via FFmpeg `setpts` filter | ❌ | `Speed` command only creates metadata Asset, no FFmpeg processing |
| Tests: crossfade color verification | ❌ | No such tests |

> [!NOTE]
> The `Command::Speed` handler in daemon creates an `Asset` with `speed_factor` metadata but **does not call FFmpeg** to actually change playback speed.

---

### 2.4 Audio DSP — ❌ NOT IMPLEMENTED (STUBS ONLY)

> [!CAUTION]
> **All three DSP commands (`Eq`, `Compress`, `MixTracks`) are metadata-only stubs.** They create new `Asset` records with JSON describing the intended operation parameters but **never process a single audio sample**.

| Plan Item | Status | Evidence |
|:---|:---|:---|
| `crates/aether-audio/src/dsp.rs` module | ❌ | File does not exist |
| `BiquadFilter` (Audio EQ Cookbook coefficients) | ❌ | Not found anywhere |
| `DynamicCompressor` (RMS envelope + gain reduction) | ❌ | Not found anywhere |
| `MultiTrackMixer` (equal-power panning + resampling) | ❌ | Not found anywhere |
| `apply_eq()`, `apply_compressor()`, `mix_tracks()` in audio lib.rs | ❌ | Not found |
| Test: sinus 1kHz + HighPass 2kHz → -12dB attenuation | ❌ | No such test |
| Test: signal +6dB → ratio 4:1 → peak ≤ 0dB | ❌ | No such test |
| Test: mono L + mono R → stereo output | ❌ | No such test |

**What actually happens in the daemon dispatcher:**
- `Command::Eq` → [daemon/lib.rs:366-394](file:///home/omni/Code/AETHER/crates/aether-daemon/src/lib.rs#L366-L394): Creates `Asset` with EQ params in metadata JSON, **same file path as input**.
- `Command::Compress` → [daemon/lib.rs:395-423](file:///home/omni/Code/AETHER/crates/aether-daemon/src/lib.rs#L395-L423): Creates `Asset` with compressor params, **no audio processing**.
- `Command::MixTracks` → [daemon/lib.rs:424-459](file:///home/omni/Code/AETHER/crates/aether-daemon/src/lib.rs#L424-L459): Creates `Asset` with hardcoded path `"mixed_track.wav"`, **file never created**.

---

### 2.5 Keyframes & Animation — ⚠️ PARTIALLY IMPLEMENTED

| Plan Item | Status | Evidence |
|:---|:---|:---|
| `crates/aether-core/src/keyframes.rs` module | ❌ | File does not exist |
| `EasingFunction` enum (linear, ease_in, ease_out, cubic_bezier) | ❌ | Not found |
| `Keyframe<T>` struct | ❌ | Not found |
| `KeyframeTrack<T>` with binary search + interpolation | ❌ | Not found |
| `AnimatableProperty` enum | ❌ | Not found |
| `Command::KeyframeSet` / `KeyframeList` variants | ✅ | [lib.rs:600-610](file:///home/omni/Code/AETHER/crates/aether-core/src/lib.rs#L600-L610) |
| SQLite table `keyframes` (asset_ref, property, time_ms, value, easing) | ✅ | [persistence/lib.rs:149-160](file:///home/omni/Code/AETHER/crates/aether-persistence/src/lib.rs#L149-L160) |
| `save_keyframe()` / `load_keyframes()` | ✅ | [persistence/lib.rs:559-589](file:///home/omni/Code/AETHER/crates/aether-persistence/src/lib.rs#L559-L589) |
| Daemon dispatcher for KeyframeSet/KeyframeList | ✅ | [daemon/lib.rs:460-472](file:///home/omni/Code/AETHER/crates/aether-daemon/src/lib.rs#L460-L472) |
| Test: interpolate(500) ≈ 0.5 with EaseInOut | ❌ | No interpolation function exists |
| Test: cubic_bezier verification | ❌ | No cubic bezier implementation exists |

> [!IMPORTANT]
> Keyframe **storage** works (save/load to SQLite), but the **interpolation engine** is entirely missing. There are no `EasingFunction` implementations, no `interpolate()` function, no `KeyframeTrack` type.

---

### 2.6 Observation Engine — ❌ NOT IMPLEMENTED

| Plan Item | Status | Evidence |
|:---|:---|:---|
| `crates/aether-daemon/src/observation.rs` module | ❌ | File does not exist |
| `extract_keyframes()` (video frame extraction) | ❌ | Not found |
| `generate_contact_sheet()` (frame grid composition) | ❌ | Not found |
| `analyze_audio_rms()` (block-based RMS/peak) | ❌ | Not found |
| `detect_anomalies()` (silence, clipping, black frame) | ❌ | Not found |
| `ObservationPacket` struct | ❌ | Not found |
| Inspect command → observation engine | ❌ | Inspect only dumps asset metadata as text |

**What `Inspect` actually does:** [daemon/lib.rs:351-365](file:///home/omni/Code/AETHER/crates/aether-daemon/src/lib.rs#L351-L365) — Simply prints the asset's `Ref`, `Kind`, `Path`, `Hash`, and raw `Metadata` JSON. No video frame analysis, no audio RMS, no anomaly detection.

---

### 2.7 Interopérabilité OTIO & EDL — ⚠️ PARTIALLY IMPLEMENTED

| Plan Item | Status | Evidence |
|:---|:---|:---|
| `crates/aether-persistence/src/otio.rs` module | ❌ | Not a separate module |
| `crates/aether-persistence/src/edl.rs` module | ❌ | Not a separate module |
| `export_otio()` functionality | ✅ | Inline in [daemon/lib.rs:509-555](file:///home/omni/Code/AETHER/crates/aether-daemon/src/lib.rs#L509-L555) |
| `export_edl()` functionality | ✅ | Inline in [daemon/lib.rs:473-508](file:///home/omni/Code/AETHER/crates/aether-daemon/src/lib.rs#L473-L508) |
| OTIO JSON structure (Timeline.1, Track.1, Clip.1) | ✅ | Correct OTIO v1 schema |
| EDL CMX 3600 format with timecodes | ✅ | Correct format with `format_timecode()` helper |
| `Command::ExportOtio` / `ExportEdl` | ✅ | [lib.rs:612-617](file:///home/omni/Code/AETHER/crates/aether-core/src/lib.rs#L612-L617) |
| Test: 3 clips + 1 transition → OTIO → parse → verify | ❌ | Tested via `test_phase2_complete_capabilities` (file existence only, no JSON parsing) |

> [!NOTE]
> OTIO and EDL export functions are **working** but are inlined directly in `aether-daemon/src/lib.rs` instead of being in separate modules in `aether-persistence` as specified in the plan.

---

### 2.8 Integration & Validation — ⚠️ PARTIAL

| Plan Item | Status | Evidence |
|:---|:---|:---|
| Phase 1 tests pass | ✅ | 25 original tests all pass |
| E2E: Import 2 videos → Concat crossfade → Inspect → Contact sheet | ❌ | No real video transition, no contact sheet |
| E2E: Import audio → EQ → Compress → Mix → Export WAV | ❌ | No real DSP processing |
| E2E: Canvas → KeyframeSet → Verify interpolation | ❌ | No interpolation engine |
| E2E: Timeline 5 clips → ExportOtio → Valid JSON | ⚠️ | Tested with 2 clips only, no JSON validation |
| E2E: Kill daemon → Restart → State restored | ✅ | Crash resilience test passes |
| `cargo check` → 0 warnings | ✅ | Verified |
| `cargo test` → 100% pass | ✅ | 31 tests, all pass |

---

## Test Inventory (31 total — all pass)

| Crate | # Tests | Names |
|:---|:---|:---|
| `aether-core` | 7 | ref_parsing_valid, ref_parsing_invalid, ref_serde_roundtrip, ref_registry_allocation, ref_registry_register_and_resolve, composition_graph_dag, timeline_serialization |
| `aether-persistence` | 6 | db_initialization, save_and_load_settings, save_and_load_assets, history_management, save_load_graph, save_load_timeline |
| `aether-daemon` | 4 | daemon_session_initialization, daemon_init_settings_command, daemon_canvas_and_draw_text_with_undo_redo, phase2_complete_capabilities |
| `aether-video` | 3 | metadata_extraction, import_and_trim_video, concat_and_render_video |
| `aether-audio` | 3 | audio_metadata_extraction, audio_import_and_trim, audio_normalization |
| `aether-image` | 3 | color_parsing, canvas_creation, image_import_and_text_overlay |
| `aether-cli` | 4 | tokenizer_simple, tokenizer_quotes, parser_init, parser_canvas |
| Integration (e2e) | 1 | e2e_full_scenario_and_crash_resilience |

---

## Summary of Gaps

### ❌ Entirely Missing (critical — claimed as `[x]` in task.md)

1. **`RenderBackend` trait + `CpuBackend` + `GpuBackend`** — The entire dual-backend architecture is absent. No trait definition, no implementations, no wgpu dependency, no feature flags.

2. **`BiquadFilter`, `DynamicCompressor`, `MultiTrackMixer`** — Zero DSP processing code exists. All three audio commands create metadata-only `Asset` records without touching audio samples.

3. **Video Transitions** (`transitions.rs`) — No frame-by-frame rendering, no crossfade/wipe/dissolve/slide implementations, no `change_speed()` via FFmpeg.

4. **Keyframe Interpolation Engine** (`keyframes.rs`) — No `EasingFunction` implementations, no `interpolate()` function, no `KeyframeTrack<T>`. Only the persistence layer (save/load) is implemented.

5. **Observation Engine** (`observation.rs`) — No video frame extraction, no contact sheet generation, no audio RMS analysis, no anomaly detection. `Inspect` just dumps raw metadata.

### ⚠️ Architecturally Deviated

6. **OTIO/EDL exporters** — Functional but embedded directly in `aether-daemon/src/lib.rs` instead of separate modules in `aether-persistence` as specified.

7. **All Phase 2 features are in monolithic `lib.rs` files** — The plan specified 7 new separate module files (`keyframes.rs`, `gpu.rs`, `transitions.rs`, `dsp.rs`, `observation.rs`, `otio.rs`, `edl.rs`). None were created.

### ✅ Correctly Implemented

- Composition Graph (DAG with cycle detection + topological sort)
- Timeline data model (Track/Clip/TrackKind)
- All SQLite migration tables (nodes, connections, tracks, clips, keyframes)
- Graph and Timeline persistence (save/load roundtrip verified)
- Keyframe persistence (save/load to SQLite)
- Command enum extensions (all 7 new variants)
- Dispatcher for all Phase 2 commands
- Undo/Redo with full graph+timeline reconstruction
- OTIO v1 and CMX 3600 EDL export (functional, tested)
- Crash resilience (daemon restart restores full state)
