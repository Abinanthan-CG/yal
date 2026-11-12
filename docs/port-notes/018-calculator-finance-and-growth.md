# 018 - The phrases about growth, loans and returns

Date: 2026-11-13
Crates: `crates/tc-calc/src/finance.rs`, `crates/tc-calc/src/finance_tests.rs`,
`crates/tc-calc/src/growth.rs`, `crates/tc-calc/src/growth_tests.rs`,
`crates/tc-calc/src/duration.rs`
Upstream: `Tinycast/Features/Calculator/Model/CalcFinance.swift`,
`Tinycast/Features/Calculator/Model/CalcGrowth.swift`

## What this slice is

The two phrase layers that answer what money does over time: the **compound
growth, loans and returns** of upstream's `CalcFinance`, and the **steady rate**
of upstream's `CalcGrowth`.

`$2,000 after 5 years at 4%` is `2,433.31 USD`, with the badge `Future Value`.
`monthly payment on $20,000 over 5 years at 6%` is `386.66 USD`, with the badge
`Monthly Payment`. `annual return on 500 invested 1000 returned after 5 years` is
`14.8698355%` - the yearly growth rate, the CAGR.

## The three things a phrase carries

The **principal**, the **term** and the **rate**, and they come in either
order: `for 3 years at 7%` or `at 7% for 3 years`. `after` and `over` both read
as `for`, so `$2,000 after 5 years at 4%` and `$2,000 for 5 years at 4%` are
the same question.

**What is asked for comes first**, and it is what the card says:

| The phrase | The badge |
| --- | --- |
| `$2,000 after 5 years at 4%` | `Future Value` |
| `interest on $2,000 after 5 years at 4%` | `Interest` |
| `present value of $1000 after 5 years at 5%` | `Present Value` |
| `monthly payment on $20,000 over 5 years at 6%` | `Monthly Payment` |
| `total payment on $20,000 over 5 years at 6%` | `Total Payment` |
| `monthly interest on 10000 for 5 years at 6%` | `Monthly Interest` |
| `total interest on 10000 for 5 years at 6%` | `Total Interest` |
| `investment required for $50000/year at 4%` | `Investment` |
| `500 invested 1000 returned` | `Return` |

### The rules that are easy to get wrong

- **Interest is added once a year unless the phrase says otherwise.** A trailing
  `compounding quarterly` makes it four times a year, `compounding monthly`
  twelve, `compounding daily` three hundred and sixty-five.
- **A loan's payments are monthly**, and the `monthly`, `annual` and `total`
  badges say how many of them there are: one a month, twelve a year, or all of
  them.
- **The interest of a loan is what the payments add over the principal**, and it
  is spread the way the payments are: the total interest is the monthly
  interest times the number of months.
- **A loan at no interest is the principal divided**, so
  `monthly payment on 12000 over 1 year at 0%` is `1,000`.
- **The yearly return is the CAGR** - the rate that would have taken the amount
  from where it started to where it ended over the whole term.

## The steady rate (`growth.rs`)

Upstream's `CalcGrowth` answers three shapes, all of which name a **period**
(`every year`, `every 2 years`):

| The phrase | The answer |
| --- | --- |
| `20 after 5 years at 5% every year` | `25.52563125` |
| `20 after 5 years at 5% decay every year` | `15.47561875` |
| `time from 20 to 30 at 5% every year` | `8.310386223 yr` |
| `time from 20 to 30 at 2 every day` | `5 day` |
| `growth every year from 20 to 30 over 5 years` | `8.44717712%` |

`growth` and `decay` say which way the rate goes, and the rate is taken as the
sign it is written with: a decay written as a positive rate still decays.

## The durations (`duration.rs`)

Upstream's arithmetic carries a unit on the value, so `5 years` is a unit
value and `5 years / year` is the plain number five. **This port's arithmetic
core does not carry units yet**, so the phrases that need a term read the
duration themselves, the way upstream's `CalcFinance.years` and `CalcGrowth`
do: a number and a time unit, or a time unit on its own - which is how a
period reads, because `every year` is one year.

The units themselves are upstream's own table, unchanged: a year is 31 556 952
seconds and a month is 2 629 746, so `18 months` is a year and a half.

**One deliberate difference.** Upstream divides the money by the year and
multiplies it back by one for `investment required for $50,000/year at 4%`,
which cancels. This port reads the year and drops it, and keeps the money, so
the answer is the same: `1,250,000.00 USD`.

## What is not in this slice

- **The rates that carry a unit.** `$50/hr for 3 hours` is `150.00 USD`, and
  that needs money multiplied by an hour - the arithmetic that carries a unit,
  which is not built yet.
- **The exchange rates**, so the money is never converted.
- **The statistics** - the median, the standard deviation, the lists.
- **The arithmetic asked in words** - `Half of $90`.

## The rows that moved

**Seventeen rows** of the calculator page are now `PORTED`, with the test that
proves each one: the twelve rows of *Interest and loans* - the future value,
the interest, the present value, the three payments, the two interests, the
return, the yearly return, the capital an investment needs, and the row that
describes them all - and the five rows of *Growth and decay*.

**208 → 225 rows PORTED.**

## Test count

`tc-calc` has 178 tests: 13 for the money phrases about growth and loans, 7
for the steady rate, and the rest for the arithmetic, the units, the
percentages, the money and the shared amounts. The workspace has **819 tests**.
