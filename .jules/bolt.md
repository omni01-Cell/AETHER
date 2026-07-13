## 2024-05-18 - Avoid Consecutive String Replaces
**Learning:** Consecutive `.replace("old", "new").replace("old2", "new")` calls create unnecessary intermediate string allocations.
**Action:** When targeting multiple single characters or substrings to replace with the *same* replacement string, use an array pattern matcher inside a single call (e.g. `.replace(['(', ')'], "")`).
