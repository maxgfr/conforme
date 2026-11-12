---
name: pre-push
description: Run the checks conforme's CI runs, in its order, before pushing a branch; use before any push or pull request.
---

Run each check from the repository root and stop at the first failure, fix it,
then start again from the top:

1. `cargo fmt -- --check` (if it fails, run `cargo fmt` and review the diff)
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test`
4. `cargo build --release`
5. `./target/release/conforme check`: every tool's config must be in step
   with the source. If it fails, run `./target/release/conforme sync`, look at
   what it wrote, and commit those files with the change that caused them.

Then make sure the commit is signed: `git log --format='%h %G? %s' -1` must
show `G`.

Report each check as passed or failed, with the failing output when one fails.
