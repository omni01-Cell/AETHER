# Execution Plan: Fix Unused Import `std::io::Write` in `bridge.rs`

## 📋 Target Invariant & Pre-requisites
- **Target Invariant**: Remove `use std::io::Write;` while ensuring code still compiles.
- **Pre-requisites**: Rust toolchain, aether-generate crate.

## 🛠️ Step-by-Step Sequence

### Step 1: Remove top-level import and update code
- [ ] **Action**: Remove line 2 from `crates/aether-generate/src/bridge.rs` and change `.write_all` to `std::io::Write::write_all(&mut stdin, ...)`.
- [ ] **Verify**: `cargo check -p aether-generate`

### Step 2: Test codebase
- [ ] **Action**: Run `cargo test -p aether-generate`.
- [ ] **Verify**: Review test output.

### Step 3: Finish and submit
- [ ] **Action**: `cargo clippy -p aether-generate` and `submit` with pre-commit instructions.

## ⚠️ Mitigations & Edge Cases
- **Risk**: Other `Write` traits might be used or `.write_all` fails.
- **Mitigation**: Update to fully qualified path if there's any ambiguity.
```text
$ cargo check -p aether-generate
    Checking aether-generate v0.1.0 (/app/crates/aether-generate)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.32s
```
```text
$ cargo test -p aether-generate
running 9 tests
test prompt::tests::test_rule_based_prompt_maker ... ok
test mock::tests::test_mock_provider ... ok
test prompter::agent::tests::test_gemini_guide_loads_without_llm ... ok
test prompter::agent::tests::test_system_prompt_header_and_complement ... ok
test registry::tests::test_model_registry ... ok
test routing_config::tests::test_routing_config_loads ... ok
test registry::tests::test_image_edit_route_has_nano_banana_fallback ... ok
test prompter::system_prompt::tests::system_md_substitutes_model_placeholder ... ok
test runtime::tests::test_generation_runtime_run_to_completion ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
