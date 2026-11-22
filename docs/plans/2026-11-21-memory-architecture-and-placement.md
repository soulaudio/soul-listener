# Memory architecture & placement — what the H7A3 actually has, where things should go, and in what order

Web-researched 2026-11-21 against ST's own documentation and against how other
DAPs and MCU audio platforms allocate memory (sources at the end), then checked
against this repo's measured numbers. Written because the question "is there
enough RAM for audio + UI + Bluetooth?" turned out to depend on four wrong
numbers and two unused memory regions rather than on capacity.

**Headline:** there is enough RAM — ~180 KiB of genuine headroom — but only if
two regions that are currently invisible to the linker get used, and the honest
blocker is not capacity. It is a **415 KiB stack transient** that is larger than
any other single consumer and has never been attacked. §8 benchmarks it and
localises it: five of seven formats open in 69–81 KiB, and essentially all of the
excess is the **Matroska container path** (301 KiB) plus **`App::render`**
(114 KiB).

## 1. What the chip actually has

| block | size | address | domain | DMA |
|---|---|---|---|---|
| AXI-SRAM (1+2+3: 256+384+384 KB) | **1024 KB** | `0x24000000`, contiguous | CD | ✅ |
| AHB-SRAM1 | 64 KB | `0x30000000` | CD | ✅ |
| AHB-SRAM2 | 64 KB | `0x30020000` | CD | ✅ |
| SRD-SRAM | 32 KB | SRD | **SRD** | ✅ |
| DTCM | 128 KB | `0x20000000` | — | ❌ |
| ITCM | 64 KB | `0x00000000` | — | ❌ |

Cross-check: the datasheet's "1.4 MB RAM" headline reconciles as 1024 + 128 + 32
+ 128 + 64 + 4 (backup) ≈ 1380 KB.

### Corrections this round

- **`memory_map::USER_SRAM` was 1208 KB, described as "AHB SRAM1/2/3".** There is
  no AHB-SRAM3 on this part; AHB total is 128 KB, not 184. Corrected to
  **1152 KB** (CD-domain, DMA-capable). `memory_budget_fits_user_sram` had been
  checking the region plan against 56 KB of RAM that does not exist.
- **SRD-SRAM is now a separate constant**, deliberately excluded from
  `USER_SRAM`: it is DMA-reachable but sits in a different power domain with its
  own retention and gating behaviour. Counting it as general-purpose RAM is how a
  buffer ends up unexpectedly un-retained across a pause.
- **The stack reserve was 256 KiB in two gates while a third enforces 480 KiB.**
  The isolated decode-open transient is ~300 KiB and the whole firmware tick is
  ~415 KiB — both *above* the reserve. (Documented elsewhere as 297/413; measured
  2026-11-21 at **301/415** against the sibling soul-audio-core. The few-KiB drift
  is the sibling-versus-pin difference, not noise in the method — the probe
  validates at 0 B after painting.) The AXI check reported ~345 KB "to spare"
  that the real stack would have run into. Corrected to **480 KiB** in both
  `dap-arch-tests` and `board-h7a3`, and the report label is now derived from the
  constant rather than restating it, since a hardcoded "256 KiB" is how the stale
  figure survived.

**AHB-SRAM1 and AHB-SRAM2 are address-contiguous** (`0x30000000` + `0x20000`),
so a single ~94 KiB framebuffer does fit across them — with a caveat in §5.

## 2. What is measured today

| item | size | source |
|---|---|---|
| `App` (decoder 58 + ring/resampler 116) | 175 KiB | `size_of`, `dap-arch-tests` |
| Shared decoder scratch | 256 KiB | `decoder_scratch_len()` |
| Worst-case stack — decode-open **and** `App::render` | **415 KiB** | QEMU stack-painting, `check-tick-qemu.sh` |
| **Total, linked AXI SRAM** | **844 of 1024 KiB** | |

Note the stack figure already includes UI rendering: the whole-tick probe runs
`App::tick → pump → poll_open` *and* `App::render` (eink-ui layout + widget draw)
in one tick, which is why it is 415 KiB rather than the 301 KiB of decode alone.
There is no separate UI stack cost waiting to be discovered — and §8 quantifies
it at **114 KiB**, making it a target in its own right.

Still forecast, never implemented: framebuffer 94 · eink-ui scratch 32 · library
listing 64 · catalog index 32 · DMA staging 64 = **286 KiB**.

**Do not add those two columns and call the result a margin.** 844 KiB is
measured; 286 KiB is a plan, and plans are wrong in both directions. The honest
statement is "844 of 1024 linked today, with 286 KiB of forecast ahead".

## 3. How comparable players allocate memory

| player | silicon | RAM | strategy |
|---|---|---|---|
| Rockbox / iPod-class | HDD | 32–64 MB | `audiobuf` 5679 KiB + `codecbuf` 1024 KiB; buffer *minutes* of compressed audio so the disk can sleep |
| Rockbox / Sansa Clip | flash | ~2 MB | ≥125 kB compressed buffer is the proven MP3 floor; codec handle 50 kB; PCM chunk cut to 16 kB |
| Tangara | ESP32 | 4 MB PSRAM | streams from SD; explicitly never holds a whole track |
| Daisy Seed | **STM32H750** | 128 KB DTCM + 512 KB AXI + **64 MB SDRAM** | pre-allocated arenas, no heap; DTCM for small hot data; SDRAM for anything large |
| Teensy 4.x | i.MXRT M7 | **1 MB, no external RAM** | fixed pool of 128-sample blocks; **32-byte-aligned DMA buffers, sizes rounded to 32 B** |
| **soul-listener** | **H7A3** | **1152 KB, no external RAM** | one resident decoder + output ring + shared scratch |

Four conclusions:

1. **We are the most constrained player here.** Teensy 4 is the only true peer on
   "no external RAM", and it is a synth platform, not a file-decoding player.
   Daisy — on our exact MCU family — solves this with 64 MB of SDRAM.
2. **Nobody holds tracks in RAM, and our architecture already matches.** Single
   resident decoder, stream from SD, gapless from the output ring. No change needed.
3. **Our scratch is comfortably above the proven floor.** Rockbox needs ≥125 kB on
   a 2 MB target; we have 256 KiB scratch + 64 KiB ring. Useful independent
   evidence that backlog #1's 256→192 KiB shrink is safe.
