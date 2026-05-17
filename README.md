# AETHER — Advanced Engine for Theatrical and Electronic Rendering

AETHER is a headless multimedia creation engine written in **Rust**, designed specifically to be driven by **AI agents** via a semantic **Domain Specific Language (DSL)** over a CLI/Daemon model.

## Core Features

- **Architectural Persistence**: Client-Daemon architecture over Unix Domain Sockets (UDS) keeping project session in memory across separate CLI execution rounds.
- **Semantic DSL & Snapshots**: Natural editing-centric commands (`trim`, `mix`, `composite`, etc.) with reference tracking (`@v1`, `@a1`) and minimal context JSON snapshots.
- **Multimodal Engines**: Core processing engines built in Rust:
  - **Video**: FFmpeg (`ffmpeg-next`) filtering + GPU-accelerated compositing (`wgpu`/`tiny-skia`).
  - **Audio**: Low-level decoding (`symphonia`), processing (`rubato`, `hound`), and parametric DSP.
  - **Image**: Multi-format raster/vector composition (`image-rs`, `resvg`, `tiny-skia`).
  - **Animation**: High-fidelity keyframe rigging and Lottie (`lottie-rs`) parser.
- **AI Integration**: Async plugins for state-of-the-art media generation APIs (Veo 3.1, Wan 2.5, Nano Banana, MiniMax Speech).
- **Professional Interoperability**: Built on SQLite metadata databases and OTIO (OpenTimelineIO) compatible timelines.

## Project Structure

This project is a Cargo Workspace composed of the following crates under `crates/`:

- [`aether-core`](crates/aether-core/): Shareable types (Refs, Snapshot, Command structures, errors).
- [`aether-cli`](crates/aether-cli/): Lightweight CLI wrapper and interactive REPL shell.
- [`aether-daemon`](crates/aether-daemon/): Long-running daemon maintaining the project state and executing rendering tasks.
- [`aether-persistence`](crates/aether-persistence/): SQLite database storage and JSON state serializer.
- [`aether-video`](crates/aether-video/): High performance video decoder, cutter, and encoder.
- [`aether-audio`](crates/aether-audio/): High precision audio processing and DSP rack.
- [`aether-image`](crates/aether-image/): Image manipulation, drawing canvas, and vector parser.

## Getting Started

To compile the entire workspace, ensure you have Rust installed along with `libav` (FFmpeg) development headers:

```bash
cargo build --workspace
```

To run all unit tests:

```bash
cargo test --workspace
```

## License

This project is licensed under the MIT License - see the LICENSE file for details.
