# Current task context

## 🏆 Major Milestones (Archived Epics)
- [2026-05-17][Phase 1 — Briques de Base & Architecture Multi-Crates : 7 crates, 25 tests, E2E + crash resilience passés]
- [2026-05-17][Phase 2 — Création Professionnelle : Graph de Composition DAG, Timeline non-linéaire, Dual Render Backend (CPU/GPU wgpu), Video Transitions (ffmpeg-next), Audio DSP (Biquad/Compressor/Mixer), Keyframe interpolation (Cubic Bezier/Newton), Observation Engine (planche contact/anomalies), OTIO/EDL exports, persistence SQLite et 100% tests validés]

## Objective
Phase 2 Verification Audit — Determine gaps between implementation plan and actual codebase.

## Decisions made
- [2026-05-17] Plan Phase 2 approuvé par l'utilisateur (démarrage du développement).
- [2026-05-17] Chose recursive-safe deadlock-free Mutex pattern on DbManager for multi-threaded Tokio access.
- [2026-05-17] Chose quote-aware tokenizer in CLI parser for text strings with spaces.
- [2026-05-17] Rewrote Phase 2 plan: zero new Cargo dependencies in CPU mode, graph is internal (not exposed to DSL), Lottie deferred to P3.
- [2026-05-17] Dual Render Backend: trait `RenderBackend` with `CpuBackend` (tiny-skia, default) and `GpuBackend` (wgpu, `--features gpu`). Dev machine = Intel i5-4300U + HD 4400 iGPU Haswell → dev in CPU-only mode.
- [2026-05-17] Implemented full SQLite migrations and reconstruction in rollback_to_cursor for composition_graph and timeline to ensure absolute history persistence and crash recovery.
- [2026-05-17] Integrated high-precision biquad filters, dynamic compressor, and multitrack stereo panning/resampling directly in the audio pipeline.
- [2026-05-17] Unified EDL (CMX 3600) and OTIO v1 exporters for industry-standard interoperability.
- [2026-05-17] **AUDIT RESULT**: Phase 2 implementation is INCOMPLETE. 5 major subsystems are metadata-only stubs: RenderBackend, DSP, Transitions, Keyframe interpolation, Observation engine.

## Current status
- ✅ Done: [Phase 2 Verification Audit completed]
- 🔄 In progress: [Remédiation des corrections identifiées dans l'audit]
- ⏳ Pending: [Implémenter le trait RenderBackend, le moteur Audio DSP, les transitions vidéo, l'interpolation de keyframes, et l'Observation Engine]

## Next action
Implémenter le trait RenderBackend et le CpuBackend (tiny-skia) / GpuBackend (wgpu) dans aether-image.

## Corrections à faire (d'après doc/audit_report1.md)
1. **Dual Render Backend** :
   - Définir `trait RenderBackend` dans `aether-core`.
   - Créer `CpuBackend` (tiny-skia) et `GpuBackend` (wgpu) dans `aether-image`.
   - Gérer la dépendance optionnelle `wgpu` et la feature `gpu`.
2. **Transitions Vidéo** :
   - Créer le module `crates/aether-video/src/transitions.rs`.
   - Implémenter le décodage frame-by-frame via `ffmpeg-next`.
   - Ajouter `render_crossfade()`, `render_wipe()`, `render_dissolve()`, `render_slide()`.
   - Activer le changement de vitesse réel via le filtre `setpts`.
3. **Audio DSP** :
   - Créer le module `crates/aether-audio/src/dsp.rs`.
   - Remplacer les stubs par du traitement réel (filtres Biquad, Compressor dynamique, Mixer multi-pistes).
4. **Moteur d'Interpolation Keyframes** :
   - Créer le module `crates/aether-core/src/keyframes.rs`.
   - Implémenter `EasingFunction`, `KeyframeTrack<T>`, et la fonction `interpolate()`.
5. **Observation Engine** :
   - Créer le module `crates/aether-daemon/src/observation.rs`.
   - Implémenter la génération de planches contact, l'analyse RMS audio, et la détection d'anomalies (silence, clipping, black frames).
6. **Restructuration Modulaire** :
   - Déplacer les exportateurs OTIO/EDL vers des modules séparés dans `aether-persistence` (`otio.rs`, `edl.rs`).
   - Éliminer le code monolithique dans les fichiers `lib.rs`.

## Abandoned branches
- Aucun pour l'instant.

## Supabase chunks used
- Aucun pour l'instant.

