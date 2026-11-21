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

A second, independent target also falls out: **`App::render` costs 114 KiB of
stack**, and unlike the decoder work it lives in `eink-ui`/`dap-core` — *not*
behind `.github/soul-audio-core.pin*. It may well be the easier of the two.

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
