# Graph Report - .  (2026-05-17)

## Corpus Check
- Corpus is ~7,045 words - fits in a single context window. You may not need a graph.

## Summary
- 11 nodes · 19 edges · 2 communities
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Core Orchestration & Core Types|Core Orchestration & Core Types]]
- [[_COMMUNITY_Media Engines & Observation|Media Engines & Observation]]

## God Nodes (most connected - your core abstractions)
1. `AETHER Daemon` - 9 edges
2. `AETHER Shared Core Types` - 9 edges
3. `Video Engine` - 3 edges
4. `Audio Engine` - 3 edges
5. `Multimodal Observation Protocol` - 3 edges
6. `AETHER CLI & REPL` - 2 edges
7. `Image Engine` - 2 edges
8. `Animation Engine` - 2 edges
9. `Persistence & SQLite` - 2 edges
10. `AI Generation Layer` - 2 edges

## Surprising Connections (you probably didn't know these)
- `Multimodal Observation Protocol` --references--> `AETHER Shared Core Types`  [EXTRACTED]
  Document complémentaire AETHER sur les problèmes à résoudre.md → aether_headless_media_engine.md
- `Multimodal Observation Protocol` --references--> `Video Engine`  [EXTRACTED]
  Document complémentaire AETHER sur les problèmes à résoudre.md → aether_headless_media_engine.md
- `Multimodal Observation Protocol` --references--> `Audio Engine`  [EXTRACTED]
  Document complémentaire AETHER sur les problèmes à résoudre.md → aether_headless_media_engine.md

## Hyperedges (group relationships)
- **AETHER Media Engines** — aether_video, aether_audio, aether_image, aether_animation [EXTRACTED 1.00]
- **AETHER Interfaces** — aether_cli, aether_mcp [EXTRACTED 1.00]

## Communities (2 total, 0 thin omitted)

### Community 0 - "Core Orchestration & Core Types"
Cohesion: 0.43
Nodes (8): AI Generation Layer, Animation Engine, AETHER CLI & REPL, AETHER Shared Core Types, AETHER Daemon, Image Engine, MCP Server Integration, Persistence & SQLite

### Community 1 - "Media Engines & Observation"
Cohesion: 0.67
Nodes (3): Audio Engine, Multimodal Observation Protocol, Video Engine

## Knowledge Gaps
- **1 isolated node(s):** `MCP Server Integration`
  These have ≤1 connection - possible missing edges or undocumented components.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `AETHER Daemon` connect `Core Orchestration & Core Types` to `Media Engines & Observation`?**
  _High betweenness centrality (0.430) - this node is a cross-community bridge._
- **Why does `AETHER Shared Core Types` connect `Core Orchestration & Core Types` to `Media Engines & Observation`?**
  _High betweenness centrality (0.356) - this node is a cross-community bridge._
- **Why does `Video Engine` connect `Media Engines & Observation` to `Core Orchestration & Core Types`?**
  _High betweenness centrality (0.015) - this node is a cross-community bridge._
- **What connects `MCP Server Integration` to the rest of the system?**
  _1 weakly-connected nodes found - possible documentation gaps or missing edges._