# Contributing to YAL

Thank you for contributing! Here is how the workspace is structured and what CI enforces.

## Getting Started

1. Read [`docs/BUILD.md`](../docs/BUILD.md) to set up Rust (`1.99.1`, MSRV `1.89`) and Windows Build Tools.
2. Open an issue before starting large features so work isn't duplicated.
3. Keep pull requests focused and include unit tests covering new behavior or bug fixes.

## Quality Gates (enforced by CI)

Before opening a pull request, run:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
cargo build --release --bin yal
.\target\release\yal.exe --self-test
```

### Rules
- **No placeholders in shipped code:** CI fails on `todo!`, `unimplemented!`, `unreachable!`, `TODO`, `FIXME`, `coming soon`, or `mock data` in `crates/` and `apps/`.
- **Pure Rust + Win32 / Direct2D:** Keep UI rendering in `crates/tc-ui` and OS bindings in `crates/tc-platform-win` so core logic crates (`tc-core`, `tc-calc`, `tc-search`, `tc-apps`, `tc-snippets`, `tc-platform`) remain testable on Linux CI.
- **License:** All contributions are licensed under **AGPL-3.0-only** (see [`LICENSE`](../LICENSE)).
- **Code of Conduct:** Please follow [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
- **Security issues:** Report privately per [`SECURITY.md`](SECURITY.md).
