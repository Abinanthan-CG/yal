# 012 - The unit table, and the conversions between two of them

Date: 2026-11-11
Crate: `crates/tc-calc` (`src/units.rs`, `src/mixed_units.rs`, `src/units_tests.rs`)
Upstream: `Tinycast/Features/Calculator/Model/CalcUnitCatalog.swift`,
`CalcUnits.swift`, `CalcDimension.swift`, `CalcMixedUnits.swift`, and
`Tests/calc-test.swift`

## What this slice is

The whole unit table upstream carries, and the conversions between two of them.

A unit is an affine map onto its base: `base = value * factor + offset`. Only the
temperatures carry an offset; everything else is a plain scale. The table is a
list of records — the symbol, the label, the scale, the aliases and, for the
temperatures, the offset — and the metric prefixes and the byte rates are built
from it.

| Upstream's behaviour | Our test |
| --- | --- |
| the everyday units and their scales | `the_table_holds_the_units_upstream_lists` |
| the spelled-out names (`nautical miles`, `sq ft`, `cubic feet`) | `the_table_holds_the_spelled_out_names` |
| the metric prefixes | `the_table_holds_the_metric_prefixes` |
| the byte rates and the bits beside them | `the_table_holds_the_byte_rates` |
| the temperatures carry their offsets | `the_temperatures_carry_their_offsets` |
| a unit is filed under its aliases, not under its symbol | `a_symbol_is_not_itself_a_name` |
| `MW` is megawatts and `mW` is milliwatts | `the_prefixes_are_case_sensitive` |
| a bare `C` is Celsius and a bare `F` is Fahrenheit | `a_bare_c_is_the_temperature_and_not_the_charge` |

### The aliases are the names

Upstream files a unit under its **alias list**, not under its symbol. `KiB` is a
symbol and not a name; `kib` is a name. `K` is the symbol of the kelvin and the
name is `k`. That is why the tokenizer lowercases a word it does not know: `°C`
becomes `°c`, and `°c` is an alias.

The port keeps this exactly. A test says so, because it looks like a mistake
otherwise.

## The conversions

| Upstream's behaviour | Our test |
| --- | --- |
| `10 km to m` is `10,000 m` | `a_length_is_converted_into_another_length` |
| the card names both units | `the_card_names_the_unit_it_came_from_and_the_one_it_became` |
| the answer is copied without the separators | `a_converted_amount_is_copied_without_the_separators` |
| `2 nautical miles to km` is `3.704 km` | `a_nautical_mile_is_converted_into_kilometres` |
| `250 sq ft to m2` is `23.22576 m²` | `a_spaced_unit_name_is_read_as_one_unit` |
| `3 cubic feet to l` is `84.95053978 L` | `a_cubic_measure_is_converted_into_litres` |
| a fraction before a unit is one amount | `a_fraction_before_a_unit_is_one_amount` |
| a mixed number before a unit is one amount | `a_mixed_number_before_a_unit_is_one_amount` |
| `as` works as well as `to` | `a_conversion_can_be_asked_for_with_as` |
| `day s` is one day in seconds | `two_unit_names_are_a_conversion_between_them` |
| a weight converts into another weight | `a_weight_is_converted_into_another_weight` |
| two units that do not fit make an error card | `a_unit_that_measures_something_else_is_an_error_card` |
| a measurement with no target converts into its counterpart | `a_measurement_without_a_target_is_converted_into_its_counterpart` |
| only the metric length asks for feet and inches | `a_metre_is_answered_in_feet_and_inches` |
| `5k` is five thousand, not five kelvin | `a_thousand_is_not_read_as_a_kelvin` |

### The two units of one answer

`1.75 m to feet and inches` is `5 ft 8.897637795 in`, and `2.5 kg to lb & oz` is
`5 lb 8.184904874 oz`. The pair is named with `and`, with `&` or with a plain
space, and the larger unit is whole.

| Upstream's behaviour | Our test |
| --- | --- |
| a measure told in two units | `a_measure_is_told_in_two_units` |
| the three ways to name the pair | `the_two_units_of_a_split_are_joined_in_three_ways` |
| two units of different kinds are not a pair | `two_units_of_different_kinds_are_not_a_mixed_answer` |

## The two things this slice deliberately does not answer

Upstream's engine runs its own **quantity parser** before the conversions, and
that parser is a separate piece of work. Where the quantity parser answers, this
port does not answer at all — it does not answer something different.

| Upstream's behaviour | Why this port leaves it |
| --- | --- |
| `60 mph * 2.5 hours` is `150 mi` | the amount carries a unit and a time, which needs the quantity algebra |
| `1 / 20ms to hz` is `50 Hz` | one divided by twenty milliseconds is a rate, not an amount |

The second one is the reason the conversion reads its amount as **a single
number**. Upstream's `CalcExpressionParser.scalar` reads any expression, but
upstream's engine only reaches the conversions after its quantity parser has
declined the query. This port has no quantity parser, so a conversion answers
only when the amount is one number — `10 km to m`, `5/16" in mm`, `1 1/2 cups to
ml` — and leaves everything else unanswered rather than answering it wrongly.

There is a test for exactly that, and it says what it is protecting:

```rust
assert_ne!(display("1 / 20ms to hz").as_deref(), Some("0.00005 Hz"));
```

## The palette

The calculator's answer is a row in the palette now. It comes first, because a
query that is a calculation is asking for the answer and not for a program, and
opening the row copies the answer to the clipboard.

A query that only repeats itself — `5` — is not a calculation, so the palette
does not crowd its list with the number the user just typed.

## Test count

`tc-calc` has 104 tests: 66 for the arithmetic core, 29 for the units, and the
rest for the rounding and the formatting. Nine of the new tests are the unit
table's own contents and the conversions above.

## What this slice does not do

- **The currencies** — upstream's currency table and its rates are a separate
  slice; the catalog answers `None` for a currency and the engine says so.
- **The dates, the time zones and the percentages** — separate slices.
- **The quantity algebra** — as described above.
- **The questions asked in words** — upstream's `CalcQuestion` turns `how many
  feet in a mile` into an expression. That is a separate feature and those rows
  stay open.