4. **The Rockbox "big buffer" doctrine does not transfer.** It exists to let a
   *motor* stop. Our storage is microSD (20–100 mA read, 250 µA sleep, no
   spin-up); a bigger ring changes burst frequency, not bytes read, so the saving
   is bounded and small. Rockbox's own manual says a large anti-skip reserve
   *costs* battery. `AUDIO_PATH_BUDGET`'s "209 KB spare is the battery-ring
   tunable" is HDD reasoning inherited into a flash design.

## 4. Two project-level deviations from industry standard

**Caches are off, and not by decision.** The Cortex-M7 resets with I-cache and
D-cache disabled; `cortex-m-rt` does not enable them, and `embassy-stm32` 0.6
contains **no cache handling at all** (verified by exhaustive grep). ST's figure:
D-cache disabled ≈ **halves** CPU performance, and I-cache "can be enabled
systematically". Industry M7 practice is caches on plus MPU-marked non-cacheable
DMA regions.

This is defensible as a deliberate choice — deterministic timing, zero coherency
hazards, simpler code, and some real-time shops do exactly this — but it is
currently an accident of a dependency default. It also matters to the power work:
the whole chain is RTF → minimum clock → current → hours, and if ST's ~2× figure
holds, L11's conclusion that opus needs VOS2 and VOS3 is unreachable may be an
artifact of running uncached. **Measuring cached vs uncached RTF is the largest
unexplored battery lever in the project**, because the model takes RTF as input.

**No MPU configuration and no DMA alignment discipline.** v1 had both — a 32-byte
DMA alignment lint and an MPU non-cacheable `.axisram` section, citing AN4839.
The v2 `dap/` stack has neither. Harmless while caches are off; a
silent-audio-corruption class of bug the moment anyone enables them for
performance. Teensy independently requires the same 32-byte alignment, so this is
cross-vendor standard practice, not an ST quirk.

**Rule: caches on *with* MPU non-cacheable DMA regions *and* 32-byte alignment,
or caches off and documented. Never partially.**

## 5. Antipatterns found while drafting this plan

Recorded because most were in the first draft of the plan itself.

1. **Filling DTCM with UI buffers forecloses the better fix.** The first draft put
   eink-ui scratch (32) + library listing (64) + catalog index (32) = exactly
   128 KiB into DTCM *and* proposed shrinking the stack. Those conflict: ST's
   guidance is that the **stack** has first claim on DTCM, so if the stack work
   lands, DTCM is the place it should go — and it would already be full.
   *Resolved by §8's measurement: even a best-case whole tick is ~195 KiB, so
   DTCM-for-stack was never achievable and the conflict dissolves. Worth keeping
   as a record of why measuring first matters — the plan had a self-contradiction
   that only a number could settle.*
2. **"It fits exactly" is a smell, not a result.** Three unrelated budgets summing
   to precisely the region size is budget-fitting dressed as design, with no
   margin for one byte of growth.
3. **Mixing measurements with forecasts and quoting the sum as a margin** — see §2.
4. **Shrinking a budget to its measurement** is the same antipattern as a budget
   that mirrors its value, inverted. `AUDIO_PATH_BUDGET` should come down *some*,
   with a stated reason, not to the 175 KiB measurement — Phase E still has to add
   a real `Sink`, and now a BT sink variant.
5. **Spanning one object across two independently-switchable SRAMs.** A 94 KiB
   framebuffer across AHB-SRAM1+2 defeats `AHBRAM1SO`/`AHBRAM2SO` per-instance
   power-down — half the reason for choosing AHB. Split it, keep it in one
   instance, or accept the coupling knowingly.
6. **"ITCM for hot code" with no measurement.** ITCM costs a linker section plus a
   boot-time copy from flash, and may be strictly worse than enabling I-cache —
   one register write. Measure before reaching for it.
7. **Optimizing placement before the dominant consumer is understood.** 415 KiB
   stack > 256 KiB scratch > 175 KiB `App`. Three loops of work went into the
   scratch; none into the stack beyond measuring it.

## 6. Bluetooth (v2, secondary output)

v2 adds Bluetooth **as an addition**, with I²S remaining the primary path. That
choice is the right one and it is nearly free in RAM.

| | **I²S-input A2DP module** (chosen) | LE Audio / LC3 over HCI |
|---|---|---|
| Audio out of the H7A3 | **I²S, same as the DAC** | compressed frames over UART/HCI |
| Who encodes | the module (SBC/AAC turnkey firmware) | WB55's M4, or the H7A3 |
| Extra RAM on H7A3 | **~0 for audio**; ~2–8 KiB for pairing UI / device list | **60–90 KiB**: HCI/ACL buffers, LC3 state, ~38 KiB air-retransmission jitter buffer |
| Headphone compatibility | **universal** | LE Audio headsets only |
| Power | ~16–28 mA module | 22–38 % lower than A2DP |

Note v1 explicitly deferred this: *"Classic BT (A2DP) is out of scope… the WB55
handles BLE control (v1) and BLE Audio streaming (v2)"*. If v2 means playing to
**ordinary** headphones, that decision is superseded — LE Audio alone will not
connect to most of them.

### Consequences, in order of how easy they are to miss

**It attacks L5's native-rate finding.** L5's best power win is that the CS43131's
dual MCLK oscillator takes both rate families, so standard content builds **no
resampler**. A Bluetooth link negotiates *one* rate for the connection and cannot
be re-clocked per track. So 44.1k content to a 48k link forces the resampler —
moving it from rare fallback to common case, on the path that
`clock_headroom_pipeline_resampled_only_just_fits_vos2` shows clears VOS2's
ceiling by **~2 MHz**. The gate already exists; it is now guarding something
common rather than rare. L5's claim is scoped to jack output in `power::model`.

**Runtime roughly halves.** Published comparison of one device: 147.8 mW over
Bluetooth (22.5 h) vs 65.7 mW wired (50.7 h). At 3.7 V that is ~40 mA vs ~18 mA,
and our modeled ~21 mA wired figure sits next to the latter. Applied to the
1100 mAh pack: **~38–67 h wired → roughly ~19–33 h over Bluetooth.**

