#!/usr/bin/env python3
"""Check the workflow files before GitHub does, so the check is cheap.

GitHub refuses to run a workflow file it cannot read, and it reports the
reason in the run's annotations rather than anywhere in the repository. That
makes a mistake here look like a mystery: the run is red, it lasted no time
at all, and it has no jobs.

The rule that bit us is context availability. GitHub does not hand the
`secrets` context to the `if` of a job or a step, so this is an error:

    - name: Sign the binaries
      if: ${{ secrets.CERTIFICATE != '' }}

The secret has to travel through the job's environment instead:

    env:
      CERTIFICATE: ${{ secrets.CERTIFICATE }}
    ...
    - name: Sign the binaries
      if: ${{ env.CERTIFICATE != '' }}

This script checks the rules GitHub checks, so a mistake is a failed build
here rather than a red cross on the repository.

    python scripts/check-workflows.py
"""

from __future__ import annotations

import re
import sys
import tempfile
from pathlib import Path

try:
    import yaml
except ImportError:  # pragma: no cover - the message is the point
    sys.exit("PyYAML is not installed. Run: pip install pyyaml")


REPO_ROOT = Path(__file__).resolve().parent.parent
WORKFLOW_DIR = REPO_ROOT / ".github" / "workflows"

# The contexts GitHub makes available in the `if` of a job or a step.
# `secrets` is deliberately absent: it is not one of them.
STEP_IF_CONTEXTS = {
    "github",
    "needs",
    "strategy",
    "matrix",
    "job",
    "runner",
    "env",
    "vars",
    "steps",
    "inputs",
}

# The contexts GitHub makes available in the `if` of a job.
JOB_IF_CONTEXTS = STEP_IF_CONTEXTS

EXPRESSION = re.compile(r"\$\{\{(.*?)\}\}", re.DOTALL)
# A context is read as `name.property` or `name[property]`, so a context name
# is always an identifier that is followed by a dot or a bracket. An
# identifier that is followed by anything else is a property of a context
# that was already named, a function, or a word.
CONTEXT_USE = re.compile(r"([A-Za-z_][A-Za-z0-9_]*)\s*[\.\[]")


class LineLoader(yaml.SafeLoader):
    """A loader that remembers where each mapping came from."""

    def construct_mapping(self, node, deep=False):  # type: ignore[override]
        mapping = super().construct_mapping(node, deep=deep)
        mapping.setdefault("__line__", node.start_mark.line + 1)
        return mapping


