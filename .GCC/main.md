# Current task context

## 🏆 Major Milestones (Archived Epics)
- [2025-06-02] Initial project structure with prompt-guides system, bridge architecture, and provider implementations
- [2025-06-02] All generation providers implemented: image (OpenAI, Google), video (Kling, Seedance, Veo), voice (ElevenLabs, Gemini, OpenAI), music (MiniMax)
- [2026-06-06] Phase 1-3 completed: Semantic automation (detect-cuts, strip-silence), layout engine (Taffy v0.10.1), and design tokens (SQLite, CLI, daemon resolution) fully implemented and tested.
- [2026-07-02] Configuration auto-review et auto-merge (Jules & Husky) complétée et vérifiée.

## Objective
Plan et implémentation SOTA des fonctionnalités manquantes d'AETHER identifiées dans `fonctionnalites_manquantes_aether.md` : moteur de layout fluide (taffy), expression engine (evalexpr), automatisation sémantique audio/vidéo (detect-cuts, strip-silence), design tokens, colorimétrie avancée, bus audio, et enrichissement du compositing.

## Decisions made
- [2025-06-02] Architecture: system.md + model JSON files for prompt injection (not external files at runtime)
- [2025-06-02] Bridge pattern: TypeScript bridge handles API calls, Rust handles orchestration
- [2025-06-02] Video providers: Kling 3.0 (Kuaishou) as primary for VideoText/VideoFrame, Seedance 2.0 (ByteDance) for VideoIngredients/VideoEdit
- [2025-06-02] Video API choices: Kling via klingai.com API, Seedance via segmind.com API, Veo via Gemini API
- [2025-06-02] Voice providers: ElevenLabs v3 as primary (Elo 1178), Gemini TTS as fallback, OpenAI TTS as secondary fallback
- [2025-06-02] Music providers: MiniMax Music 2.5 via FAL.AI ($0.035/gen)
- [2026-06-06] Chose Google Lyria (lyria-3-clip-preview) as a music model and google-chat as a chat completion handler for Gemini-based agents.
- [2026-06-06] Fixed bridge.rs input_asset_paths validation to make it conditional on the generation kind.
- [2026-06-06] Modified MockProvider in mock.rs to generate real media files (PNG/WAV/MP3/MP4) using FFmpeg and tiny byte-array fallbacks.
- [2026-06-06] SOTA technology selection for missing features: taffy v0.10.1 (layout), evalexpr v13.1.0 (expressions), FFmpeg scdet (scene cuts), nnnoiseless (denoiser), whisper-rs v0.16.0 (STT)
- [2026-06-06] Implémentation des design tokens et persistance SQLite résolue.
- [2026-06-06] Mis à jour le rapport d'analyse d'écarts fonctionnalites_manquantes_aether.md en y cochant les fonctionnalités finalisées.
- [2026-06-06] Effectué la recherche SOTA /deep-research et rédigé le plan d'implémentation des manquants de Premiere Pro.

## 🌿 Active Branches / Plans
- `bolt-optimize-compressor-resample` : Precompute compressor linear threshold and replace resampler float operations with integer casts ([plan_bolt-optimize-compressor-resample.md](file:///home/omni/Code/AETHER/.GCC/branches/plan_bolt-optimize-compressor-resample.md))
- `bolt-multi-track-mixer-opt` : Optimize MultiTrackMixer array access
- `bolt-optimize-compressor` : Optimize DynamicCompressor decibel math
- `plan_add_git_folders` : Suivi des dossiers .agent et .GCC dans Git ([plan_add_git_folders.md](file:///home/omni/Code/AETHER/.GCC/branches/plan_add_git_folders.md))


## Current status
- ✅ Done: Phase 1 (Automatisation Sémantique: detect-cuts, strip-silence, analyze-color)
- ✅ Done: Phase 2 (Moteur de disposition Taffy v0.10.1)
- ✅ Done: Phase 3 (Design Tokens & Thèmes: SQLite persistence, CLI parsing, daemon resolution)
- ✅ Done: Configuration auto-review & auto-merge
- 🔄 In progress: Phase 4 & Implémentation Premiere Pro — ⏳ En attente de l'approbation de l'utilisateur pour le plan d'implémentation
- ⏳ Pending: Implémentation des phases 5 à 7 (Colorimétrie, Calques d'ajustement, Proxies, Queue)

## Next action
Attendre l'approbation de l'utilisateur sur le plan d'implémentation avant de corriger expressions.rs (Phase 4) et de commencer les manquants de Premiere Pro (Phases 5 à 7).

## Abandoned branches
- None
