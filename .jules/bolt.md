## 2024-05-17 - [Iterator zipping in Rust DSP]
**Learning:** In audio DSP code (e.g. Biquad Filters, Compressors), replacing array indexing bounds inside multi-channel loops with iterator zipping (`iter_mut().zip()`) completely avoids inner loop bounds checking and silences performance lints (like `clippy::needless_range_loop`).
**Action:** When optimizing multi-channel nested processing structures, always restructure state accesses (like `filters`, `envelope`) into synchronized parallel iterators to ensure maximal performance.
