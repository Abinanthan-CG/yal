# Building YAL with an AI coding assistant

This document is written for an AI coding assistant — or for a person who is
using one — working on YAL.

YAL is a Windows port of [Tinycast](https://github.com/abue-ammar/tinycast).
The port is judged by a ledger, not by taste. If you follow the rules below,
your work fits the project; if you break them, the build fails or the ledger
lies, and both are worse than a missing feature.

Read this whole document before you change anything.

---

## 1. The five rules

These are not suggestions. CI enforces the first three, and the parity
validator enforces the last two.

### 1.1 No placeholders in shipped code

No `todo!()`, no `unimplemented!()`, no `unreachable!()`, no `TODO`, no
`FIXME`, no "coming soon", no mock data. Not in `crates/`, not in `apps/`,
not in `frontend/`.

CI greps for these and fails the build. If a behaviour is not implemented yet,
leave the row `NOT PORTED` in the ledger and say so in your report. An honest
gap is fine; a fake implementation is not.

### 1.2 Never claim a feature is ported without evidence

A parity row is `PORTED` only when a test actually ran and actually passed.

The evidence field names a test as `crate::test_name`, for example
`tc-core::the_built_in_command_list_is_complete`. The validator loads that
crate's sources and checks that a function with that name exists. If it does
not, the row is rejected.

A test that exists but has never been run is not evidence. Run it.

### 1.3 The ledger is generated, not edited

`PARITY.md` and `parity/parity.toml` are generated. Never edit them by hand.

```powershell
cargo run -p xtask -- parity          # regenerate
cargo run -p xtask -- parity --check  # what CI runs
```

The rows themselves come from `parity/parts/generated/*.toml`, which in turn
comes from the upstream documentation via `cargo run -p xtask -- ledger`. When
you change a row's status or its evidence, change it in the part file and
regenerate.

### 1.4 Write the test before the implementation

Every behaviour gets a test that fails before the code exists and passes after.
This is not ceremony: the test *is* the evidence, and the evidence is what
makes a row `PORTED`.

### 1.5 Log the decisions

When two sources disagree, or when you have to choose between two readings of
upstream, write it in `docs/DECISIONS.md` with the date and the reasoning.
When you hit something that blocked you, write it in `docs/port-notes/`.

A decision you did not write down is a decision you will make again next week,
differently.

---

## 2. The shape of the repository

```text
crates/tc-core/         pure domain: commands, rows, deeplinks, ids
crates/tc-search/       matching and ranking
crates/tc-calc/         the calculator engine
crates/tc-snippets/     the snippet engine
crates/tc-clipboard/    clipboard history
crates/tc-files/        file search
crates/tc-windows/      window management
crates/tc-calendar/     calendars and events
crates/tc-backup/       backup and restore
crates/tc-i18n/         the nine-language string catalogue
crates/tc-platform/     the traits, and in-memory fakes for tests
crates/tc-platform-win/ the Win32 implementation of those traits
crates/tc-ai/           AI providers
crates/tc-mcp/          the Model Context Protocol
crates/tc-extensions/   the extension system
apps/yal/               the Tauri shell
apps/yal-indexer/       the file indexer helper
apps/yal-ocr/           the text recognition helper
apps/yal-dictation/     the dictation helper
xtask/                  the build tool
```

### The dependency rule

`tc-platform` defines traits. `tc-platform-win` implements them for Windows.
**Nothing else in the workspace may call Win32 directly.**

That rule is what makes the domain testable without a desktop, and it is what
lets CI run the pure crates on Linux in seconds.

If you need a new platform capability:

1. Add the trait to `tc-platform`, with a fake in `tc-platform/src/fakes.rs`.
2. Add the Windows implementation to `tc-platform-win`.
3. Write the trait's tests against the fake (fast, runs everywhere) and
   against the real implementation (slower, runs on Windows).

### The helper rule

The indexer, the OCR helper and the dictation helper are **separate
processes**. A large scan, a large document and a large model must never block
the palette, and a crash in any of them must not take the app down.

Keep them separate. They communicate through their standard output as JSON.

---

## 3. The parity workflow

This is the loop you will repeat most often.

### Step 1 — find the row

```powershell
cargo run -p xtask -- parity --summary
```

That prints the counts. For the rows themselves, look in `parity/parity.toml`
or in `PARITY.md`. Rows are named `<heading>.<slug-of-behaviour>`.

### Step 2 — find the upstream behaviour

The upstream source is the authority, not the documentation. The Swift sources
are under `upstream/Tinycast/Features/<Feature>/`, and the tests are under
`upstream/Tests/`.

> **Reading upstream files:** this repository's tooling decodes UTF-8
> correctly, but some editors in this environment do not. If a file shows
> mojibake (`Ã—` for `×`, `â` for `→`), the file is fine and your reader is
> wrong. Read it with an explicit UTF-8 decode rather than assuming the file
> is broken.

### Step 3 — write the failing test

Put it in the crate that will own the behaviour. Name it in full sentences,
because the name is the evidence:

```rust
#[test]
fn the_volume_action_can_be_set_to_a_percentage() {
    // ...
}
```

### Step 4 — implement it

Write the smallest thing that makes the test pass.

### Step 5 — record the evidence

Update the row's `status` and `evidence` in the part file:

```toml
[[row]]
id = "launcher.system-actions.set-the-volume-to-a-level"
page = "launcher/system-actions"
behavior = "Set Volume to 25%"
status = "PORTED"
evidence = ["tc-core::the_volume_action_can_be_set_to_a_percentage"]
```

### Step 6 — regenerate and check

```powershell
cargo run -p xtask -- parity
cargo run -p xtask -- parity --check
cargo test --workspace
```

### Step 7 — commit

Small, conventional commits:

```text
feat(tc-calc): port the unit conversion rules
fix(tc-platform-win): close the clipboard handle on failure
docs: record the decision about the trash
chore: update the dependency licences
```

---

## 4. Translating macOS to Windows

The port is a translation. Upstream's Swift is the authority for *what* a
feature does; Windows decides *how*.

The full table is in [`docs/WINDOWS-MAPPING.md`](docs/WINDOWS-MAPPING.md). The
short version:

| Upstream | YAL |
|---|---|
| ⌘ | <kbd>Ctrl</kbd> |
| ⌥ | <kbd>Alt</kbd> |
| ⇧ | <kbd>Shift</kbd> |
| ⌃ | <kbd>Ctrl</kbd> + <kbd>Alt</kbd> |
| ⌘ Delete | <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>Backspace</kbd> |
| ⌥ Tab | <kbd>Ctrl</kbd> + <kbd>Tab</kbd> |
| ⌘ Esc | <kbd>Ctrl</kbd> + <kbd>Esc</kbd> |
| The menu bar | The notification area |
| Spaces | Virtual desktops |
| Apple Shortcuts | Automations |
| The Trash | The Recycle Bin |
| Secure Input | Administrator windows, the secure desktop |
| `~/Library/Application Support/com.tinycast.app/` | `%APPDATA%\yal\` |
| `~/.config/tinycast/settings.json` | `%USERPROFILE%\.config\tinycast\settings.json` |

### The shortcut rule that matters most

**A built-in shortcut must never shadow standard text editing.**
<kbd>Ctrl</kbd>+<kbd>C</kbd> is copy. If a shortcut would intercept it, the
shortcut is wrong, not the convention.

---

## 5. Commands you will use

```powershell
# Everything, in order
.\scripts\build.ps1

# The tests
cargo test --workspace
cargo test -p tc-core
cargo test -p tc-core the_built_in_command_list_is_complete

# The linters
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# The ledger
cargo run -p xtask -- ledger
cargo run -p xtask -- parity
cargo run -p xtask -- parity --summary
cargo run -p xtask -- parity --check
cargo run -p xtask -- tags

# The build
cargo run -p xtask -- dist
cargo run -p xtask -- dist --arch x64
cargo run -p xtask -- selftest

# The strings
cargo run -p xtask -- i18n

# The application
.\target\release\yal.exe --self-test
.\scripts\smoke-test.ps1
```

---

## 6. What "done" means

A feature is done when all of these are true:

- [ ] The behaviour works on real Windows, not just in a fake.
- [ ] There is a test that fails without the change and passes with it.
- [ ] The parity row says `PORTED` and names that test.
- [ ] `cargo run -p xtask -- parity --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- [ ] `cargo test --workspace` passes.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] Any decision you made is in `docs/DECISIONS.md`.
- [ ] Nothing in the diff is a placeholder.
- [ ] Your report says what you verified and what you did not.

