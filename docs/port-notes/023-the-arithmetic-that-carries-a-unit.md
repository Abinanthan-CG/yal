# 023 - The arithmetic that carries a unit

Date: 2026-11-13
Crate: `crates/tc-calc`
Subject: the measurement a value carries, and the arithmetic between measurements

## Why this note exists

Upstream's calculator does not answer `10kg + 500g` with a number. It answers
with a number *and a unit*: `10,500 g`. The unit is not decoration on the
answer - it is part of the value, and it decides what the next operation means.

YAL had the unit table and the conversions between two units, but a value
carried no unit at all: `Value` held an amount, a percentage flag, a boolean
flag and a currency. `10kg + 500g` was therefore two unsupported words, and
the whole unit section of the parity ledger - forty rows - sat at
`NOT PORTED` with the reason "the calculator engine is not ported yet".

So this slice gives a value its unit back, and writes down what that means.

## The measurement

`crates/tc-calc/src/measure.rs` holds the new type. It is upstream's `UnitDef`
with the parts a calculation needs:

| Field | What it is |
| --- | --- |
| `symbol` | how the unit is written: `kg`, `km/h`, `kg/m³` |
| `name` | the long label for the card's badge: `Kilograms` |
| `category` | what the unit measures |
| `factor` | the scale onto the base unit |
| `offset` | the offset onto the base unit; only the temperatures have one |
| `dimension` | the dimension the unit has |

A measurement is `Copy`, so it can be carried through a calculation without a
heap allocation on every step. The catalog hands it over as a shared
reference into the catalog's own table, which is why `Catalog` grew one method:

```rust
fn unit_ref(&self, _name: &str) -> Option<&'static UnitDef> { None }
```

A catalog that knows the names but not the scales answers `None`, and the
core then carries no unit rather than a guessed one.

### The compound unit

Two measures of different kinds make a third: `2kg / 4m3` is `0.5 kg/m³`, and
`1kg * 1m` is `1 kg·m`. The symbol keeps the way the two were joined, because
`kg/m³` and `kg/m³` are not the same way of writing.

Upstream builds that symbol with string concatenation, so the symbol of a
compound is decided when it is made. This port does the same, and keeps one
copy of every symbol it has met - a calculator meets the same compound again
and again, and the copy is what makes two compounds with the same writing the
same measurement. The copies are few and short; the registry that holds them
is in `measure.rs`.

### The base unit of a dimension

A combined measurement is answered in the base unit of the dimension it lands
on, so five metres times four metres is twenty square metres rather than a
compound. The base units are upstream's `CalcUnits.baseUnits`: a metre, a
kilogram, a second, and the rest. The list is in `measure.rs`.

### The unit a rate accrues in

A rate multiplied by a time accrues in the numerator of the rate, so
`60 mph * 2.5 hours` is `150 mi` and `90km/h * 20min` is `30 km`. Power over
time gives watt hours or kilowatt hours, and current over time gives ampere
hours or milliampere hours. This is upstream's `CalcUnits.productUnit`, and
the unit it picks is the *accrued* one - the mile of the mile per hour, not
the metre of the metre per second.

## What the parser does with it

`crates/tc-calc/src/parser.rs` carries the unit through the calculation.

**The last unit typed decides the unit of the answer.** `10kg + 500g` is
`10,500 g`; `500g + 1kg` is `1.5 kg`. A written `+` converts the left side
into the unit of the right side.

**Composite notation is the exception.** `5 feet 3 inches` is one amount in
composite notation, not a sum, so it answers in the *leading* unit: `5.25 ft`.
The invisible `+` between two quantities written side by side binds tighter
than a multiplication, so `5w * 3h 30min` is five watts times three and a
half hours, and `90km / 1h 30min` divides by the whole ninety minutes. This
is upstream's `CalcExpressionParser.peekBinary` reporting the operator with
`consumesToken: false`, and `addOrSubtract` keying the unit choice off
exactly that flag.

**A half-typed unit earns no card.** `1hr 30` is one keystroke short of
`1hr 30min`, and answering `31 hr` would be worse than answering nothing.

**A bare amount takes the unit next to it.** `5kg + 5` is `10 kg`.

**Two measures of one kind divide into a plain ratio.** `5kg / 2kg` is `2.5`,
with no unit on the answer.

**A plain amount over a unit is the inverse of that unit.** `1 / 20ms` is
`50 Hz`.

**A root raises the unit as well.** `square root of 25m2` is `5 m`, and the
cube root of `-8m3` is `-2 m`, because an odd root keeps the sign.

## The card

