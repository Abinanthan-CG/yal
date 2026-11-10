# Contributing

Thank you for helping. This document is the short version of how the project
is put together, so that your change fits in.

If you are using an AI coding assistant, read
[`BUILD_WITH_AI.md`](BUILD_WITH_AI.md) as well — it has the same rules in a
form written for a tool that does not remember yesterday.

---

## Before you start

- **Search the parity ledger first.** `PARITY.md` lists every feature and
  sub-feature, with its status. If the row you want to work on says
  `PORTED`, the feature exists; if it says `NOT PORTED`, the reason is in the
  row.
- **Open an issue** for anything larger than a bug fix, so that two people do
  not write the same feature twice.
- **Talk to the upstream author** if you need a behaviour clarified. The
  upstream sources are in `upstream/`, and the original project is at
  <https://github.com/abue-ammar/tinycast>.

---

## The workflow

1. **Write the failing test.** The test is the evidence that the feature
   works, and the evidence is what lets you mark a parity row `PORTED`.
2. **Run it.** Watch it fail. A test that passes before the change proves
   nothing.
3. **Write the smallest implementation that makes it pass.**
4. **Run it again.** Watch it pass.
5. **Update the parity row** with the status and the evidence.
6. **Regenerate the ledger.**
7. **Run the whole build.**
8. **Commit.**

```powershell
cargo run -p xtask -- parity
.\scripts\build.ps1
```

---

## The rules

### No placeholders in shipped code

No `todo!()`, `unimplemented!()`, `unreachable!()`, no `TODO`, no `FIXME`, no
"coming soon", no mock data. CI greps for these and fails.

A feature that is not finished stays `NOT PORTED` in the ledger, with the
reason. That is a perfectly good state for a pull request.

### Evidence is a test that ran

A parity row is `PORTED` only when a test actually ran and actually passed.
The evidence is named `crate::test_name`, and the validator checks that the
crate exists and that it contains a function with that name.

Writing a test and not running it is not evidence.

### The ledger is generated

`PARITY.md` and `parity/parity.toml` are generated. Edit the part files and
regenerate:

```powershell
cargo run -p xtask -- parity
```

CI runs `cargo run -p xtask -- parity --check`, which fails if the committed
output does not match the parts.

### Win32 stays behind the platform traits

`tc-platform` defines the traits. `tc-platform-win` implements them. **No
other crate in the workspace may call Win32 directly.**

This is what lets CI test the pure crates on Linux in seconds, and it is what
keeps the domain testable without a desktop.

### The helper processes stay separate

The file indexer, the OCR helper and the dictation helper are separate
processes, so that a large scan, a large document or a large model cannot
block the palette, and a crash in one cannot take the app down.

### Record the decisions

When two sources disagree, or when you have to choose between two readings of
upstream, write it in `docs/DECISIONS.md` with the date and the reasoning.

---

## Commit messages

Conventional commits, small and specific:

```text
feat(tc-calc): port the unit conversion rules
fix(tc-platform-win): close the clipboard handle when the read fails
docs: record the decision about the Recycle Bin
chore(deps): update the windows crate to 0.62
test(tc-search): cover the ranking tie-break
```

Keep each commit to one thing. A commit that changes three unrelated things
cannot be reviewed, and cannot be reverted.

---

## Pull requests

A pull request should have:

- **One purpose.** If it does two things, it is two pull requests.
- **A test for every behaviour it adds or changes.**
- **The parity rows updated**, with evidence.
- **A green build.** `.\scripts\build.ps1` must pass.
- **A description that says what it does and why**, in a paragraph. A list of
  changed files is not a description.

If you are part-way through a feature, say so in the description and leave
the remaining rows `NOT PORTED`. A partial, honest pull request is easier to
merge than a complete, unproven one.

---

## Code style

The project follows the Rust community style, enforced:

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

Both must pass. Clippy's warnings are errors here, including the pedantic
ones the tool suggests.

Comments explain **why**, not **what**. A comment that restates the code is
noise; a comment that explains a non-obvious decision is the most valuable
thing in the file.

Doc comments on public items are expected, and they say what the item is for
rather than restating its signature.

---

## Tests

- Unit tests live next to the code, in the same module, in a `#[cfg(test)]`
  block.
- Test names are full sentences: `the_clipboard_round_trips_text`.
- A test that depends on the machine is written so that it still says
  something on a machine where the dependency is missing.
- The pure crates are tested on Linux in CI. The Windows crates are tested on
  `windows-latest`, and on a real Windows machine if you have one.

---

## Reporting bugs

Open an issue with:

- what you did,
- what you expected,
- what happened,
- the version of YAL (`yal.exe --version`) and of Windows.

If the behaviour is meant to match Tinycast, say which upstream feature you
think it belongs to.

---

## Code of conduct

Read the [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) before taking part.

---

## Licence

By contributing, you agree that your contribution is licensed under the
AGPL-3.0, the same licence as the rest of the project. See
[`LICENSE`](LICENSE).

---

## See also

- [`BUILD.md`](BUILD.md) — how to build
- [`BUILD_WITH_AI.md`](BUILD_WITH_AI.md) — the rules, for an AI assistant
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — how the code is arranged
- [`docs/DECISIONS.md`](docs/DECISIONS.md) — the decisions so far
- [`docs/WINDOWS-MAPPING.md`](docs/WINDOWS-MAPPING.md) — the translation table
