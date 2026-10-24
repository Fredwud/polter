# Contributing to Polter

Thank you for wanting to help. Polter is made to be driven by AI agents as much as by people, and contributions from both are welcome; the rules are the same. Polter is developed and maintained by Claude, an AI agent working in Claude Code, for the repository's owner, who has the last word. Claude reviews pull requests and answers issues, signed "— Claude". The work happens in sessions, so an answer can take some days.

## Before you start

- Read [API.md](API.md) (the whole API, short enough to read in full) and [SPEC.md](SPEC.md) (what Polter is meant to become, and why).
- If you are an agent: [llms.txt](llms.txt) is your first page.
- For anything bigger than a fix, open an issue first and say what you want to change.
- A bug report is best when it can be replayed: the command, its seed and a `.haunt` file (`play --record FILE` writes one while you play).

## Building and checking

You need stable Rust. On Linux, also the development packages for ALSA and udev (`libasound2-dev libudev-dev pkg-config` on Debian and Ubuntu).

```sh
cargo build --release -p polter-dev        # the polter program, in target/release/polter
tools/check.sh                              # what every change must pass
tools/check.sh --quick                      # the same without the golden frames and a fresh game
tools/check.sh --changed                    # while working: only the packages a change touches
```

`tools/check.sh` runs the tests, clippy with warnings as errors, rustfmt, the golden frames, and a game fresh from `polter new` through clippy as it is made and as it ships. `--changed` checks what is not committed yet (or what changed since a revision: `--changed REV`): the tests and clippy of the packages it touches and everything built on them, and rustfmt. Each step reports on its own; a step that fails shows its last lines and where its log is. CI runs the tests, clippy, rustfmt and the golden frames on Windows, Linux and macOS for every push and pull request.

## The rules

- **Determinism is the promise.** The same build, seed and input give the same state and the same pixels, on every machine. In game logic: no wall clock, no `HashMap` iteration, randomness only from `ctx.rng`, and `polter::math` instead of the platform's `sin`, `exp` or `powf` (`clippy.toml` names all of these). The window's pacing and a shipped game's seed are the two allowed exceptions, each marked where it is.
- **Every change comes with its test**, and a new check is broken once on purpose to see it fail.
- **Pictures are tests.** If a change moves a pixel on purpose, rewrite the golden run it moves (`python3 tests/golden/make.py write NAME`) and say in the commit why the picture changed. If it moves a pixel by accident, that is a bug the golden frames just found.
- **Numbers in the docs have a source**: a measurement with its date and machine, or a test.
- **English** everywhere in the code and the docs; comments say why, in plain sentences.
- **A new Rust file starts with the MPL header**, as the others do; `tests/docs.rs` checks every file in `src`, `tests`, `dev`, `carts/lantern` and `carts/testcard`.
- `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings` must be clean.

## Licence

Polter is licensed under the [Mozilla Public License 2.0](LICENSE). By contributing you agree that your contribution is licensed the same way. Games made with Polter can use any licence, closed and sold; files of the engine stay MPL-2.0.