The card shows the unit with the number, and the badge on its right names the
unit it answered in: `10kg + 5kg` shows `15 kg` with `Kilograms` on the
right. The expression it echoes is normalized the way upstream normalizes it:
`10 kg + 5 kg`, and `5 ft 3 in` for `5 feet 3 inches`.

A conversion can sit at the end of a calculation: `10kg + 500g to lb` is
`23.14853753 lb`, and the badge names `Pounds`. A plain amount before the
connector is still answered by the conversion layer, with its own badges -
`10kg to lb` is `22.04622622 lb` with `Kilograms` and `Pounds` - so a
calculation and a conversion do not look alike.

## What the documentation loses in its markdown

The unit section of upstream's documentation writes its examples with a
multiplication sign, and the markdown reads the sign as emphasis and drops
it. The documentation says `5m 4m`; the example is `5m * 4m`. The
documentation says `90km/h 20min`; the example is `90km/h * 20min`. The
documentation says `60 mph 2.5 hours`; the example is `60 mph * 2.5 hours`.

Upstream's own tests write the sign out, and without it the two quantities
stand side by side and are one amount: `5 feet 3 inches` is `5.25 ft`, not
twenty square metres. This port answers both readings the way upstream does,
and the test `the_documentation_loses_a_multiplication_sign_in_its_markdown`
holds both.

The ledger rows that quote the documentation without the sign are marked
`PORTED` with the evidence of the example *with* the sign, and this note is
the place where that is written down.

## The tests

`crates/tc-calc/src/measure_tests.rs` holds them. Every expectation in the
file is upstream's own, taken from `Tests/calc-test.swift` and from the
calculator page of the documentation. Twenty-two tests, and the whole file
runs against the unit catalog and the full catalog.

The two placeholder tests that said a calculation with a unit "is not
answered yet" now assert the answer.

## What is still not ported

Twelve rows of the unit section stay open, each with its own reason:

- a conversion in the *middle* of a calculation (`10kg to lb + 3lb`) - the
  unit the amount is converted into is not carried into the addition;
- a conversion asked in words (`Convert 5 km into miles`, `how many feet in
  a mile`) - there is no phrase layer that reads them;
- a conversion of money in the middle of a calculation - the rates of the
  currencies are not fetched yet;
- the targets `duration` and `timespan` - a duration is not answered in the
  parts that make it;
- a target that is itself a compound unit (`1kg/m3 to g/cm3`) - the unit of
  the answer is ported, but the target is not read.

Two more things are known to differ and are not in the ledger because they are
not rows of their own: the multi-argument functions lose the unit
(`min(1km,999m)` answers `1` rather than `0.999 km`), and a mismatch of
measures answers with no card rather than with the error upstream shows
(`5w * 3h + 30min`).
## The second half of the slice: the functions of a measurement

A function that takes a list of measures answers in the unit of its first
argument: `min(1km,999m)` is `0.999 km`, not `1`, and `sum(1km,500m)` is
`1.5 km`. The arguments are told in that unit first, and the answer keeps it.

The functions that do this are upstream's `CalcMath.measurements`: `hypot`,
`round`, `min`, `max`, `sum`, `avg`, `mean`, `average`, `median`, `stddev` and
`stdev`. The list was already in `tc-calc::math`; the parser now uses it.

The phrases that reduce a list keep the unit too, because they are lowered to
the same function call: `sum of 1 km and 500 m` is `1.5 km`, and the card is
labelled `Sum` rather than `Result`.

### A unit beside another unit name

`60 miles per hour` is `60 mi/hr`, not a division of two measures. The rule is
upstream's: when a unit is followed by a multiplication or a division and then
the *name* of another unit, the two make a compound and the amount stands
alone. `10 kg/m3` is `10 kg/m³` for the same reason, and `50 usd per hour` is
`50 USD/hr`, because money is a unit of its own dimension.

The compound is read by a small reader of unit expressions, upstream's
`UnitParser`: a unit, a group of units in brackets, a power, and a run of units
joined by `*` or `/`.

### The percentage after a measure

Two things had to be true for `50 as % of 200` to be `25%`:

- a percentage is a plain amount, so the parse fails when a percentage follows
  a measure - upstream's `parseOperand` asks for a scalar there;
- a phrase that wants a plain amount does not take one that carries a measure -
  upstream's `CalcExpressionParser.scalar` asks for the scalar kind.

Without the second one, `50 as % of 200` answered `100`: the `as` was read as
the unit of a coulomb, and the phrase took the fifty as a plain percentage.

The same rule applies to the percentage phrases, the finance phrases, the
growth phrases, the spoken phrases and the rounding phrases: a run of tokens
that has to be a plain amount is not one when it carries a measure.