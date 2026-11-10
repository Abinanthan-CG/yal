# Port note 002 — The parity ledger is generated

**Date.** 2026-11-10

---

## The problem

The brief requires a ledger that lists **every feature and sub-feature** with
exactly one of two statuses, and that marks a row `PORTED` only when real
evidence exists.

The upstream documentation has 42 pages. Between them they describe 391
headings and 2 309 distinct behaviours. Writing 2 309 rows by hand is how you
get 2 309 rows that are subtly wrong: one behaviour missed, one duplicated,
one with a typo in its page path.

## What was tried first

The plan was to author the rows in fragments under `parity/parts/`, several
people at once. Four parallel workers were dispatched. Three produced nothing
usable inside the time limit, and the one that finished:

- duplicated rows that the generator already produced, and
- carried a page-path typo, so every row it wrote pointed at a page that
  does not exist.

Hand-authoring was abandoned.

## What is done instead

`cargo xtask ledger` generates the rows from the documentation.

```text
upstream-docs/**/*.md
        │
        ▼
  the generator
        │
        ▼
parity/parts/generated/*.toml   (committed)
        │
        ▼
  cargo xtask parity
        │
        ├──► parity/parity.toml  (committed, 2.37 MB)
        └──► PARITY.md            (committed, 942 KB)
```

### What counts as a row

The generator walks each page and produces a row for:

| In the documentation | Rows produced |
|---|---|
| A heading | One row for the heading's behaviour |
| A table row | One row per cell that describes a behaviour |
| A list item | One row per item |
| A paragraph | One row per paragraph that describes a behaviour |
| A `·`-separated list | **One row per item**, with the category kept as the row's `group` |
| A shorthand percentage | Folded out — `Set Volume to 0%` followed by `25%` becomes `Set Volume to 25%` |

That last two rules matter. A list like:

```text
- Set Volume to
  · 0%
  · 25%
  · 50%
  · 100%
```

is **three** behaviours, not one, and each needs its own row, because each
needs its own evidence.

### The result

| | |
|---|---|
| Pages | 42 |
| Headings | 391 |
| Rows | 2 309 |
| Tier 1 rows | 1 815 |
| Tier 2 rows | 494 |

## What stays human

The generator writes the *behaviour*. A person writes the *judgement*:

```toml
status = "PORTED"
evidence = ["tc-core::the_built_in_command_list_is_complete"]
```

or:

```toml
status = "NOT PORTED"
reason = "Apple Shortcuts have no Windows equivalent; automations cover Power Automate Desktop."
```

That is the part that needs a decision, and the part the validator checks.

## The validator

`cargo xtask parity` refuses to write a ledger that:

- uses a status other than `PORTED` or `NOT PORTED`;
- has a duplicate row identifier;
- puts a row on a page that is not in `parity/pages.toml`;
- has a row whose tier disagrees with its page's tier;
- leaves a generated heading without a row;
- marks a row `PORTED` without evidence naming a test that exists in a crate
  that exists;
- marks a row `NOT PORTED` without a reason;
- binds a `rows` count check to the wrong number of rows;
- leaves an `asserted` count check without evidence.

An `asserted` count check with no evidence is reported as **pending**. It is
never reported as passed. That distinction matters: upstream states many
counts ("the tap window is 250 ms") that are properties a test must prove
later, and pretending they are covered would make the ledger a lie.

## The check CI runs

```powershell
cargo run -p xtask -- parity --check
```

This regenerates the ledger in memory and compares it byte for byte with the
committed files. A stale ledger fails the build.

The output has to be **deterministic**, which is why the writer is a
hand-rolled serialiser rather than `toml_edit`: `toml_edit`'s array API
cannot hold tables in the form needed, and a ledger that reformats itself on
every run produces noise that hides real staleness.

## See also

- [`docs/DECISIONS.md`](../DECISIONS.md)
- [Port note 001 — Getting the upstream sources and the documentation](001-upstream-acquisition.md)
