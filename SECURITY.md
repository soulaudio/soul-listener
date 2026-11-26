# Security Policy

## Reporting a vulnerability

Please report security issues **privately**, not via public issues or PRs.

- Email: sebastian.stupak@pm.me
- Or open a GitHub [private vulnerability report](https://github.com/soulaudio/soul-listener/security/advisories/new).

Include a description, the affected revision, and reproduction steps. Say whether
it reproduces in `dap-sim` or needs the board. We aim to acknowledge within a few
days.

## Scope

This repository is firmware for a battery-powered device with no IP networking.
There is no server to attack and, as of today, no over-the-air update path: a
build reaches a board over SWD or USB, by someone holding it. That does not make
it input-free, though, and the radio and the USB port are both reachable by
someone standing nearby.

The interesting attack surface is therefore what the device parses from
untrusted input, and it is worth reporting:

- malformed audio files or metadata that crash, hang, or corrupt memory while
  decoding (the decoders live in
  [soul-audio-core](https://github.com/soulaudio/soul-audio-core), but a finding
  reached through this firmware belongs here)
- anything reachable over USB (UAC2), BLE/GATT, or the microSD filesystem
- a stack overflow on the audio path, which on a Cortex-M7 with no MMU is a
  memory-safety bug and not merely a crash

## What the build already enforces

These are CI gates rather than aspirations, and a report that one of them can be
bypassed is itself a valid security report:

- **No `unsafe` on the embedded path.** Every crate there declares
  `#![forbid(unsafe_code)]`, and `tools/check-forbid-unsafe.sh` fails CI if any
  crate loses the attribute.
- **A hard stack budget.** `tools/mcu-stackprobe` links the real decode call
  chain into a bare-metal ELF and asserts measured frame-size baselines, so
  unbounded recursion or a stack-hungry decoder fails before it ships.
- **No allocator on the audio path.** The probe declares no
  `#[global_allocator]`, so pulling in `alloc` there fails to link.
- **Dependency advisories.** `cargo audit` runs against the shipped lockfiles
  (`dap/` and `eink-ui/`); `cargo deny` gates licenses, bans, and sources. The
  audio dependencies are additionally audited in soul-audio-core's own CI.
- **Pinned dependency revision.** CI builds against the soul-audio-core commit in
  `.github/soul-audio-core.pin`, so a build is reproducible and a supply-chain
  change is a reviewed diff rather than a surprise.

## Known gaps

Stated rather than implied, since a security policy that only lists strengths is
not much of a policy:

- `cargo vet`'s trust annotations were configured for the old root workspace only
  and have not been ported to `dap/` and `eink-ui/`; `cargo audit` covers
  advisories meanwhile.
- The display path's no-alloc gate is narrower than the audio path's
  (`dap/sim/tests/alloc_policy.rs`).
- There is no signed-update mechanism, because there are no over-the-air updates
  to sign. If one is added, this section has to change with it.
