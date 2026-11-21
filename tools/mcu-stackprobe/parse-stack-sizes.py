#!/usr/bin/env python3
"""Parse `llvm-readobj --stack-sizes --elf-output-style=JSON` output.

Two modes:

  (default)      print the gate's two tracked numbers as shell variable
                 assignments, meant to be `eval`'d by check-stack.sh.
  --table <N>    print the N largest frames as a human-readable table, for
                 DIAGNOSIS rather than gating. Added 2026-11-21: the two
                 gate numbers tell you *whether* the budget holds, never
                 *where* the stack goes, so every investigation re-derived
                 this by hand. The 2026-11-21 placement analysis needed it to
                 find that three nested Matroska *construction* frames
                 (`MatroskaDecoder::new_in` 124 KiB + `FlacDecoder<
                 MatroskaFlacReader<..>>::new_in` 111 KiB + `MatroskaFlacReader
                 ::new` 48 KiB) account for essentially all of the 301 KiB
                 Matroska-FLAC open transient, while every non-Matroska format
                 opens in 69-81 KiB. Printing it on every gate run means the
                 next person starts from the breakdown.

                 Symbols are Rust v0-mangled and printed raw: `llvm-readobj
                 --demangle` silently degenerates the `--elf-output-style=JSON`
                 document (verified: 183 bytes of nothing), so demangling here
                 would mean a new tool dependency in CI. They are legible as-is
                 (`..matroska15MatroskaDecoder..6new_in`); pipe through
                 `rustfilt` for the full form.

The gate's two tracked numbers:

  MAIN_FRAME / MAIN_NAME       -- the `__cortex_m_rt_main` entry frame: the
                                   open_seekable -> Deck::load -> Deck::pump
                                   open-path stack transient (the `Deck`
                                   itself lives in a StaticCell, off this
                                   frame -- see src/main.rs's DECK doc
                                   comment and commit d97c820).
  LARGEST_FRAME / LARGEST_NAME -- the single largest OTHER function frame
                                   on the path. main's own entry frame is
                                   excluded here since it is tracked
                                   separately above; today this is
                                   MatroskaDecoder<SliceSource,..>::new_in,
                                   which (now that src/main.rs actually opens
                                   and decodes a Matroska-wrapped-FLAC
                                   fixture, not just WAV) has the nested
                                   FlacDecoder<MatroskaFlacReader<..>,..>::new_in
                                   inlined into it -- see check-stack.sh's
                                   own comment for the 2026-11-18 re-measurement.

Not a Rust/xtask helper because this is pure text/JSON munging with no
interesting logic worth compiling -- see check-stack.sh for why a plain
script (not an xtask subcommand) was chosen for this gate: mcu-stackprobe is
a standalone single-crate `[workspace]` (own Cargo.lock, nightly-only `-Z`
flags) deliberately kept outside the root workspace xtask lives in, so
xtask can't depend on it without re-entangling the two.
"""

import json
import shlex
import sys


def load_entries(path: str) -> list[tuple[int, str]]:
    """`[(size, symbol), ...]`, descending by size."""
    with open(path, encoding="utf-8") as f:
        doc = json.load(f)
    out: list[tuple[int, str]] = []
    for section in doc:
        for item in section.get("StackSizes", []):
            entry = item["Entry"]
            names = entry.get("Functions") or []
            out.append((entry["Size"], names[0] if names else "<unknown>"))
    out.sort(reverse=True)
    return out


def print_table(path: str, top: int) -> int:
    entries = load_entries(path)
    if not entries:
        print("error: no .stack_sizes entries found", file=sys.stderr)
        return 1
    total = sum(size for size, _ in entries)
    shown = entries[:top]
    share = sum(size for size, _ in shown) / total * 100 if total else 0.0
    print(
        f"-- {len(entries)} frames, {total} B ({total // 1024} KiB) summed; "
        f"top {len(shown)} = {share:.0f}% of that --"
    )
    print("   (per-function OWN frame; the dynamic QEMU probes measure the")
    print("    nested total that actually has to fit. Sum != peak.)")
    for size, name in shown:
        print(f"   {size:>8} B  {name[:116]}")
    return 0


def main() -> int:
    if len(sys.argv) == 4 and sys.argv[1] == "--table":
        try:
            top = int(sys.argv[2])
        except ValueError:
            print("usage: parse-stack-sizes.py --table <N> <json>", file=sys.stderr)
            return 2
        return print_table(sys.argv[3], top)

    if len(sys.argv) != 2:
        print(
            "usage: parse-stack-sizes.py <stack-sizes.json>\n"
            "       parse-stack-sizes.py --table <N> <stack-sizes.json>",
            file=sys.stderr,
        )
        return 2

    with open(sys.argv[1], encoding="utf-8") as f:
        doc = json.load(f)

    entries = doc[0]["StackSizes"]

    main_size = None
    main_name = None
    largest_size = -1
    largest_name = None

    for item in entries:
        entry = item["Entry"]
        names = entry.get("Functions") or []
        name = names[0] if names else "<unknown>"
        size = entry["Size"]

        # cortex-m-rt's #[entry] macro emits the real user code under
        # `__cortex_m_rt_main`; the symbol literally named `main` is just its
        # tiny trampoline (disable interrupts, call __cortex_m_rt_main, loop).
        # Both are excluded from the "largest OTHER frame" search below.
        is_main_entry = "__cortex_m_rt_main" in name or name == "main"

        if "__cortex_m_rt_main" in name and (main_size is None or size > main_size):
            main_size, main_name = size, name
        if not is_main_entry and size > largest_size:
            largest_size, largest_name = size, name

    if main_size is None:
        print(
            "error: no __cortex_m_rt_main entry found in .stack_sizes "
            "(did the build actually run with -Z emit-stack-sizes?)",
            file=sys.stderr,
        )
        return 1

    print(f"MAIN_FRAME={main_size}")
    print(f"MAIN_NAME={shlex.quote(main_name)}")
    print(f"LARGEST_FRAME={largest_size}")
    print(f"LARGEST_NAME={shlex.quote(largest_name)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
