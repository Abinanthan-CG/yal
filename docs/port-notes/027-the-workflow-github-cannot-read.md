# 027 - The workflow GitHub cannot read

Date: 2026-11-15
Subject: a red cross on every push, and the one line that caused it

## The symptom

Every push to the branch produced two runs. `CI` was red for a reason that
was visible in the log. The other one was not a run at all, really:

```
.github/workflows/release.yml          failure      0s
```

It lasted no time. It had no jobs. Its name was the *path of the workflow
file* rather than the name the file gives itself. And it happened on pushes
that did not touch the file - which was the first thing that should have
said "this is not about the content of the file".

The file was not a candidate for corruption, either. It was byte-identical
to the copy GitHub had, ASCII from the first byte to the last, with no
duplicate keys, no tabs, no BOM and no trailing whitespace. A YAML parser
read it. So did a JSON Schema for GitHub workflows - zero errors.

## Where the reason actually was

Not in the repository. In the run's annotations, which the API returns only
as a check-suite with no check runs in it, and which the HTML of the run
page carries in a partial that is fetched after the page loads:

```
Invalid workflow file: .github/workflows/release.yml#L1
(Line: 63, Col: 13): Unrecognized named-value: 'secrets'.
Located at position 1 within expression: secrets.WINDOWS_CERTIFICATE_PFX != ''
```

Line 63 was this:

```yaml
      - name: Sign the binaries
        if: ${{ secrets.WINDOWS_CERTIFICATE_PFX != '' }}
```

## The rule

GitHub does not hand the `secrets` context to the `if` of a job or a step.
The contexts it does hand them are:

```
env, github, inputs, job, matrix, needs, runner, steps, strategy, vars
```

`secrets` is not among them, because a secret is not known until the job
that holds it is running, and a condition that gates a step has to be
decidable before the step's environment exists.

This is not a schema rule, which is why the schema check passed. The JSON
Schema for workflows says nothing about which contexts may be read where.
Neither does a YAML parser. Only GitHub knows, and it only says so in the
one place nobody looks.

## The fix

The secret has to travel through the job's environment, which is set before
any step runs, and the step looks for it there:

```yaml
  build:
    env:
      WINDOWS_CERTIFICATE_PFX: ${{ secrets.WINDOWS_CERTIFICATE_PFX }}
    steps:
      - name: Sign the binaries
        if: ${{ env.WINDOWS_CERTIFICATE_PFX != '' }}
```

Putting the `env:` on the *step* would have been wrong in a quieter way: a
step's own environment is not yet in scope for its own `if`, so the value
would read as empty, the condition would always be false, and the signing
would never happen - without a single error anywhere.

## Three other things the release path needed

The release workflow had never run, because it had never been readable. So
the failure it would have had next was still waiting. Reading it found
three:

- **The ARM64 build tools.** The ARM64 build needs the MSVC ARM64 compiler
  and linker, which the runner image does not always carry. The workflow
  now looks for them and installs them only when they are genuinely
  missing, so the common case costs nothing.
- **The target being built for.** `rust-toolchain.toml` pins both targets,
  but the workflow did not add the one it was about to build.
- **The signing key.** `mlugg/setup-minisign` fails when the key is
  missing. The key is optional - a release without one still goes out,
  unsigned, and saying so - so the signing steps are now gated on the key
  being there.

## The linter

`scripts/check-workflows.py` checks the workflow files the way GitHub
checks them: the YAML parses, the workflow has a name, an `on` and jobs,
every job has a `runs-on` and steps, every step does something, and - the
rule that cost the time - no `if` reads a context GitHub does not give it.

It has a self-test, and the self-test is pointed at the exact file GitHub
refused, and at the corrected version of it. A linter that never says no is
not a linter, so it has to be shown saying no to the right thing.

CI runs both.

## What this note is about

The mistake was not the `secrets` line. The mistake was that a thing which
is checked by a machine we do not control, at a moment we do not choose,
was not checked by us first. A schema check was not enough, because the
rule that broke was not in the schema. What is in the schema was checked;
what was not in the schema was found by GitHub, four pushes later, in a
place that does not appear in the repository.

So the workflow files are now checked locally and in CI, by a script that
knows the rule that GitHub knew and the schema did not.
