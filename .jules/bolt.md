## 2024-05-09 - [Avoid test optimizations]
**Learning:** I learned that applying optimizations to tests provides absolutely zero measurable impact in an application context. The focus needs to be on actual application code processing data.
**Action:** Always filter out `#[cfg(test)]` files or test directories from the list of files to optimize to ensure changes are made in the actual application code path.
