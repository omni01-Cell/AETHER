# Plan: Bolt Compressor Optimization

## Description
Optimizing the `DynamicCompressor::process` inner DSP loop in `crates/aether-audio/src/dsp.rs`.
This optimization pre-calculates mathematical invariants outside the loop, removes direct slice indexing using `iter_mut().zip()`, and elides per-sample decibel space math when below the compression threshold.

## Status
In Progress
