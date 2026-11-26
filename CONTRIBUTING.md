# Contributing

Thanks for taking an interest. Bug reports, fixes, and ideas are all welcome.
Taking part means following the [Code of Conduct](CODE_OF_CONDUCT.md), which is
short and amounts to "be decent to people".

This is firmware for a specific board, so one thing is worth saying up front:
**you do not need the hardware to contribute.** The DAP is developed and
validated host-side through `dap-sim` and on-target fitness tests, which is the
whole point of the split in [README.md](README.md#workspaces).

## Before you start

For anything larger than a bug fix, open an issue first and say what you have in
mind. It is better to hear "we already tried that, here is why it did not work"
before you write the code than after.

Some things are deliberately out of scope rather than unimplemented. Classic
Bluetooth A2DP is the main one, and the README says why.

## Getting set up

`dap/` path-depends on the sibling [soul-audio-core](https://github.com/soulaudio/soul-audio-core)
repository, checked out next to this one:

```sh
git clone https://github.com/soulaudio/soul-listener
git clone https://github.com/soulaudio/soul-audio-core   # sibling, not nested
```

Then the three workspaces, per [README.md](README.md#quick-start):

```sh
cd dap      && cargo test                       # dap-core / dap-sim / arch-tests / eink-ssd1677
cd eink-ui  && cargo test --features std        # the no_std UI toolkit
cargo xtask verify                              # everything, the way CI runs it
```

Cross-building the board binary additionally needs the embedded target:

```sh
rustup target add thumbv7em-none-eabihf
cd dap && cargo build -p board-h7a3 --target thumbv7em-none-eabihf --features hardware
```

Git hooks are opt-in per clone and run the same gates: `lefthook install`, after
which `pre-commit` runs `cargo xtask precommit` and `pre-push` runs
`cargo xtask verify`.

## The rules the codebase keeps

These are enforced by CI, so it is cheaper to know them up front. Several are
unusual, because the target has 1 MB of SRAM and no operating system.

- **No `unsafe`, and the absence is itself gated.** Every crate on the embedded
  path declares `#![forbid(unsafe_code)]`, and `tools/check-forbid-unsafe.sh`
  asserts each one still has it. A rule that only lives in a doc drifts the first
  time somebody adds a crate or softens a `forbid` to `deny` in a refactor.
- **The stack budget is a hard number.** `tools/mcu-stackprobe` links
  soul-audio-core's real open-and-decode call chain into a bare-metal
  thumbv7em ELF and reads `.stack_sizes`, failing CI if either the entry frame or
  the largest frame on the path grows past its measured baseline. A decoder that
  is fine on a desktop and ruinous on a Cortex-M7 fails here rather than on a
  board.
- **The audio path does not allocate.** mcu-stackprobe declares no
  `#[global_allocator]` on purpose, so the instant anything on that path pulls in
  `alloc` the probe fails to *link*, before the stack assertions even run.
- **Architecture boundaries are tests.** `dap-core` stays board-agnostic, and
  `eink-ui` stays a standalone toolkit; both boundaries have their own CI job, as
  does the H7A3 memory map (`dap/arch-tests`).
- **`todo!()` and `unimplemented!()` are denied** as workspace lints, and a job
  verifies the lints are still declared so that the clippy run remains effective.
- **Clippy is clean** (`-D warnings`), **`cargo fmt` is law**, every crate opts
  into the workspace lint table, and `Cargo.lock` stays committed and out of
  `.gitignore`, because a firmware build that is not reproducible is not a
  firmware build.

## The soul-audio-core pin

CI builds the DAP against the exact soul-audio-core commit in
`.github/soul-audio-core.pin`, not against its `main`. Without that, an unrelated
merge in the other repository would change what your PR means depending on when
it ran.

The pin going stale is the other failure mode, so a job reports when it is behind
and never blocks on it. Bumping is deliberate:
`tools/soul-audio-core-pin.sh --latest`, then re-verify the `dap/` build in the
same PR.

## Pull requests

- Branch off `main`. Keep your branch up to date with it; CI must pass on the
  merge result, not just on your branch.
- Commit messages and PR titles follow
  [Conventional Commits](https://www.conventionalcommits.org) (`fix(dap): ...`,
  `feat(eink-ui): ...`). `cargo xtask check-commit-msg` enforces it locally and
  CI enforces it again.
- Keep a PR to one change. Two unrelated fixes are two PRs.
- Say what you did and why. If it is a bug fix, say how to reproduce the bug.
- Run `cargo xtask verify` first. It mirrors the CI gate, so a green local run is
  a good sign, though CI also runs the QEMU and cross-build legs.
- One check, `CI success`, is required to merge. It aggregates every other job,
  so if you add a CI job, add it to that job's `needs` in the same commit.

## Reporting bugs

The useful details are: which workspace, whether it reproduces in `dap-sim` or
only on hardware, the board revision and toolchain, the audio file format
involved, and what you expected instead. A failing test beats a description.

For security issues, do not open a public issue: see [SECURITY.md](SECURITY.md).

## Licensing

The project is AGPL-3.0. By contributing you agree your contribution ships under
that license.
