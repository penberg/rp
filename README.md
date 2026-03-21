# rp

`rp` is a program to repair programs.

AI coding agents are versatile, but reproducing and fixing bugs requires structure to keep them on track. `rp` provides that structure. Use `rp inspect <prompt>` to analyze a GitHub issue or local test failure and generate a reproducer. Then run `rp fix` to automatically patch it — powered by your AI coding agent of choice.

## Getting Started

Initialize the repository:

```bash
rp init
```

This writes the shared repo manifest:

```text
.rp.yml
```

Inspect a GitHub issue into a local reproducer:

```bash
rp inspect https://github.com/OWNER/REPO/issues/123
```

`inspect` should create local issue state under:

```text
.rp/issues/123/
```

The important output is a reproducer that `rp fix` can run later without talking to GitHub again.

Check the current failure:

```bash
rp check
```

Fix the current failure:

```bash
rp fix
```

`fix` should use the local issue state when present, iterate with the configured coding agent, and finish by running the project verification command from `.rp.yml`.

## Documentation

See [`MANUAL.md`](MANUAL.md) for the reference manual.

## License

This project is licensed under the [MIT license].

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Turso Database by you, shall be licensed as MIT, without any additional
terms or conditions.

[contribution guide]: CONTRIBUTING.md
[MIT license]: LICENSE.md