---

## 7. Your report

When you finish, write a report that contains **only verified claims**.

Say what you ran and what it printed. If a test failed, say so. If something is
unfinished, say so and point at the ledger row.

Never write "all tests pass" unless you ran the suite and it passed. Never
write "the feature is ported" unless the ledger row says `PORTED` and names
the test that proves it.

A report that is accurate about its gaps is more useful than one that is
confident and wrong.

---

## 8. Things that will bite you

**The `read` tool mis-decodes UTF-8 in this environment.** Use an explicit
UTF-8 decode when the content matters.

**Windows-rs 0.62 moved things.** `BOOL` is `windows::core::BOOL`, `HANDLE`
is in `Win32::Foundation`, `CF_UNICODETEXT` is in `Win32::System::Ole`,
`TOKEN_QUERY` is in `Win32::Security`. `GetWindowRect`, `OpenProcessToken` and
`GetTokenInformation` return `Result` and take out-parameters.

**Raw pointers are not `Send`.** Wrap a kernel handle in a newtype with
`unsafe impl Send` rather than casting.

**Closure capture is precise.** `move || waiting.0` captures the field, not
the wrapper, so the wrapper's `Send` impl is not enough. Bind the whole value
inside the closure.

**The parity validator is strict about tier.** A row's tier must agree with
its page's tier in `parity/pages.toml`.

**A `NOT PORTED` row needs a reason.** One line, in English, saying why.

**An `asserted` count check needs a test too.** `kind = "rows"` checks that
the tagged rows number the right amount; `kind = "asserted"` checks that a
test proves the number. Both need evidence. An `asserted` check with no
evidence is reported as pending, never as passed.

---

## 9. Where things are

| You want to know | Look in |
|---|---|
| What is in scope | `parity/pages.toml`, `PARITY.md` |
| What is decided | `docs/DECISIONS.md` |
| Why a port note exists | `docs/port-notes/` |
| What happened today | `docs/SESSION-LOG.md` |
| How the code is arranged | `docs/ARCHITECTURE.md` |
| How macOS maps to Windows | `docs/WINDOWS-MAPPING.md` |
| Every shortcut | `docs/SHORTCUTS.md` |
| How far the port has got | `PROGRESS.md` |
| What the original does | `upstream/` (git-ignored), and the upstream docs |
