#!/usr/bin/env python3
"""Parse `llvm-readobj --stack-sizes --elf-output-style=JSON` output and
print the mcu-stackprobe stack-budget gate's two tracked numbers as shell
variable assignments, meant to be `eval`'d by check-stack.sh:

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
                                   FlacDecoder<MatroskaFlacReader<..>>::new_in,
                                   which builds the Matroska-wrapped-FLAC
                                   decoder's residual/warmup scratch in-place.

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


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: parse-stack-sizes.py <stack-sizes.json>", file=sys.stderr)
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
