## 2026-07-17 - Performance Optimization: Collapsing `replace()` calls
**Learning:** Consecutive `.replace('a', "").replace('b', "")` calls on strings cause unnecessary intermediate allocations in Rust.
**Action:** Use `.replace(['a', 'b'], "")` instead for better performance, as recommended by `clippy::collapsible_str_replace` (triggered under `clippy::perf`).
