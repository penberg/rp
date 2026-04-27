# rp Manual

## Name

`rp` - repair programs

## Synopsis

```text
rp init
rp inspect [--verbose] <prompt>
rp explain [issue]
rp check
rp fix
rp config
rp config list
rp config get <key>
rp config set <key> <value>
```

## Description

`rp` is built around a local workflow:

1. `rp init`
   Create the shared repo manifest.
2. `rp inspect`
   Turn a bug report into a local reproducer under `.rp/issues/`.
3. `rp explain`
   Re-read the saved issue explanation in human terms.
4. `rp check`
   Run that reproducer and record the result.
5. `rp fix`
   Invoke the configured coding agent to change the repository until the reproducer stops failing.

The important split is:

- `.rp.yml` describes the repository contract
- `.rp/` holds local runtime state

## Files

### `.rp.yml`

Shared repository metadata created by `rp init`.

This file should be checked in. It tells `rp` how to verify the repository and where repo-native tests should live.

Minimal example:

```yaml
version: 1

verify-cmd: make test
```

Example with repo-native tests:

```yaml
version: 1

verify-cmd: make test

tests:
  - testing/conformance

guidance: |
  Prefer adding a minimal regression test under testing/conformance.
  Choose the most specific suite for the bug, and update the suite Makefile when needed.
```

The `testing/conformance` path above is only an example. The point is that `tests:` tells `rp fix` where durable regression tests are expected to be added.

### `.rp/config`

Local repository configuration.

This file stores local execution choices such as the coding agent backend.

Example:

```text
agent = codex
```

If `agent` is not set, `rp` resolves a default backend from `claude`, `codex`, and `opencode` in alphabetical order, using the first executable found on `PATH`.

### `.rp/issues/`

Local issue state created by `rp inspect`.

Each issue gets its own directory:

```text
.rp/issues/<issue-id>/
```

Typical contents:

- `SOURCE.txt`
  Original inspect input
- `SUMMARY.txt`
  Condensed issue summary
- `EXPLANATION.md`
  Saved human explanation of the issue
- `reproducer.sh`
  Local reproducer
- `status`
  Current issue-local state
- `check.stdout`
  Output from `rp check`
- `check.stderr`
  Error output from `rp check`
- `check.status`
  Machine-readable check result

## Config Keys

### `verify-cmd`

Single repository verification command.

This is the repo-owned final validation contract. `rp fix` should aim to make the active reproducer stop failing and then make this command pass.

Example:

```yaml
verify-cmd: make test
```

### `tests`

List of repository paths where durable regression tests should be added.

If `tests:` is configured, `rp fix` requires the fix to introduce or update repo-native tests under those roots.

Example:

```yaml
tests:
  - testing/conformance
```

### `guidance`

Optional multi-line text passed to the coding agent during `fix`.

Use this for repo-specific conventions that are too project-specific to infer reliably.

Example:

```yaml
guidance: |
  Prefer adding tests under testing/conformance/linux-x86 for Linux x86 guest bugs.
  Update the suite Makefile when introducing a new test file.
```

## Commands

### `rp init`

Initialize repository metadata.

Responsibilities:

- detect a default verification command
- emit `.rp.yml`
- include `tests:` guidance when common test roots are recognized

`init` only writes shared repo metadata. It does not write local agent selection.

### `rp inspect [--verbose] <prompt>`

Materialize a bug report into a local reproducer workspace.

`<prompt>` may be:

- a GitHub issue URL
- an issue number
- free-form text

Responsibilities:

- create `.rp/issues/<id>/`
- invoke the configured agent in non-interactive mode
- write a candidate reproducer and saved explanation

With `--verbose`, `inspect` streams backend activity in a readable form while the agent works.

### `rp explain`

Print the saved human explanation for the current issue. Pass an optional issue id to select a specific issue, for example `rp explain 9`.

Responsibilities:

- find the active issue under `.rp/issues/`, or use the explicitly selected issue
- read `SUMMARY.txt`
- read `EXPLANATION.md`

### `rp check`

Run the current issue reproducer. Pass an optional issue id to select a specific issue, for example `rp check 9`.

Responsibilities:

- find the active issue under `.rp/issues/`, or use the explicitly selected issue
- run `reproducer.sh`
- capture stdout, stderr, and exit code
- print a concise summary and show artifact locations
- print captured reproducer output inline when the reproducer is broken
- write `check.stdout`, `check.stderr`, and `check.status`
- update `status`

Current verdicts are:

- `reproduced`
  reproducer exited with code `1`
- `not_reproduced`
  reproducer exited with code `0`
- `broken_reproducer`
  reproducer exited with some other code or by signal

### `rp commit`

Commit the current tracked diff with an agent-written commit message.

Responsibilities:

- read the current diff against `HEAD`
- include the saved active issue summary and explanation when available
- invoke the configured agent in non-interactive mode
- run `git commit -am` with the generated message

`rp commit` also includes untracked files in the prompt context so the generated message can describe them, but like `git commit -a` it does not add untracked files to the commit.

### `rp fix`

Fix the current issue in the tree.

Responsibilities:

- find the active issue under `.rp/issues/`
- read the issue explanation and reproducer artifacts
- invoke the configured agent in non-interactive mode
- require a repo-native regression test when `tests:` is configured
- run the reproducer again through `rp check`

`fix` currently operates on the current source tree.

If `tests:` is configured, `fix` checks for new or modified files under those roots after the agent run. If there are no new test changes, `fix` fails even if the temporary reproducer passes.

### `rp config`

List effective local configuration values.

This behaves like `rp config list`.

### `rp config list`

List effective local configuration values.

Output format:

```text
key=value
```

### `rp config get <key>`

Print the effective value for a single local configuration key.

### `rp config set <key> <value>`

Write a local configuration value to `.rp/config`.

## Workflow

Typical workflow:

```bash
rp init
rp config set agent codex
rp inspect https://github.com/OWNER/REPO/issues/123
rp explain
rp check
rp fix
```

State flow:

```text
bug report -> .rp/issues/<id>/reproducer.sh -> rp check -> rp fix -> verify-cmd
```

If the repository defines `tests:`, the durable end state should include a repo-native regression test under one of those paths.