**Do not share one SAI between the DAC and the BT module if pins allow
otherwise.** v1 used that pattern for the ES9038/TAS5805 pair and left it as an
*open hardware decision*. Two real risks: audio isolation depends on a software
mute (a failed or un-ramped mute leaks to the wrong output), and switching output
requires re-clocking the SAI *and* flipping the MCLK oscillator, which means an
audible gap or pop unless carefully sequenced. **Dedicate a second SAI to
Bluetooth.** It removes a defect class for a few pins — and the user has
confirmed hardware can still change.

**Two gates will need rework** — recorded in `power::peripherals`: a sibling
`PlaybackBt` state (not nested), and `every_billed_output_path_device_...`
becoming per-state. Nothing was added speculatively: the module is not chosen, so
its control bus has no RCC coordinate yet, and inventing one would defeat the
point of requiring a real one.

## 7. The plan, in order

**Phase 0 — done in this change.** Free, no behaviour change, and it stops the
budget from reading as comfortable when it is not: `USER_SRAM` corrected, `SRD_SRAM`
separated, stack reserve corrected in both gates, L5 scoped to jack, BT gaps
recorded where the code lives.

**Phase 1 — reduce the open transient. This is the real work**, and it is now
measured rather than hypothesised. See §8.

**Phase 2 — place, with the stack's real size known.** DTCM to the **stack** if
Phase 1 lands; UI buffers get what is left. Framebuffer within a single AHB-SRAM
instance (or split deliberately). AXI keeps scratch + `App` + any remaining stack.

**Phase 3 — the cache decision, as an all-or-nothing bundle** (§4).

**Phase 4 — Bluetooth.** Dedicated SAI; sibling `PlaybackBt` state; a measured
`BT_PLAYBACK_MA`; second runtime figure.

Deciding Phase 2 before Phase 1 is deciding with the biggest variable unknown —
how much of the 415 KiB is recoverable changes whether AHB-SRAM is needed at all.

## 8. Phase 1, measured (2026-11-21)

Benchmarked on QEMU `mps2-an500` (Cortex-M7) against the **sibling**
soul-audio-core, plus `-Z emit-stack-sizes` for the static per-frame breakdown.

### Dynamic: only the two Matroska-container paths are pathological

| open path | transient |
|---|---|
| **Matroska-FLAC** | **301 KiB** |
| **Opus-in-WebM** | **231 KiB** |
| WAV | 81 KiB |
| WavPack | 71 KiB |
| ALAC-CAF | 69 KiB |

Whole firmware tick (open + `App::render` in one tick): **415 KiB**. That
decomposes as **301 KiB** of Matroska open plus **114 KiB** of `App::tick`
wrapper and `App::render`.

So this is not "the stack is too big" in general — five of seven formats open in
**69–81 KiB**, which is unremarkable. It is specifically the Matroska container
path, and secondarily the UI render.

### Static: three nested *construction* frames account for it

| frame | own size |
|---|---|
| `MatroskaDecoder::new_in` | 124,560 B |
| `FlacDecoder<MatroskaFlacReader<..>>::new_in` | 111,064 B |
| `MatroskaFlacReader::new` | 48,312 B |
| `open_seekable` | 61,056 B |

