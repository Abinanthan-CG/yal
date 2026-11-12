# 017 - The percentages, the money and the shared amounts

Date: 2026-11-13
Crates: `crates/tc-calc/src/percent.rs`, `crates/tc-calc/src/percent/tests.rs`,
`crates/tc-calc/src/currency.rs`, `crates/tc-calc/src/currency/table.rs`,
`crates/tc-calc/src/currency_tests.rs`, `crates/tc-calc/src/split.rs`,
`crates/tc-calc/src/split_tests.rs`
Upstream: `Tinycast/Features/Calculator/Model/CalcPercent.swift`,
`Tinycast/Features/Calculator/Model/CalcCurrency.swift`,
`Tinycast/Features/Calculator/Model/CurrencyData.generated.swift`,
`Tinycast/Features/Calculator/Model/CalcSplit.swift`
Generator: `scripts/gen-currency-table.ps1`

## What this slice is

The three layers of the calculator that the plain arithmetic cannot do on its
own: the **percentage phrasings**, the **money**, and the **shared amounts**.

Upstream's calculator is not a calculator with a calculator in it. `20% off 500`
is not arithmetic, it is a phrase, and it answers `400` with the badge
`Discounted`. `50 as % of 200` answers `25%` with the badge `Percentage`.
`85 + 20% split 3` answers `34` with the badge `Per Person`. Those are the
rows this slice moves.

## The percentages (`percent.rs`, 34 tests)

Upstream's `CalcPercent` is a table of shapes: a query has to *have* a shape,
and the shape decides what is done with it. The port keeps the table and the
order.

### The order the phrases are tried in

Upstream tries them in a fixed order, and so does this port:

1. **an amount scaled by a percentage** - `20% off 500`, `15% tip on 42`,
   `30 is 20% of what`
2. **the rule of three with an amount asked for** - `if 30 is 20%, what is 50%`
3. **a trailing tax** - `100 + 10% tax`
4. **a percentage** - `50 as % of 200`
5. **the rule of three with a percentage asked for** - `if 30 is 20% what is 60`
6. **a ratio** - `ratio of 1920 to 1080`

### The word between the percentage and the relation

**A word between the percentage and of, on or off only names it.** So
`20% discount off 100` is `20% off 100`, and `20% markup on 50` is
`20% on 50`.

The words with a meaning of their own are kept: `tip` is the tip on its own, and
a tax that is `in` a price is already inside it - `20% vat in 120` is `20`, not
`144`. `tax`, `sales tax`, `vat` and `gst` all work the same way.

A word the catalog names as a unit or a currency is kept as well, so a unit is
never mistaken for a comment.

### The badges

The card says what it answered, instead of a generic `Result`: `Tip`,
`Discounted`, `Increased`, `Total`, `Result`, `Tax`, `Percentage`,
`Percent Change`, `Proportion` and `Ratio`.

### The rules that are easy to get wrong

- **The percentage in a shape is the number before the `%`, not the fraction.**
  `30 is 20% of what` is `30 × 100 ÷ 20 = 150`, not `30 × 0.2`.
- **A change is signed the way it happened.** `45 is what % off 50` is `10%`
  (what is left), while `66 is what % on 60` is `10%` (what was added) - and
  both are computed as `share - 100` or `100 - share` around the same share.
- **The rule of three reads the two orders alike**: `30 is 20%` and `20% is 30`
  mean the same thing, and `if` and the commas are dropped first.
- **A ratio is reduced**: `1920 to 1080` is `16 : 9`, and only whole numbers
  are reduced, so a ratio that is not whole is not answered.
- **A single number can be a percentage**: `0.125 as %` is `12.5%`, and
  `3/8 as percent` is `37.5%`.

## The money (`currency.rs`, 18 tests)

Upstream's currency table is generated from the CLDR data by
`Scripts/gen-currencies.js`. **This port transcribes that generated file into
Rust with `scripts/gen-currency-table.ps1`**, so the two cannot drift apart by
hand, and the hand-written tables - the contested nouns, the ISO names, the
codes people spell from the sign, and the coins - are written out here.

The table holds **159 fiat currencies and 23 coins**, with the nouns that name
one: the code (`usd`), the noun exactly one currency uses (`afghani`), and the
contested ones (`dollars`, `pounds`, `francs`, `pesos`, `rupees`), which
upstream writes down by hand because no standards body settled them.

**A word several countries share is not guessed at.** `krona` could be Swedish
or Icelandic, so it is not in the table and the query earns no card; `kronor`
is only Sweden's, so it is.

### The money a calculation carries

Upstream's value carries its currency through the calculation, and so does this
port's: `100 usd + 50` is `150.00 USD`, and `20% discount off $100` is
`80.00 USD` - the discount does not take the money off it.

The money is written the way a bill is: `80.00 USD`, two decimals, the code
after the amount.

### The rates are not here yet

Upstream fetches the exchange rates and converts between two currencies. **The
port has no rate feed**, so:

- an amount of money is answered - `100 usd` is `100.00 USD`;
- two *different* currencies in one calculation are **not** answered, because
  the answer would be a made-up number.

The rows that need the rates stay `NOT PORTED`.

## The shared amounts (`split.rs`, 14 tests)

`85 + 20% split 3` is `34`: the amount is calculated first and shared out
afterwards, and it is shared out **before it is rounded**, so
`$1 / 6 split 2` is the same as `$1 / 6 / 2`.

The shapes upstream reads:

| The query | The share |
| --- | --- |
| `split 120 between 4` | `30` |
| `120 split 4` | `30` |
| `120 split among 4` | `30` |
| `120 / 4 people` | `30` |
| `$120 split 4` | `30.00 USD` |
| `Eur 99 among 3 friends` | `33.00 EUR` |
| `85 + 20% split 3` | `34` |
| `20% tip on 85 split 3` | `5.666666667` |

The nouns that name the people - `people`, `persons`, `ways`, `guests`,
`friends`, `diners` - may be left out, and the card says `Per Person`.

A headcount is a number between one and ten thousand; a larger one is not
shared out.

## What is not in this slice

- **The exchange rates**, and everything that needs them: the conversions
  between two currencies, the amount of money on its own converted to the
  user's own currency, the rate on its own, and the refresh button.
- **The arithmetic that carries a unit**, so `750 m as % of 3 km` is still not
  answered: the units are in the catalog, but the arithmetic core does not
  carry them yet.
- **`Half of $90`** - the arithmetic asked in words is not built.
- **The statistics** - the median, the standard deviation, the lists.

## The rows that moved

**Thirty-seven rows** of the calculator page are now `PORTED`, with the test
that proves each one:

- twenty-eight rows of *Percent, ratios and lists* - the discounts, the tips,
  the taxes, the shares, the changes, the rule of three, the ratios, and the
  comment-word rule;
- seven rows of *Splitting a bill* - the shares, the money that is kept, and
  the `Per Person` badge;
- two rows of *Currency and crypto* - the 159 currencies, and the words several
  countries share.

**171 → 208 rows PORTED.**

## Test count

`tc-calc` has 157 tests: 34 for the percentages, 18 for the money, 14 for the
shared amounts, and the rest for the arithmetic, the units and the rounding.
The workspace has **794 tests**.
