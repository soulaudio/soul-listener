# Picking a component, and getting it into firmware and the board

How a proposal becomes a decision, and how that decision reaches the two things
that have to agree with it.

## 1. The pipeline proposes

```sh
tools/sourcing-run.py refresh                          # fetch, export, snapshot
cargo run -p dap-arch-tests --example propose           # -> PROPOSALS.md + proposals.json
cargo run -p dap-arch-tests --example sourcing-report   # diagnostics: what the filter did
```

`hardware/bom/PROPOSALS.md` is the shortlist: per requirement, every part that
passes that requirement's catalogue-checkable constraints, ranked by the spec's
own `DEFAULT_RANKING` — EU source depth first, then what it actually costs to buy
the build quantity — with a clickable distributor link per row.

**Ranking is the shortlist, not the decision.** Every requirement also carries
`Evidence::Datasheet` constraints that no distributor publishes: dynamic range,
phase noise, break-before-make, QDID inheritance, UN 38.3 reports, insertion
cycles. A part that ranks first can still be the wrong choice because of one of
them, and `PROPOSALS.md` lists which ones are still outstanding for each
requirement.

Two sockets found this way illustrate the split. `2908-05WB-MG` ranks well on
sourcing — two distributors, 53 000 in stock — and is wrong on function: eight
contacts and no ninth, so no card-detect switch. `105027-0001` ranks second on
raw stock and is unbuyable for a prototype at MOQ 2000. Neither is visible from
a price and a stock figure alone.

## 2. You pick

A pick is **one edit**: set `Preference::Preferred` on the candidate in
`dap/hw-spec/src/v2.rs`.

There is deliberately no second place to update. The requirement describes what
the board needs; the candidates are parts that might satisfy it; the preference
says which one is used today. That split is what makes the lock soft — the rate
matrix depends on "a 22.5792 MHz oscillator", not on one manufacturer's part
number.

If the part is new, add a `Candidate` with a note saying why, and add it to
`hardware/bom/snapshot.json` so the fetcher prices it. `every_viable_candidate_is_in_the_snapshot`
will tell you if you forget.

### What checks the pick

Run `cargo xtask verify`. These are the ones that bear on a swap:

| gate | what it refuses |
|---|---|
| `every_picked_part_qualifies_against_its_own_requirement` | a part that fails the constraints of the requirement it was picked for |
| `preferred_parts_have_more_than_one_eu_source` | a single-sourced pick, unless accepted in `SINGLE_SOURCE_ACCEPTED` with a reason |
| `user_swappable_requirements_span_two_manufacturers` | a user-replaceable part with one maker, unless accepted in `SINGLE_MANUFACTURER_ACCEPTED` |
| `no_preferred_part_is_end_of_life` | a pick a vendor has marked obsolete |
| `no_spec_candidate_depends_on_a_conflicted_attribute` | a pick that qualifies only because two vendors disagree and the tie-break happened to favour one |
| `every_viable_candidate_is_in_the_snapshot` | a pick nobody has priced |

The first is the one that makes switching cheap: change the MPN, run the tests,
and either it qualifies or you are told which constraint it fails **and why that
constraint exists**. The alternative — discovering it from a fabricated board —
is the thing this is all for.

## 3. The decision is published

`propose` writes `hardware/bom/selected.json`: one entry per requirement, with
the MPN, manufacturer, datasheet and distributor links. Generated from
`Preference::Preferred` — never edited by hand.

It is deliberately smaller than `snapshot.json`. The snapshot is the sourcing
audit trail and records every offer from every vendor for every candidate;
`selected.json` answers one question — *which part is this requirement built
with* — for consumers that have no business reading price breaks.

## 4. Firmware

The seam exists and is assertion-based. `dap/arch-tests/tests/hw_spec_bridge.rs`
pins the firmware's own constants against the spec: the battery envelope, the
peripheral currents, the MCU's maximum clock, the linker script's regions, and
that `board-h7a3` targets the spec's MCU.

So switching the MCU means editing `MCU` in `v2.rs`, and the bridge test fails
until the board crate's `embassy_feature` and `memory.x` agree. The spec is the
source; the firmware is checked against it.

What is **not** generated: there is no codegen emitting Rust constants from the
spec. That is a deliberate stopping point — the assertions are few, they read
clearly, and a generator would be more machinery than the problem has earned.
The bridge file's own header says each assertion should eventually be deletable
once its subject is derived rather than duplicated.

## 5. The board (atopile)

**This seam does not exist yet, and that is the honest state.**

`hardware/elec/src` is still V1 — `ES9038Q2M`, external SDRAM, no MCLK mux — and
its `.ato` files are hand-written with no link to the spec. The V2 migration
(`docs/plans/2026-11-15-v2-migration-design.md`) has not happened.

What the sourcing side has now done is make that migration a transcription job
rather than a research one: `selected.json` names the part per requirement with
its datasheet, and `PROPOSALS.md` holds the alternatives that were considered.
`interfaces.ato` already carries an `SDMMCBus` and `mcu.ato` an `mcu.sdcard`
port, so the microSD socket — which until now was the one component the user
physically touches most and the only one with no part behind it — has three
sourced, two-distributor candidates waiting for it.

When the migration happens, the generator that writes `.ato` part stubs should
read `selected.json` and nothing else, so that a component swap stays one edit
in the spec.

## Why it is "quite easy to switch", concretely

1. One edit: the `Preference` in `v2.rs`.
2. `every_picked_part_qualifies_against_its_own_requirement` immediately checks
   the new part against the requirement, with the constraint's reason in the
   failure message.
3. The AVL, EOL, manufacturer-independence and conflict gates re-run against
   real distributor data.
4. `propose` regenerates the proposals and `selected.json`.
5. The firmware bridge fails if a board constant still describes the old part.

What is *not* easy, and should not be hidden: a swap that changes a footprint or
a pin count is a board change, and no amount of pipeline makes that cheap.
`STM32H7A3IIT6Q` is recorded as `Evaluating` rather than `Alternate` for exactly
that reason — it is the same die with the same memory in a 176-pin package
instead of 144, and well stocked where the 144-pin part is not, but taking it
means re-routing.
