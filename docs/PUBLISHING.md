# Publishing YAL

The port is published from one repository: **`https://github.com/Abinanthan-CG/yal`**.

That repository already exists and holds the earlier, unrelated YAL build - a
pure-Rust launcher written with egui. When the port is finished, that content
is replaced by this one. This page is the procedure, written down so the last
step of the build is mechanical and nothing is lost by accident.

## What the publish does, and what it destroys

The repository is **public**. Flushing it means the earlier build's history,
its issues and its pull requests stop being reachable at that address. The
earlier build was never released and has no tags, so nothing a user could
have installed disappears - but the history is gone from the repository, and
the only copy left is whatever was pushed elsewhere.

**The flush and the push are one action, and they are not reversible from
GitHub.** The local clone of the earlier build stays where it is
(`C:\Users\dabin\development\yal`), so its history is still on this machine.

## Before the flush

Everything below has to be true, and each one is checked rather than assumed.

| Check | Command | What it proves |
| --- | --- | --- |
| The tree is clean | `git status --short` | Nothing uncommitted is lost |
| The tests pass | `cargo test --workspace` | The code that is published works |
| The linter is clean | `cargo clippy --workspace --all-targets -- -D warnings` | No warnings are published |
| The ledger is current | `cargo run -q -p xtask -- parity --check` | `PARITY.md` matches the rows |
| The builds are there | `cargo run -q -p xtask -- dist` | `dist/` holds both installers, the zips and the checksums |
| The remote is right | `git remote -v` | It points at `Abinanthan-CG/yal` |

## The steps

```powershell
# 1. Make sure everything is committed and the branch is main.
git status --short
git branch --show-current          # main

# 2. Push our history over the old one.
git push --force origin main

# 3. Make sure the default branch is main and the pages are right.
gh repo edit Abinanthan-CG/yal --default-branch main
gh repo view Abinanthan-CG/yal

# 4. Tag the release and let the workflow build it.
git tag -a v0.1.0 -m "YAL 0.1.0 - the Windows port of Tinycast"
git push origin v0.1.0
```

The release workflow (`.github/workflows/release.yml`) then builds the x64 and
the ARM64 installers on `windows-latest`, signs them if the certificate
secret is set, and publishes the release with the checksums.

## What the repository should say

- **Name:** `yal`
- **Description:** `YAL - a Windows port of Tinycast, the macOS command palette (AGPL-3.0).`
- **Default branch:** `main`
- **Topics:** `windows`, `launcher`, `tinycast`, `tauri`, `rust`, `agpl`
- **License:** AGPL-3.0-only, which `LICENSE` and `NOTICE.md` already carry.

```powershell
gh repo edit Abinanthan-CG/yal `
  --description "YAL - a Windows port of Tinycast, the macOS command palette (AGPL-3.0)." `
  --default-branch main `
  --enable-issues --enable-wiki --enable-projects
gh repo edit Abinanthan-CG/yal --add-topic windows,launcher,tinycast,tauri,rust,agpl
```

## The repository's own rules

The README says YAL is an **unofficial community port** of Tinycast and is
not affiliated with it. That stays true after the publish: this is not
Tinycast, and the original project keeps its own repository, its own issue
tracker and its own support address.

Support for **Tinycast** goes to <https://tinycast.dev/support>, as the
README says. Support for **YAL** goes to the repository's own issue tracker.

## The notices that travel with the port

`NOTICE.md` records the third-party material: the material carried over from
upstream Tinycast, and the parts of the earlier YAL build that this port
reuses. The earlier build was written by the same person who writes this one,
so the notices are a matter of accuracy rather than of obligation - but they
are kept, because they say which code came from where.

## If the push goes wrong

- **A wrong commit went up.** `git push --force origin main` again after
  fixing it. The old history is recoverable from the reflog on this machine
  for as long as the clone lives.
- **The repository was set to private by accident.** `gh repo edit
  Abinanthan-CG/yal --visibility public` puts it back.
- **The default branch is wrong.** `gh repo edit Abinanthan-CG/yal
  --default-branch main`.