def load(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as handle:
        return yaml.load(handle, Loader=LineLoader)


def normalise(document: dict) -> dict:
    """YAML 1.1 reads the key `on` as the boolean True; GitHub reads it as `on`."""
    if True in document:
        document["on"] = document.pop(True)
    return document


def contexts_in(expression: str) -> set[str]:
    """The context names an expression reads, ignoring plain words and literals."""
    found: set[str] = set()
    for body in EXPRESSION.findall(expression):
        # Drop anything inside a quoted string: `if: ${{ env.X == 'a b' }}`.
        body = re.sub(r"'[^']*'", " ", body)
        body = re.sub(r'"[^"]*"', " ", body)
        for word in CONTEXT_USE.findall(body):
            found.add(word)
    return found


def check_file(path: Path, problems: list[str]) -> None:
    try:
        document = normalise(load(path))
    except yaml.YAMLError as error:
        problems.append(f"{path.name}: the file is not valid YAML: {error}")
        return

    try:
        relative = path.relative_to(REPO_ROOT)
    except ValueError:
        relative = path

    if "name" not in document:
        problems.append(f"{relative}: the workflow has no name")
    if "on" not in document:
        problems.append(f"{relative}: the workflow has no `on` block")
    if "jobs" not in document:
        problems.append(f"{relative}: the workflow has no jobs")
        return

    for job_id, job in (document.get("jobs") or {}).items():
        if job_id == "__line__":
            # The loader leaves its own line marker in every mapping.
            continue
        if not isinstance(job, dict):
            problems.append(f"{relative}: the job {job_id} is not a mapping")
            continue

        job_line = job.get("__line__", 0)

        if "runs-on" not in job:
            problems.append(f"{relative}:{job_line}: the job {job_id} has no `runs-on`")

        condition = job.get("if")
        if isinstance(condition, str):
            for context in sorted(contexts_in(condition)):
                if context not in JOB_IF_CONTEXTS:
                    problems.append(
                        f"{relative}:{job_line}: the `if` of the job {job_id} reads "
                        f"'{context}', which GitHub does not give it. The contexts "
                        f"available are: {', '.join(sorted(JOB_IF_CONTEXTS))}."
                    )

        steps = job.get("steps")
        if steps is None:
            problems.append(f"{relative}:{job_line}: the job {job_id} has no steps")
            continue
        if not isinstance(steps, list):
            problems.append(f"{relative}:{job_line}: the steps of {job_id} are not a list")
            continue

        for index, step in enumerate(steps):
            if not isinstance(step, dict):
                problems.append(f"{relative}: the step {index} of {job_id} is not a mapping")
                continue

            step_line = step.get("__line__", job_line)
            label = step.get("name") or step.get("uses") or f"step {index}"

            if "uses" not in step and "run" not in step:
                problems.append(
                    f"{relative}:{step_line}: '{label}' in {job_id} neither uses an "
                    f"action nor runs a command"
                )

            step_condition = step.get("if")
            if isinstance(step_condition, str):
                for context in sorted(contexts_in(step_condition)):
                    if context not in STEP_IF_CONTEXTS:
                        problems.append(
                            f"{relative}:{step_line}: the `if` of '{label}' in {job_id} "
                            f"reads '{context}', which GitHub does not give it. The "
                            f"contexts available are: {', '.join(sorted(STEP_IF_CONTEXTS))}."
                        )


def self_test() -> int:
    """Check that the checker catches the mistake it was written for.

    A linter that never says no is not a linter. So it is pointed at the
    exact workflow file GitHub refused to read, and at one that GitHub
    would have accepted, and it has to say no to the first and yes to the
    second.
    """
    refused = """
name: The workflow that GitHub refuses
on:
  push:
    branches: ['**']
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - name: Sign the binaries
        if: ${{ secrets.WINDOWS_CERTIFICATE_PFX != '' }}
        run: echo signing
"""

    accepted = """
name: The workflow that GitHub accepts
on:
  push:
    branches: ['**']
jobs:
  build:
    runs-on: ubuntu-latest
    env:
      WINDOWS_CERTIFICATE_PFX: ${{ secrets.WINDOWS_CERTIFICATE_PFX }}
    steps:
      - name: Sign the binaries
        if: ${{ env.WINDOWS_CERTIFICATE_PFX != '' }}
        run: echo signing
      - name: Check the architecture
        if: ${{ matrix.arch == 'arm64' }}
        run: echo arm
"""

    with tempfile.TemporaryDirectory() as directory:
        refused_path = Path(directory) / "refused.yml"
        refused_path.write_text(refused, encoding="utf-8")
        problems: list[str] = []
        check_file(refused_path, problems)

        accepted_path = Path(directory) / "accepted.yml"
        accepted_path.write_text(accepted, encoding="utf-8")
        other: list[str] = []
        check_file(accepted_path, other)

    failures = 0

    if not any("secrets" in problem for problem in problems):
        print("FAIL: the checker did not catch the `secrets` in an `if`.")
        failures += 1
    else:
        print("ok: the checker catches `secrets` in an `if`")

    if other:
        print(f"FAIL: the checker complained about a workflow GitHub accepts: {other}")
        failures += 1
    else:
        print("ok: the checker accepts the workflow GitHub accepts")

    if failures:
        return 1
    print("the checker agrees with GitHub on both files")
    return 0


def main() -> int:
    if len(sys.argv) > 1 and sys.argv[1] == "--self-test":
        return self_test()

    if len(sys.argv) > 1:
        targets = [Path(argument) for argument in sys.argv[1:]]
        files: list[Path] = []
        for target in targets:
            if target.is_dir():
                files += sorted(target.glob("*.yml")) + sorted(target.glob("*.yaml"))
            else:
                files.append(target)
    else:
        if not WORKFLOW_DIR.is_dir():
            print(f"no workflow directory at {WORKFLOW_DIR}")
            return 1
        files = sorted(WORKFLOW_DIR.glob("*.yml")) + sorted(WORKFLOW_DIR.glob("*.yaml"))

    if not files:
        print("no workflow files")
        return 1

    problems: list[str] = []
    for path in files:
        check_file(path, problems)

    if problems:
        print(f"{len(problems)} problem(s) in the workflow files:\n")
        for problem in problems:
            print(f"  {problem}")
        return 1

    print(f"{len(files)} workflow file(s) are sound: {', '.join(p.name for p in files)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