(`mcu_stackprobe::__cortex_m_rt_main` is 179,096 B but is the probe's own frame,
not on the board's path — see `check-stack.sh`.)

124 + 111 + 48 = **283 KiB of nested construction frames** against a 301 KiB
measured transient. **The static and dynamic measurements agree**, which is what
makes the diagnosis trustworthy rather than a story.

### The mechanism, and why the obvious fix is unavailable

The pattern is visible directly in `MatroskaOpus::new_in`:

```rust
Ok(Self { demux, core: OpusFrameDecoder::new(channels), …,
          packet: [0; MAX_OPUS_PACKET], scratch })
```

Large inline arrays inside a struct returned by value from a fallible
constructor. Each nesting level pays a full copy, because `Result<Self, E>`
routinely defeats RVO — exactly the reasoning `board-h7a3` already cites for
using `StaticCell::init_with` instead of `init`.

The usual fix — construct into a caller-provided `&mut MaybeUninit<Self>` —
**is not available here**: soul-audio-core is `#![forbid(unsafe_code)]`, and
there is no allocator. So the tractable fix is the one the project already
started: **move the remaining inline arrays into the shared `P` scratch via
`ScratchCursor`**, which shrinks the struct and the construction frames
together, with no `unsafe`.

### The prize, and a correction to this doc's earlier target

This converges with L15's separate finding that **Matroska has the largest
*inline* footprint (59,760 B) and sets `size_of::<AnyDecoder>()`**. The same
buffers cause both. One change fixes both: `AnyDecoder` would drop to MP3's
44,240 B *and* the open transient would fall toward the other formats' ~80 KiB.

Bringing the two Matroska paths in line with the rest takes the whole tick from
**415 KiB to roughly 195–230 KiB — a saving of ~185–220 KiB**, larger than DTCM
and AHB-SRAM combined (192 KiB). That is the single biggest memory lever in the
project.

**But the earlier "target under 128 KiB so the stack can live in DTCM" was
wrong, and the measurement is what shows it.** Even the cheapest format's whole
tick would be ~81 KiB open + 114 KiB render ≈ **195 KiB** — still well over
DTCM's 128 KB. **DTCM-for-stack is off the table** regardless of how well Phase 1
goes. The stack stays in AXI SRAM (where flip-link guards it), and DTCM is
genuinely available for the UI working set after all — which makes §7's Phase 2
simpler, not harder.

### The other 114 KiB is NOT render — and it is not fixable in-repo

A first pass attributed the whole tick's 114 KiB over the isolated decode path to
`App::render`, and guessed it would be the easier target because `eink-ui` is not
behind the pin. **Both halves were wrong**, and the frame table settles it:

- **All** eink-ui / screens / `embedded_graphics` frames together come to
  **2,584 B**, with `App::render` itself at **664 B**. Immediate-mode rendering
  through a borrowed `DrawTarget` is essentially free on the stack. There is no
  UI stack problem.
- The 114 KiB is **`dap_core::audio::AudioPlayer::pump`'s own frame: 119,416 B**
  — almost exactly two `size_of::<AnyDecoder>()` (59,760 B). One slot is
  `open_seekable`'s by-value return; the other is the decoder bound before it
  moves into the engine (`Engine::provide` is inlined, so it has no frame).

It is in `dap-core`, so it *looked* in-repo. It isn't fixable there. Three
source-level variants were implemented and measured:

| variant | `pump` frame |
|---|---|
| baseline | 119,416 B |
| drop the named local, pass the call result straight to `provide` | 119,416 B |
| `#[inline(never)]` on the `provide` hand-off | 119,424 B |
| outline the entire loop body | 119,416 B (frame moves to the helper, still live during the decode — **peak unchanged**) |

The two slots are inherent to receiving a ~58 KiB value by value and moving it
onward; the receiving slot is necessarily live *during* the call that fills it.
The fix is an **in-place construction API upstream** — an
`open_seekable_into(&mut Option<AnyDecoder>)`, or
`provide_with(track, impl FnOnce() -> Result<D, E>)` on the engine — which lives
in `soul-audio-core`/`soul-audio-engine`, behind the pin. `AudioPlayer::pump`
now carries this as a doc comment so it isn't re-attempted.

### The unifying result: every frame is a multiple of `size_of::<AnyDecoder>()`

Dividing each measured frame by the 59,760 B decoder type:

| frame | size | ÷ `AnyDecoder` |
|---|---|---|
| `AudioPlayer::pump` | 119,416 B | **2.00×** |
| `MatroskaDecoder::new_in` | 124,560 B | **2.08×** |
| `FlacDecoder<MatroskaFlacReader<..>>::new_in` | 111,064 B | **1.86×** |
| `open_seekable` | 61,352 B | **1.03×** |
| `MatroskaFlacReader::new` | 48,304 B | **0.81×** |
| sum | 464,696 B | **7.78×** |

The entire stack transient is **~7.8 copies of one 58 KiB type**. That is the
whole problem, stated in one line.

### Correction: the "in-place construction API" fix is not possible

An earlier draft of this section proposed an upstream
`open_seekable_into(&mut Option<AnyDecoder>)` or
`provide_with(track, impl FnOnce() -> …)` to eliminate the copies. **Rust cannot
express that safely.** There is no stable placement-new: `MaybeUninit` requires
`unsafe` (forbidden crate-wide), `Box::new_in` requires an allocator (none), and
`Option::get_or_insert_with` still takes its closure's return *by value*. Worse,
the closure-based variant doesn't even help in principle — the decode happens
*inside* the closure, so the closure's return slot is live during the decode,
which is exactly today's situation.

The copies are irreducible. **But they are all proportional**, which means the
lever is not the call shape — it is the *type size*:

> `7.78 × 59,760 B = 465 KiB`. `7.78 × 12,000 B ≈ 93 KiB`.

So the upstream work is not a new API. It is **finishing the externalization the
project already started**: move the remaining inline arrays out of the decoder
structs into the shared `P` scratch via `ScratchCursor`, exactly as WavPack
already does (96 B inline, versus Matroska's 59,760 B).

**This makes it one change with three payoffs**, which L15 and this section
arrived at independently:

1. `size_of::<AnyDecoder>()` drops — L15's finding, the static footprint.
2. The three nested Matroska construction frames shrink proportionally.
3. `AudioPlayer::pump`'s double slot shrinks proportionally — no `dap-core`
   change needed at all, which is why none of the three local variants moved it.

Scope guidance: FLAC already sits at 12,472 B inline and WavPack at 96 B, so
~12 KiB is a demonstrated target rather than an aspiration. Getting the worst
variant there would take the whole tick from **415 KiB to roughly 95–120 KiB** —
and at that point the stack *would* fit DTCM after all, reopening the option §8
earlier closed. Worth re-checking once the real number is known rather than
planning on it.

**Consequence for sequencing:** there is no easier in-repo alternative, and no
API design to agree first. It is one coherent upstream change — shrink the
decoder structs — behind one pin bump.

## 9. First upstream slice, done and measured (2026-11-22)

soul-audio-core PR [#111](https://github.com/soulaudio/soul-audio-core/pull/111), branch
`perf/matroska-codec-private-embedded`.

`MatroskaDemuxer` kept its `CodecPrivate` in a **16 KiB inline array**, sized
entirely for Vorbis's three setup headers. Everything else needs orders of
magnitude less (`OpusHead` 19 B; AAC's `AudioSpecificConfig` a handful; FLAC's is
independently capped at 1 KiB by `MatroskaFlacReader`). Under `embedded-profile`
it is now **8 KiB**; desktop builds are untouched.

The cap is derived, not chosen: `testdata/tone-vorbis.mka` carries a **4,286 B**
`CodecPrivate`, so a first attempt at 2 KiB regressed
`decode::open_decodes_matroska_vorbis_matches_ogg` — caught by the suite, then
settled by reading the fixture's actual EBML element. 8 KiB clears it with ~1.9x
headroom and declines anything larger as `Unsupported`, the same tradeoff class
as `MAX_FLAC_FRAME`'s 48→28 KiB cap.

| | before | after | Δ |
|---|---|---|---|
| `size_of::<AnyDecoder>()` | 59,680 B | **51,488 B** | −8,192 |
| open — matroska-flac | 301 KiB | **250 KiB** | −51 |
| open — opus-in-webm | 231 KiB | **182 KiB** | −49 |
| open — wav | 81 | **70** | −11 |
| open — wavpack | 71 | **54** | −17 |
| open — alac-caf | 69 | **52** | −17 |
| **whole tick** | **415 KiB** | **356 KiB** | **−59** |

**The proportionality model is confirmed.** 8 KiB off the type bought 59 KiB of
stack — a **7.2x multiplier** against the predicted ~7.8x. And every format
improved, not just Matroska, because the shrink lands on the shared enum rather
than one container path. §8's claim that "the lever is the type, not the API" is
no longer a hypothesis.

Upstream verification: `cargo xtask verify` green on soul-audio-core's **pinned
1.93.1** (`rust-toolchain.toml`) — fmt, clippy `-D warnings`, **3,143 tests**,
`no_std` thumbv7em cross-builds, including the `--features embedded-profile` test
step that exercises the change.

### Sequencing — the ratchets must NOT be tightened yet

> **Done — see §13.** All four steps below were executed on 2026-11-22: #111,
> #112 and #113 merged, the pin bumped to `c0c35c38`, and the budgets ratcheted
> to 288/416 KiB against CI's post-bump measurement. The constraint this
> section describes is exactly why the bump and the ratchet were separate
> commits.

soul-listener's CI builds the **pinned** soul-audio-core, which does not contain
this change, so the current ratchet budgets (`DYNAMIC_STACK_BUDGET_KIB` 384,
`DYNAMIC_TICK_BUDGET_KIB` 480) must stay as they are. Tightening them now would
make CI fail against the pin. The order is:

1. merge soul-audio-core [#111](https://github.com/soulaudio/soul-audio-core/pull/111),
2. bump `.github/soul-audio-core.pin` (`tools/soul-audio-core-pin.sh`),
3. **then** ratchet the two budgets down to lock the win in,
4. and re-check `any_decoder_variant_inline_footprint_roster` — Matroska at
   51,488 B still binds (MP3 is 44,240 B), so the roster's expectation holds, but
   the per-variant ceiling has ~14 KiB more headroom than it did.

## 10. What is left, and why it now needs a decision (2026-11-22)

Re-measured after §9, because the composition changed. Per-variant, with the
Matroska-FLAC path fully decomposed:

| variant | size | | field | bytes |
|---|---|---|---|---|
| **MatroskaDecoder** (binder) | **51,488** | | `MatroskaFlacReader.frame` | **28,672** |
| Mp3Decoder | 44,176 | | `FlacDemuxer.seek_points` | **8,192** |
| MatroskaOpus | 41,832 | | `MatroskaDemuxer` (post-§9) | 9,360 |
| OpusDecoder | 41,000 | | `FlacDecoder.bytes` | 4,096 |
| AlacDecoder | 25,832 | | `MatroskaFlacReader.codec_private` | 1,024 |

### The staircase, and the floor

`size_of::<AnyDecoder>()` is `max(variants)`, so reductions only pay out until
the next variant takes over:

- Matroska 51,488 → below **44,176** ⇒ MP3 binds. Gain **7,312 B**.
- Then MP3 44,176 → below **41,832** ⇒ MatroskaOpus binds. Gain 2,344 B.
- Then the **Opus cluster at ~41,000** — CELT + SILK state, algorithmically
  required.

**So the whole remaining prize is 51,488 → ~41,000 = −10,488 B ≈ −73 KiB of
stack, and it takes every step below to collect it.** §9's no-tradeoff win was
the cheap one; what is left is not cheap.

### Each remaining lever, and what it costs

| lever | saving | the catch |
|---|---|---|
| `MAX_FLAC_FRAME` 28 → 20 KiB under embedded-profile | 8,192 B — enough on its own | **Format support.** 28 KiB was chosen to hold a worst-case 4096-block 24-bit stereo *verbatim* FLAC frame (4096 × 2 × 3 = 24,576 B). 20 KiB covers 16-bit but rejects that. Same class as the dropped surround decoders — a product call. |
| `MAX_SEEK_POINTS` 512 → fewer | up to 7,680 B | **Not graceful as written.** The fill is `if seek_count < MAX_SEEK_POINTS { … }` — it keeps the *first* N and drops the rest, so a smaller table indexes only the start of a track and seeks beyond it decode-discard forward from there. Fixing it properly means *subsampling* (the SEEKTABLE length gives the point count upfront, so a stride is computable before the loop) — better coverage at ¼ the memory, but real decoder-logic work in soul-audio-core. |
| Externalize `MatroskaFlacReader.frame` (28,672 B) | 7,312 B (capped by MP3) | **Phase A3's byte-arena seam.** The shared scratch is `[i32]`, this buffer is `[u8]`, and `#![forbid(unsafe_code)]` rules out reinterpreting one as the other. Needs either a second owned `B: AsMut<[u8]>` parameter on `open`/`open_seekable` (API change rippling to every caller and repo) or streaming the SimpleBlock instead of buffering it (demuxer-interface change across every codec path). Note the 28 KiB buffer only buys 7,312 B — the rest is absorbed by MP3. |
| Thread the scratch into `Mp3Decoder` (`.pcm` 9,216 B inline) | 9,216 B off MP3 | **Premature.** MP3 is not the binder, so this pays nothing until Matroska is below it. Worth noting that MP3 is the *only* compressed format that takes no `P` — every other codec already externalizes its PCM carry buffer. |

### Recommendation, and what was then taken

The no-downside option on that list — the `MAX_SEEK_POINTS` subsampling fix —
**was done**: soul-audio-core PR
[#113](https://github.com/soulaudio/soul-audio-core/pull/113). See §11.

What remains after it all carries a cost: a format-support decision
(`MAX_FLAC_FRAME`), a cross-repo API change (the byte arena), or work that pays
nothing yet (`Mp3Decoder`). **Recommend stopping there** unless one of those
tradeoffs is actively wanted — the stack now fits with room to spare. (This
section predicted 344 KiB against the 480 KiB ratchet. CI measured **308 KiB**
once the pin was bumped, now gated at **416 KiB** — sized above a local run's
393 KiB rather than CI's figure, because that probe does not reproduce across
environments; see §13.)

## 11. Second upstream slice — seek-table subsampling (2026-11-22)

soul-audio-core PR [#113](https://github.com/soulaudio/soul-audio-core/pull/113),
branch `perf/flac-seektable-subsample`. Independent of #111 in content, but
*stacked* on [#112](https://github.com/soulaudio/soul-audio-core/pull/112) — a
pre-existing `soul-audio-organize` shared-temp-dir test race that blocked its
pre-push gate twice and turned out to be a real bug worth its own fix (see
below).

`FlacDemuxer` kept the **first** N seek points and dropped the rest, while the
constant's doc claimed that meant "seeking stays correct, just coarser". It did
not: past the cap everything was **unindexed**, so a seek into the tail of a long
stream decode-discarded forward from the truncation point. It now retains an
evenly-spaced subset spanning the whole table (the point count is known upfront
from the SEEKTABLE block length, so a Bresenham selection keeps exactly
`min(points, cap)`), and the cap drops 512 → 128 for **−6,144 B**.

**Web research is what made 128 defensible rather than arbitrary.** The reference
encoder's default is `-S 10s` — one point per ten seconds — so:

| stream length | before | after |
|---|---|---|
| ≤ ~21 min (every ordinary track) | all points | **identical** |
| 21–85 min | all points | coarser, full-span |
| > ~85 min | **tail unindexed** | **full-span** |

The common case is untouched; the case that changes most is the one that was
broken. (Rockbox caps at 5000 — sensible with megabytes of RAM, where this array
was 8,192 B of a 59,680 B `AnyDecoder`.)

### Combined effect of #111 + #112

The two cut different buffers, so they are additive (8,192 + 6,144):

| | baseline | both |
|---|---|---|
| `size_of::<AnyDecoder>()` | 59,680 B | **45,344 B** (−24%) |
| open — matroska-flac | 301 KiB | **220 KiB** |
| open — opus-in-webm | 231 KiB | **170 KiB** |
| open — wav / wavpack / alac | 81 / 71 / 69 | **64 / 48 / 46** |
| **whole tick** | **415 KiB** | **344 KiB** (−71) |

### A pre-existing test race, found by being blocked by it

Pushing #113 failed its pre-push gate twice on `soul-audio-organize` tests with
no relation to FLAC. Rather than bypass the gate, the mechanism turned out to be
findable: `plan::tests` and `apply::tests` each declare their own
`static N: AtomicU32` but build temp paths as `organize-test-{pid}-{tag}-{n}` in
the same process and temp directory — and **both use the tags `src-move` and
`dest-move`**, so the independent counters can mint the identical path. One test
then populates the other's destination and the planner correctly reports
`SkipDuplicate` where `Move` is expected.

Deterministic repro (filtering to the two tests makes both counters start at
zero): `cargo test -p soul-audio-organize --lib move_` fails 4/4 on main and
passes 4/4 with the fix. Under the full suite it is intermittent, which is why it
reads as noise — `xtask verify` fails on one or other of those two tests roughly
one run in several, unrelated to whatever is being changed. Fixed in
[#112](https://github.com/soulaudio/soul-audio-core/pull/112) by putting
`module_path!()` in the directory name.

Worth recording as a method note: "pre-existing flake, CI is the arbiter" was the
right call on *attribution* but too passive as a stopping point. The mechanism
cost ten minutes to find and converted a shrug into a two-line fix with a
deterministic repro.

### The multiplier decays — a correction to §8's model

§8 measured the transient as ~7.8 *copies* of `AnyDecoder` and predicted
reductions would scale by that factor. #111's 8,192 B did buy 59 KiB (~7×). This
second 6,144 B adds only a further **12 KiB (~2×)**. Fewer frames are
simultaneously live at the peak as the type shrinks, so the proportionality holds
in *direction* but not as a constant — which is what approaching a floor looks
like. The floor is the Opus cluster (~41 KB of CELT + SILK state): with both
changes in, Matroska is 45,344 B and MP3 44,176 B, so the next reduction of any
kind hands the binding constraint to MP3 and this staircase is done.

### Scope note
The decoder work is a `soul-audio-core` change, so it lands behind the pin and
needs that repo's full decode matrix green. It is deliberately **not** bundled
with this analysis. `check-stack.sh` now prints the frame table on every run
(`STACK_FRAME_TABLE_TOP`, default 12), so each iteration can be verified against
the same breakdown that produced this section.

## Sources

ST: [AN4891 system architecture & performance](https://www.st.com/resource/en/application_note/an4891-stm32h72x-stm32h73x-and-singlecore-stm32h74x75x-system-architecture-and-performance-stmicroelectronics.pdf) ·
[AN4839 L1 cache on F7/H7](https://www.st.com/resource/en/application_note/an4839-level-1-cache-on-stm32f7-series-and-stm32h7-series-stmicroelectronics.pdf) ·
[STM32H7A3xI/G datasheet](https://www.st.com/resource/en/datasheet/stm32h7a3ai.pdf) ·
[AN5450 H7A3 smart power management](https://www.st.com/resource/en/application_note/an5450-stm32h7a37b3-lines-and-stm32h7b0-value-line-smart-power-management-expansion-package-for-stm32cube-stmicroelectronics.pdf) ·
[AN5307 H7A3 hardware development](https://www.st.com/resource/en/application_note/an5307-getting-started-with-stm32h7a37b3-line-and-stm32h7b0-value-line-microcontroller-hardware-development-stmicroelectronics.pdf) ·
[AN5293 F7/H743 → H7A3 migration](https://www.st.com/resource/en/application_note/an5293-migration-guide-from-stm32f7-series-and-stm32h743753-line-to-stm32h7a37b3-and-stm32h7b0-value-line-devices-stmicroelectronics.pdf) ·
[STM32H7 system overview (training)](https://www.st.com/content/ccc/resource/training/technical/product_training/group0/8b/11/92/3f/1c/8d/4e/4c/STM32H7-System-Overview_OVR/files/STM32H7-System-Overview_OVR.pdf/_jcr_content/translations/en.STM32H7-System-Overview_OVR.pdf) ·
[ST community: DMA not working on STM32H7](https://community.st.com/t5/stm32-mcus/dma-is-not-working-on-stm32h7-devices/ta-p/49498)

Comparables: [Rockbox AllocatingRAM](https://www.rockbox.org/wiki/AllocatingRAM) ·
[Rockbox manual, anti-skip buffer](https://download.rockbox.org/manual/rockbox-ipodvideo/rockbox-buildch7.html) ·
[Rockbox FS#10605 (Clip low-memory floors)](https://www.rockbox.org/tracker/task/10605) ·
[Tangara (CNX Software)](https://www.cnx-software.com/2024/02/06/tangara-portable-open-source-esp32-music-player/) ·
[Daisy: Memory — what is the difference](https://daisy.audio/pages/memory-what-is-the-difference) ·
[Teensy 4 DMA/OCRAM alignment (PJRC)](https://forum.pjrc.com/threads/58234?p=220645&viewfull=1)

Bluetooth: [Microchip BM83 datasheet](https://ww1.microchip.com/downloads/en/DeviceDoc/70005402C.pdf) ·
[IS2083/BM83 applications design guide](https://ww1.microchip.com/downloads/aemDocuments/documents/WSG/ApplicationNotes/ApplicationNotes/AN3118-IS2083-BM83-Bluetooth-Applications-Design-Guide-DS00003118.pdf) ·
[EE Times: streaming audio, battery life and music quality](https://www.eetimes.com/streaming-audio-how-to-optimize-battery-life-and-music-quality/) ·
[Silicon Labs: developing Bluetooth audio devices](https://www.silabs.com/documents/public/presentations/Developing-Bluetooth-Audio.pdf)

## 12. Pin bumped to the AAC-gated rev (2026-11-22)

`main` moved while this analysis was in flight:
[#41](https://github.com/soulaudio/soul-listener/pull/41) bumped
`.github/soul-audio-core.pin` `8d8e4e0` → `9cf69fd` and **dropped AAC from the
DAP firmware** — upstream now patent-gates it behind an `aac` cargo feature which
this firmware does not enable.

That broke §8's per-variant roster, which imported `formats::aac::AacDecoder`:
the module does not exist in a build without the feature, so CI failed with
`unresolved import soul_audio_core::formats::aac`. Caught by CI testing the
*merge* of this branch with the moved `main`, not by any local run — the local
sibling checkout still had AAC ungated. A good argument for reading the merge
result rather than the branch.

**And the reason matters more than the build break.** The gate is a *licensing*
one: AAC is a Via LA patent ($15,000 one-time + $0.98 per unit), so
`dap-arch-tests`' new `aac_license.rs` asserts `!soul_audio_core::AAC_ENABLED` —
the patented decoder must never be linked into a shipped device. The roster was
importing that decoder into exactly that firmware. Removing it is therefore not
merely a compile fix; a per-variant footprint roster has no business naming a
decoder the product is legally required not to ship, and the roster would have
quietly kept it alive in the dependency graph.

AAC removed from the roster. Re-measured against the **new pin**:

| variant | inline bytes |
|---|---|
| **Matroska** | **59,760** (binder, unchanged) |
| MP3 | 44,240 |
| Opus | 41,104 |
| ALAC | 25,896 |
| CAF | 25,784 |
| WAV | 18,584 |
| FLAC | 12,472 |
| AIFF | 104 |
| WavPack | 112 |

Dropping AAC does not move the binding constraint — AAC was 25,816 B, far below
Matroska — so every conclusion in §8–§11 survives. Clippy and the full dap suite
(140 tests) are clean against the new pin.

**One thing to keep straight.** §9/§11's figures (`AnyDecoder` 51,488 / 45,344 B;
tick 356 / 344 KiB) are *with soul-audio-core PRs #111 and #113 applied*. The
pin does not contain them, so the DAP's shipping baseline today is the 59,760 B
above and a 415 KiB tick. Those wins land in the firmware only when the pin is
bumped again — step 2 of §9's sequencing, now one bump further along than when
it was written.

## 13. Pin bumped, and the ratchets closed on CI's numbers (2026-11-22)

§9 step 2 and step 3, done — as **two** commits, deliberately.

soul-audio-core #112, #111 and #113 are merged (`4d18d18`, `440a2bd`,
`c0c35c38`) and `.github/soul-audio-core.pin` now names `c0c35c38`, so the DAP
finally builds the sliced decoders instead of forecasting them.

### What CI measured, before and after

| probe | pin `9cf69fdb` | pin `c0c35c38` | delta |
|---|---|---|---|
| open — matroska-flac (worst) | 297 KiB | **219 KiB** | −78 |
| open — opus-in-webm | 229 | **171** | −58 |
| open — wav / wavpack / alac | 78 / 69 / 67 | **64 / 49 / 47** | −14 / −20 / −20 |
| **whole firmware tick** | **414 KiB** | **308 KiB** | **−106** |

New budgets:

| gate | was | now | CI | local | basis |
|---|---|---|---|---|---|
| `DYNAMIC_STACK_BUDGET_KIB` | 384 | **288** | 219 | 222 | ratcheted to the measurement (+31%) |
| `DYNAMIC_TICK_BUDGET_KIB` | 480 | **416** | 308 | **393** | sized above the *higher* reading |
| `BOARD_STACK_RESERVE` / `STACK_RESERVE_BYTES` | 480 KiB | **416 KiB** | 308 | 393 | lockstep with the tick budget |

### The tick probe does not reproduce across environments — so it is not ratcheted to CI

This is the finding that changed the plan, and it was nearly missed.

At **byte-identical content** (pin `c0c35c38`, sibling checkout at that exact
rev) the two environments disagree:

| probe | CI | local (NixOS, rustc 1.95.0) | spread |
|---|---|---|---|
| worst open transient | 219 KiB | 222 KiB | **1.4%** |
| whole firmware tick | 308 KiB | **393 KiB** | **27.6%** |

The first draft of this section set `DYNAMIC_TICK_BUDGET_KIB` to 360 — derived
from CI's 308 while holding the old budget's ~16% margin. That would have been a
gate that is **green in CI and red on a contributor's machine** (393 > 360): the
exact trap the ratchet exists to avoid, arrived at by careful reasoning from one
environment's numbers. 416 KiB clears the higher reading with ~6% to spare and
still locks in 64 KiB of the win.

The isolated decode probe *is* reproducible, so it IS ratcheted to its
measurement. Treat the two probes differently until the spread is explained.

**Open item (do not tighten the tick budget before closing it):** find the
85 KiB. Both runs validate at `after_paint=0`, so neither measurement is
unsound. The suspicious detail is `probe-total(ctx)`: CI reports 308 KiB —
exactly equal to its tick figure — while locally it is 537 KiB against a
393 KiB tick. That smells like a difference in how much caller context is live
at the paint point (inlining of `App::tick`/`render` under a different rustc),
not a difference in the firmware's real stack need. Until that is pinned down,
CI's 308 KiB is **not** established as the on-target figure.

### §11's combined-effect table was too pessimistic about the tick

§11 predicted the two slices would take the tick 415 → 344 KiB (−71). CI
measured 308 (−106). The open-path predictions were accurate (220 vs 219
actual), so the error is specific to the whole-tick figure: §11 derived it by
adding the isolated decode saving to a fixed tick overhead, but the wrapper
frames the tick adds (`App::tick` → `AudioPlayer::pump` → `Engine::poll_open`)
hold decoder-sized locals of their own, so they shrink with the type too. The
saving compounds through the wrapper chain instead of sitting on top of it.
Corrected above; §11's table left as written with this pointer, since the
reasoning error is the useful part.

### Why the bump and the ratchet are separate commits

A local run of these probes does not reproduce CI's numbers. On identical
content this machine read **435 KiB** where CI read **414** on the tick probe
(~5%), while agreeing within ~1% on the isolated decode probe (300 vs 297) —
and the per-format drift runs in *both* directions (local `wavpack` 65 vs CI 69,
local `wav` 80 vs CI 78), so it is environment variance, not an offset that
could be corrected for. The two environments also disagreed about the *size* of
the saving (local −42 KiB on the tick, CI −106).

Setting a blocking gate from the local figure would have put a budget in the
repo that CI cannot reproduce — the same class of defect as §2's four
"numbers that looked enforced and weren't". So the bump landed alone and CI
measured the post-bump transient on its own runner.

The first version of this rule read "the ratchet is only ever set from a CI run
of the probe, never a local one." That is **wrong as stated**, and the 27%
post-bump spread above is why: a budget set from CI alone fails on a
contributor's machine. The rule that survives contact with the measurements is:

> Ratchet to the measurement only where the probe is reproducible. Where it is
> not, size the budget above the highest reading any supported environment
> produces, and treat the spread itself as the bug to fix before tightening
> further.

### One consequence to keep in view

Lowering `STACK_RESERVE_BYTES` 480 → 416 KiB *frees* 64 KiB of AXI-SRAM for
statics — it loosens the board's compile-time assert rather than tightening it.
That is the wanted direction (Phase E needs that room for a framebuffer and DMA
rings), but it inverts the usual ratchet intuition, so the constant must only
ever be lowered against a fresh measurement, never to make a static fit.

It also means the 592 KiB "board headroom" figure quoted throughout this doc and
the reserve are two different things: the headroom is what is left after the
statics, the reserve is what we *promise* to leave. Only the reserve moved.

## 14. The 85 KiB spread was the compiler — and the gate was never reproducible (2026-11-22)

§13 closed with an open item: explain the 85 KiB gap between CI's 308 KiB tick
and a local 393 KiB at byte-identical content, and do not tighten the tick
budget until it is explained. It is explained.

**It was the rustc version, and nothing else.** CI's dynamic job ran
`dtolnay/rust-toolchain@stable`, which resolved to **1.100.0** (2026-11-09).
The local default was **1.95.0** (2026-04-14). Re-running the probe locally
under 1.100.0 reproduces CI *exactly* — 308 KiB, 316,056 B to the byte, with
every per-format value identical (`wav=64 mka-flac=219 wavpack=49
opus-webm=171 alac-caf=47`) and the same `probe-total(ctx)`.

| rustc | whole tick | worst open |
|---|---|---|
| 1.95.0 (2026-04-14) | **393 KiB** | 222 KiB |
| 1.100.0 (2026-11-09) | **308 KiB** | 219 KiB |

So the probe is not noisy — it is perfectly deterministic *given a compiler*.
Seven months of inlining and frame-layout changes moved the whole-tick figure
27%. The isolated decode probe barely moved (222 → 219), which is why the two
disagreed: the whole-tick probe measures through the `App::tick` → `pump` →
`poll_open` wrapper chain, and whether those caller frames survive or get
inlined away is exactly what changed.

### The actual defect was in the gate, not the measurement

A gated number computed by a **floating** toolchain is not a gate. Before this,
a new stable release could move `DYNAMIC_TICK_BUDGET_KIB`'s subject under the
repo with no source change — in either direction: silently granting slack, or
failing CI for a reason that has nothing to do with the firmware. And a
contributor running the script locally measured whatever their default
toolchain happened to be, which is how the 393 KiB reading appeared in the
first place and nearly got written into the repo as the budget.

The repo's own policy already said not to do this. `ci.yml`'s header: *"pinned
to a specific stable release for reproducible builds"*. Every MSRV job pins
`@1.87`. The **one job whose output is a number other gates enforce** was the
one left on `@stable`.

Fixed in two places:

- **`tools/mcu-stackprobe/rust-toolchain.toml`** (new) pins `1.100.0` +
  `thumbv7em-none-eabihf`. Both probe scripts `cd` into that directory before
  building, so this governs CI *and* a local run — the measurement is now the
  same number everywhere. It is scoped to that workspace deliberately: a
  repo-root pin would override the `@1.87` MSRV jobs and silently stop them
  checking the MSRV, and it does not touch the static `-Z emit-stack-sizes`
  gate, which selects `+nightly` explicitly.
- **`ci.yml`**'s dynamic job pins `@1.100.0` to match, so the action does not
  download a second toolchain.

### Which let the budgets close properly

With compiler drift removed, the margins only have to cover real code growth:

| gate | §13 | now | measured | margin |
|---|---|---|---|---|
| `DYNAMIC_STACK_BUDGET_KIB` | 288 | **256** | 219 | ~17% |
| `DYNAMIC_TICK_BUDGET_KIB` | 416 | **352** | 308 | ~14% |
| `BOARD_STACK_RESERVE` / `STACK_RESERVE_BYTES` | 416 KiB | **352 KiB** | 308 | lockstep |

Net against where this roadmap started: the tick budget went **480 → 352 KiB**
and the open-transient budget **384 → 256 KiB**, both now sitting on a
reproducible measurement rather than a forecast. The reserve dropping to
352 KiB frees **128 KiB** of AXI-SRAM for Phase E's statics.

### The method note worth keeping

§13 wrote a rule, then had to retract it one commit later, then it was the
retraction that turned out to be incomplete too:

1. *"Set the ratchet only from CI."* Wrong — fails on a contributor's machine.
2. *"Size it above the highest reading any environment produces."* Safe, but it
   treats irreproducibility as a fact of life and pays for it in permanent slack
   (416 instead of 352).
3. **What was actually required: make the measurement reproducible, then ratchet
   to it.** An irreproducible measurement is a bug in the gate, not a tolerance
   to budget around.

The intermediate step was not wasted — shipping 416 was the right call while the
spread was unexplained, because the alternative was a gate that is green in CI
and red locally. But "I cannot reproduce this number" should have read as *fix
the harness*, not *widen the budget*, and it took measuring the same source
under two compilers to see it.
