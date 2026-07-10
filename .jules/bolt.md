## 2024-03-24 - Avoiding unnecessary string allocations in parsing
**Learning:** Found parsing code doing consecutive `str::replace` calls which each allocate a new String.
**Action:** Replace `s.replace("prefix", "").replace("(", "").replace(")", "")` with `s.strip_prefix("prefix").unwrap_or(s).trim_matches(|c| c == '(' || c == ')')` to do zero-allocation string slicing.
