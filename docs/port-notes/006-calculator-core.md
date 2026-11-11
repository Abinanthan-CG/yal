# 006 - The calculator's arithmetic core

Date: 2026-11-11
Crate: `crates/tc-calc`
Upstream: `Tinycast/Features/Calculator/Model/` (49 Swift files) and `Tests/calc-test.swift`

## What this slice is

The calculator is the largest single page in the ledger: 326 rows, more than a
seventh of everything upstream documents. Upstream splits it across 49 Swift files,
and `Tests/calc-test.swift` is 150 KB.

This slice ports the **arithmetic core** — the part that answers a query without
needing a unit catalog, a currency table, a calendar or a time zone:

| Our module | Upstream file | What it does |
| --- | --- | --- |
| `token.rs` | `CalcToken.swift`, `CalcOperator.swift` | the tokens and the operators |
| `tokenizer.rs` | `CalcTokenizer.swift` | the text becomes tokens |
| `parser.rs` | `CalcExpressionParser.swift` | the tokens become a tree |
| `engine.rs` | `CalcEngine.swift` | the tree becomes an answer |
| `math.rs` | `CalcMath.swift` | the functions and the constants |
| `formatter.rs` | `CalcFormatter.swift` | the answer becomes text |
| `rounding.rs` | `CalcRounding.swift` | rounding to a step, a direction, a place |
| `catalog.rs` | — | the seam to the units and the currencies |

## The oracle

`upstream/Tests/calc-test.swift` is the acceptance oracle, exactly as it was for the
search slice. Its expected values were read out of that file, not computed from our
own implementation. A spot check of the file against our tests:

| Upstream | Ours |
| --- | --- |
| `expectDisplay("2^3^2", "512")` | `assert_display("2^3^2", "512")` |
| `expectDisplay("2**3**2", "512")` | `assert_display("2**3**2", "512")` |
| `expectCopy("-9007199254740992 + 0", "-9007199254740992")` | `assert_copy("-9007199254740992 + 0", "-9007199254740992")` |
| `expectCopy("-0 * 1234", "0")` | `assert_copy("-0 * 1234", "0")` |
| `expectDisplay("10 divided by 2", "5")` | `assert_display("10 divided by 2", "5")` |
| `expectDisplay("2 trillion", "2,000,000,000,000")` | `assert_display("2 trillion", "2,000,000,000,000")` |
| `expectDisplay("remainder of 20 divided by 3", "2")` | **not supported yet** |

`tc-calc` has 52 tests and one doc test.

## The seam to the units and the currencies

The engine does not own a unit catalog or a currency table. It asks a `Catalog` —
an interface with `unit()` and `currency()` — and answers
`CalcOutcome::Unsupported { area, detail }` for a query it cannot finish. That is
a real answer, not a stub: `10 kg` against a catalog that knows `kg` reports
`Unsupported { area: "units", detail: "kg" }`, and `10 zzz` against the same
catalog earns no card at all, because `zzz` is not a unit.

This is the shape stage 2 will fill: the units and the currencies become a second
catalog implementation, and the engine does not change.

## What is deliberately not here

- **The units and the quantities** — `CalcUnits`, `CalcUnitCatalog`,
  `CalcUnitExpression`, `CalcQuantity`, `CalcMixedUnits`. This is stage 2, and it
  is the largest remaining piece of the calculator.
- **The currencies and the crypto rates** — `CalcCurrency`, `CurrencyData.generated`,
  `CurrencyFeed`. A currency needs a rate source, which upstream caches for 24 hours.
- **The dates, the times and the time zones** — `CalcDateTime`, `CalcDateFacts`,
  `CalcTimeZone`, `CalcZoneSchedule`, `CountryZoneData.generated`.
- **The percentages** — `CalcPercent`.
- **The statistics, the finance, the growth, the splitting, the spoken maths, the
  holidays, the substance, the pitch, the pixel density** — these are small files
  each, but each is its own feature.

## Three behaviours upstream has that this slice does not

Found by reading upstream's own tests and checking them against the port:

- `remainder of 20 divided by 3` — the word order upstream uses for a remainder is
  not read yet; only `mod` is.
- `5 root of 32` — the root with the index first is not read yet; only
  `root 4 of 81` is.
- `is 97 prime`, `is 91 a prime number`, `5 choose 3`, `5 permutation 3`,
  `6 is to 600 as 8 is to what`, `clamp 20 between 5 and 15`, `2 trillion` — of
  these, only `2 trillion` is ported. The primality check, the combinations, the
  permutations, the proportions and the clamp are not in the core.

Each of these is a row in the ledger with its own reason naming the missing piece.

## A note on how this slice was built

The first draft was written by a background worker agent, which read the upstream
Swift and wrote the crate. Two things it left behind were fixed by hand:

1. The tests were written into `crates/tc-calc/src/tests.rs` but the module was
   never declared, so **zero tests ran** while the crate still compiled and the
   doc test still passed. `#[cfg(test)] mod tests;` is now in `lib.rs`.
2. Four clippy errors were left (`-D warnings` fails on them).

The lesson is worth keeping: a crate that compiles is not a crate that is tested.
`cargo test` output has to be read, not assumed.
