# 019 - The reductions over a spoken list, and the math asked in words

Date: 2026-11-13
Crates: `crates/tc-calc/src/statistics.rs`,
`crates/tc-calc/src/statistics_tests.rs`, `crates/tc-calc/src/spoken.rs`
Upstream: `Tinycast/Features/Calculator/Model/CalcStatistics.swift`,
`Tinycast/Features/Calculator/Model/CalcSpokenMath.swift`

## What this slice is

The two layers that answer a question asked in words rather than typed as
arithmetic: the **reductions over a list** - `median of 3, 1, 2` - and the
**math asked in words** - `is 97 prime`, `5 choose 3`,
`6 is to 600 as 8 is to what`.

## The reductions (`statistics.rs`)

The phrase is **lowered to the function call it names**, so a list is reduced by
the same arithmetic as `median(3, 1, 2)` - and the badge says which reduction
ran, so `min` and `max` are told apart on the card.

| The phrase | The answer | The badge |
| --- | --- | --- |
| `median of 3, 1, 2` | `2` | `Median` |
| `count of 4, 8, 15, 16, 23, 42` | `6` | `Count` |
| `standard deviation of 2, 4, 4, 4, 5, 5, 7, 9` | `2.138089935` | `Standard Deviation` |
| `sum of 10 and 20` | `30` | `Sum` |
| `larger of 3 and 7` | `7` | `Maximum` |
| `midpoint between 10 and 30` | `20` | `Average` |
| `gcd of 12 and 18` | `6` | `Result` |
| `gcf of 20 and 30` | `10` | `Result` |
| `lowest common multiple of 4 and 6` | `12` | `Result` |

**A list is separated by commas or by `and`**, and the separator only counts at
the top level: `sum of 2*3, 4` stays two operands. The standard deviation is
the **sample** deviation, the same as `STDEV` in a spreadsheet.

## The math asked in words (`spoken.rs`)

Upstream's shapes, in upstream's order:

| The phrase | The answer | The badge |
| --- | --- | --- |
| `remainder of 20 divided by 3` | `2` | `Remainder` |
| `is 97 prime` | `true` | `Prime Number` |
| `clamp 20 between 5 and 15` | `15` | `Clamped` |
| `5 root of 32` | `2` | `Root` |
| `6 is to 600 as 8 is to what` | `800` | `Proportion` |
| `5 choose 3` | `10` | `Combinations` |
| `5 permutation 3` | `60` | `Permutations` |
| `logarithm 81 base 3` | `4` | `Logarithm` |
| `1000 is 10 to the power of what` | `3` | `Logarithm` |
| `5 as multiplier of 1` | `5x` | `Multiplier` |

### The rules that are easy to get wrong

- **A prime check works for every whole number up to 9 007 199 254 740 992.**
  It is Miller-Rabin with the bases that are deterministic for every 64-bit
  whole number, and the modular multiplication is done in 128 bits so it never
  overflows.
- **A prime answer does not chain.** `is 97 prime` is the end of the query, so
  the answer cannot be carried into the next calculation - and neither can a
  multiplier.
- **The combinations run the product, not the factorial**, so neither ever
  forms a factorial larger than its answer.
- **A multiplier is written with an `x`**: `5x`, `2x`.
- **A logarithm is answered in either order**: `logarithm 81 base 3` and
  `1000 is 10 to the power of what`.

## The engine now tries every layer

Upstream's engine tries every layer before it gives up. **This port stopped at
the first layer that reported an area it could not answer**, and that was
wrong: `5 as multiplier of 1` has a word the arithmetic reads as a unit -
`as` is the attosecond - and the phrase layer answered it anyway.

The engine now keeps the area it could not answer and reports it only when
nothing else answers, exactly as upstream does. The answer for a query that
needs a unit is the same as before - `10 kg` still says the units are not
supported - and the answer for a query that only *looks* like it needs one is
now the right one.

## What is not in this slice

- **The lists that carry a unit**: `sum of 1 km and 500 m` is `1.5 km`, and
  `2 km is to 4 km as what is to 10 kg` is `5 kg`. Both need the arithmetic that
  carries a unit, which is not built yet.
- **The questions asked in words** - `what is 20% of 50` - which is upstream's
  own question layer.
- **The exchange rates**, so the money is never converted.

## The rows that moved

**Twenty-one rows** of the calculator page are now `PORTED`, with the test that
proves each one: eight rows of *Lists and statistics* - the median, the count,
the standard deviation, the larger of two, the midpoint, the greatest common
divisor, the greatest common factor and the least common multiple - and
thirteen rows of *Math in words* - the two prime checks, the row that says the
prime check works for the whole range, the remainder, the clamp, the
proportion, the combination, the permutation, the two logarithms, the root and
the two multipliers.

**225 → 246 rows PORTED.**

## Test count

`tc-calc` has 194 tests: 17 for the reductions and the math asked in words, and
the rest for the arithmetic, the units, the percentages, the money, the shared
amounts and the phrases about money over time. The workspace has **834 tests**.
