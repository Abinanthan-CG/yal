## What this pull request does

<!-- One paragraph. What changes, and why. A list of changed files is not a
     description. -->

## Type of change

- [ ] A bug fix
- [ ] A new feature
- [ ] A port of an upstream Tinycast feature
- [ ] Documentation only
- [ ] Build or CI

## The parity ledger

- [ ] The rows this change affects are updated in `parity/parts/`.
- [ ] Every row marked `PORTED` names a test that **exists and passes**.
- [ ] Every row marked `NOT PORTED` gives a one-line reason.
- [ ] `cargo run -p xtask -- parity` has been run, and `PARITY.md` and
      `parity/parity.toml` are committed.

## The build passes

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `cargo run -p xtask -- parity --check`
- [ ] `.\scripts\build.ps1` (on Windows)
- [ ] `yal.exe --self-test` (on Windows)

## Notes for the reviewer

<!-- Anything that is not obvious from the diff: a decision you had to make,
     a behaviour that is hard to test, a risk you are taking. If you had to
     choose between two readings of upstream, say which and why. -->
