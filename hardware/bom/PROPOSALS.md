# Proposed components

**Generated** — do not edit by hand. Regenerate with
`cargo run -p dap-arch-tests --example propose`.

Every part below PASSES its requirement's catalogue-checkable constraints and is ranked by the spec's own `DEFAULT_RANKING`: EU source depth first, then what it actually costs to buy the build quantity (`max(moq, build_qty) x unit_price`), then lead time. Source depth leads deliberately — a cheaper part with one distributor is worse than a dearer part with two.

What this list does NOT decide: every requirement also carries `Evidence::Datasheet` constraints that no distributor publishes — dynamic range, phase noise, break-before-make, QDID inheritance, UN 38.3 reports. Those are read off a datasheet by a person, and a part ranking first here can still be the wrong choice because of one of them. Ranking is the shortlist, not the decision.

Stock and price were true when this was generated. Follow the distributor link in each row before ordering.

## `dac`

stereo DAC + headphone amp, I2S slave, both MCLK families, >=120 dB(A) DR

3 of 1860 swept candidates qualify. Currently preferred: `CS43131-CNZ`.

Still to confirm by hand for whichever part is chosen: `dynamic_range_db`, `integrated_headphone_amp`, `accepts_both_mclk_families`.

| # | MPN | manufacturer | EU src | stock | MOQ | EUR @1 | where |
|---|---|---|---:|---:|---:|---:|---|
| 1 | `CS43198-CNZ` | Cirrus Logic | 2 | 796 | 1 | 16.15 | [digikey](https://www.digikey.nl/en/products/detail/cirrus-logic-inc/CS43198-CNZ/7430361) · [mouser](https://cz.mouser.com/en/ProductDetail/Cirrus-Logic/CS43198-CNZ?qs=%252BEew9%252B0nqrBKvFG1qnP85Q%3D%3D) |
| 2 | `CS43131-CNZ` **<-- picked** | Cirrus Logic | 1 | 347 | 1 | 16.44 | [digikey](https://www.digikey.nl/en/products/detail/cirrus-logic-inc/CS43131-CNZ/7388594) · [mouser](https://cz.mouser.com/en/ProductDetail/Cirrus-Logic/CS43131-CNZ?qs=%252BEew9%252B0nqrDUhBYVE1mW3Q%3D%3D) |
| 3 | `CS43131-CNZR` | Cirrus Logic Inc. | 1 | 221 | 1 | 16.44 | [digikey](https://www.digikey.nl/en/products/detail/cirrus-logic-inc/CS43131-CNZR/7388593) |

## `mclk-osc-44k1`

22.5792 MHz fixed oscillator, operating range covering 3.3 V, low phase noise, SMD

179 of 371 swept candidates qualify. **Nothing is picked yet for this requirement.**

Still to confirm by hand for whichever part is chosen: `phase_noise_dbc_1khz`.

| # | MPN | manufacturer | EU src | stock | MOQ | EUR @1 | where |
|---|---|---|---:|---:|---:|---:|---|
| 1 | `ASV-22.5792MHZ-LR-T` | ABRACON | 2 | 18319 | 1 | 2.11 | [digikey](https://www.digikey.nl/en/products/detail/abracon-llc/ASV-22-5792MHZ-LR-T/15995980) · [mouser](https://cz.mouser.com/en/ProductDetail/ABRACON/ASV-22.5792MHZ-LR-T?qs=fnpEE5GccuOy%2FTECAxvETg%3D%3D) · [tme](https://www.tme.eu/en/details/asv22.5792mhzlrt-0/) |
| 2 | `CCHD-957-25-22.5792` | Crystek Corporation | 2 | 126 | 1 | 29.24 | [digikey](https://www.digikey.nl/en/products/detail/crystek-corporation/CCHD-957-25-22-5792/2742155) · [mouser](https://cz.mouser.com/en/ProductDetail/Crystek-Corporation/CCHD-957-25-22.5792?qs=0ahj2ebvT2%2FMyeJwq3kSsA%3D%3D) |
| 3 | `SX3M22.5792B20F30TNN` | Shenzhen SCTF Electronics | 1 | 72000 | 10 | 1.20 | [digikey](https://www.digikey.nl/en/products/detail/shenzhen-sctf-electronics/SX3M22-5792B20F30TNN/25700692) |
| 4 | `SX3M22.5792M20F30TNN` | Shenzhen SCTF Electronics | 1 | 72000 | 10 | 1.20 | [digikey](https://www.digikey.nl/en/products/detail/shenzhen-sctf-electronics/SX3M22-5792M20F30TNN/25572973) |
| 5 | `SX2M22.5792B20F30TNN` | Shenzhen SCTF Electronics | 1 | 72000 | 10 | 1.22 | [digikey](https://www.digikey.nl/en/products/detail/shenzhen-sctf-electronics/SX2M22-5792B20F30TNN/25699566) |
| 6 | `SX2M22.5792M20F30TNN` | Shenzhen SCTF Electronics | 1 | 72000 | 10 | 1.22 | [digikey](https://www.digikey.nl/en/products/detail/shenzhen-sctf-electronics/SX2M22-5792M20F30TNN/25572909) |
| 7 | `SX3M22.5792B10F20TNN` | Shenzhen SCTF Electronics | 1 | 72000 | 10 | 1.23 | [digikey](https://www.digikey.nl/en/products/detail/shenzhen-sctf-electronics/SX3M22-5792B10F20TNN/25573811) |
| 8 | `SX5M22.5792B20F30TNN` | Shenzhen SCTF Electronics | 1 | 72000 | 10 | 1.23 | [digikey](https://www.digikey.nl/en/products/detail/shenzhen-sctf-electronics/SX5M22-5792B20F30TNN/25699473) |
| 9 | `SX5M22.5792M20F30TNN` | Shenzhen SCTF Electronics | 1 | 72000 | 10 | 1.23 | [digikey](https://www.digikey.nl/en/products/detail/shenzhen-sctf-electronics/SX5M22-5792M20F30TNN/25572838) |
| 10 | `SX2M22.5792B10F20TNN` | Shenzhen SCTF Electronics | 1 | 72000 | 10 | 1.24 | [digikey](https://www.digikey.nl/en/products/detail/shenzhen-sctf-electronics/SX2M22-5792B10F20TNN/25699736) |

_169 further qualifying parts not shown; `sourcing-report` states the full count and what rejected the rest._

## `mclk-osc-48k`

24.576 MHz fixed oscillator, operating range covering 3.3 V, low phase noise, SMD

218 of 605 swept candidates qualify. **Nothing is picked yet for this requirement.**

Still to confirm by hand for whichever part is chosen: `phase_noise_dbc_1khz`.

| # | MPN | manufacturer | EU src | stock | MOQ | EUR @1 | where |
|---|---|---|---:|---:|---:|---:|---|
| 1 | `ASV-24.576MHZ-EJ-T` | ABRACON | 3 | 18797 | 1 | 2.00 | [digikey](https://www.digikey.nl/en/products/detail/abracon-llc/ASV-24-576MHZ-EJ-T/675513) · [mouser](https://cz.mouser.com/en/ProductDetail/ABRACON/ASV-24.576MHZ-EJ-T?qs=GgB7MJ%2FTIsRYewXBpNiXkA%3D%3D) · [tme](https://www.tme.eu/en/details/asv24.576mhzejt-0/) |
| 2 | `ASDDV-24.576MHZ-LC-T3` | ABRACON | 2 | 43557 | 1 | 1.37 | [digikey](https://www.digikey.nl/en/products/detail/abracon-llc/ASDDV-24-576MHZ-LC-T3/15771176) · [tme](https://www.tme.eu/en/details/asddv24.576mhzlc-0/) |
| 3 | `ASEM1-24.576MHZ-LC-T` | ABRACON | 2 | 21728 | 1 | 2.72 | [digikey](https://www.digikey.nl/en/products/detail/abracon-llc/ASEM1-24-576MHZ-LC-T/1873210) · [tme](https://www.tme.eu/en/details/asem124.576mhz-0/) |
| 4 | `ASFL1-24.576MHZ-EC-T` | ABRACON | 2 | 13310 | 1 | 1.94 | [digikey](https://www.digikey.nl/en/products/detail/abracon-llc/ASFL1-24-576MHZ-EC-T/687864) · [tme](https://www.tme.eu/en/details/asfl124.576mhzec-0/) |
| 5 | `ASEMB-24.576MHZ-LC-T` | ABRACON | 2 | 10837 | 1 | 2.82 | [digikey](https://www.digikey.nl/en/products/detail/abracon-llc/ASEMB-24-576MHZ-LC-T/2810001) · [tme](https://www.tme.eu/en/details/asemb24.576mhzlc-0/) |
| 6 | `ASFLMB-24.576MHZ-LR-T` | ABRACON | 2 | 4000 | 1000 | 1.40 | [digikey](https://www.digikey.nl/en/products/detail/abracon-llc/ASFLMB-24-576MHZ-LR-T/2624014) · [tme](https://www.tme.eu/en/details/asflmb24.576mhzl-0/) |
| 7 | `ASDMB-24.576MHZ-LY-T` ~~RULED OUT~~<br>_MEMS — see the 44.1 kHz family for the measured reasons (5-10 ps jitter, 0..+70 C, no supp…_ | ABRACON | 2 | 2967 | 1 | 3.92 | [digikey](https://www.digikey.nl/en/products/detail/abracon-llc/ASDMB-24-576MHZ-LY-T/2623831) · [mouser](https://cz.mouser.com/en/ProductDetail/ABRACON/ASDMB-24.576MHZ-LY-T?qs=CnD89xF1VSmmkEgUdf3hMQ%3D%3D) |
| 8 | `SX3M24.576B20F30TNN` | Shenzhen SCTF Electronics | 1 | 72000 | 10 | 1.20 | [digikey](https://www.digikey.nl/en/products/detail/shenzhen-sctf-electronics/SX3M24-576B20F30TNN/25700693) |
| 9 | `SX3M24.576M20F30TNN` | Shenzhen SCTF Electronics | 1 | 72000 | 10 | 1.20 | [digikey](https://www.digikey.nl/en/products/detail/shenzhen-sctf-electronics/SX3M24-576M20F30TNN/25573000) |
| 10 | `SX2M24.576B20F30TNN` | Shenzhen SCTF Electronics | 1 | 72000 | 10 | 1.22 | [digikey](https://www.digikey.nl/en/products/detail/shenzhen-sctf-electronics/SX2M24-576B20F30TNN/25699701) |

_208 further qualifying parts not shown; `sourcing-report` states the full count and what rejected the rest._

## `mclk-mux`

2:1 clock mux, break-before-make, <=50 MHz, single digital select

193 of 668 swept candidates qualify. Currently preferred: `SN74LVC1G3157DCKR`.

Still to confirm by hand for whichever part is chosen: `max_freq_hz`, `break_before_make`.

| # | MPN | manufacturer | EU src | stock | MOQ | EUR @1 | where |
|---|---|---|---:|---:|---:|---:|---|
| 1 | `SN74LVC1G3157DCKR` **<-- picked** | TEXAS INSTRUMENTS | 3 | 1435471 | 1 | 0.19 | [digikey](https://www.digikey.nl/en/products/detail/texas-instruments/SN74LVC1G3157DCKR/562895) · [mouser](https://cz.mouser.com/en/ProductDetail/Texas-Instruments/SN74LVC1G3157DCKR?qs=PVVDbbWpW3J21wbz0QWbxg%3D%3D) · [tme](https://www.tme.eu/en/details/sn74lvc1g3157dckr/) |
| 2 | `TS5A3157DBVR` | TEXAS INSTRUMENTS | 2 | 53370 | 1 | 0.26 | [digikey](https://www.digikey.nl/en/products/detail/texas-instruments/TS5A3157DBVR/705351) · [tme](https://www.tme.eu/en/details/ts5a3157dbvr/) |
| 3 | `TS5A3159ADBVR` | TEXAS INSTRUMENTS | 2 | 21678 | 1 | 0.65 | [digikey](https://www.digikey.nl/en/products/detail/texas-instruments/TS5A3159ADBVR/936395) · [tme](https://www.tme.eu/en/details/ts5a3159adbvr/) |
| 4 | `DG419DY-T1-E3` | VISHAY | 2 | 3447 | 1 | 1.38 | [digikey](https://www.digikey.nl/en/products/detail/vishay-siliconix/DG419DY-T1-E3/2621760) · [tme](https://www.tme.eu/en/details/dg419dy-t1-e3/) |
| 5 | `NLVASB3157DFT2G` | ONSEMI | 2 | 937 | 1 | 0.21 | [digikey](https://www.digikey.nl/en/products/detail/onsemi/NLVASB3157DFT2G/5257984) · [tme](https://www.tme.eu/en/details/nlvasb3157dft2g/) |
| 6 | `TS5A9411DCKR` | TEXAS INSTRUMENTS | 2 | 607 | 1 | 0.34 | [digikey](https://www.digikey.nl/en/products/detail/texas-instruments/TS5A9411DCKR/1739550) · [tme](https://www.tme.eu/en/details/ts5a9411dckr/) |
| 7 | `NLAS4599DFT2` | onsemi | 1 | 2349310 | 3000 | 0.26 | [digikey](https://www.digikey.nl/en/products/detail/onsemi/NLAS4599DFT2/609578) |
| 8 | `74LVC1G3157FZ4-7` | Diodes Incorporated | 1 | 180330 | 1 | 0.17 | [digikey](https://www.digikey.nl/en/products/detail/diodes-incorporated/74LVC1G3157FZ4-7/6003947) |
| 9 | `DG4157EDL-T1-GE3` | Vishay Siliconix | 1 | 161121 | 1 | 0.61 | [digikey](https://www.digikey.nl/en/products/detail/vishay-siliconix/DG4157EDL-T1-GE3/7422954) |
| 10 | `DG4157EDN-T1-GE4` | Vishay Siliconix | 1 | 122215 | 1 | 0.72 | [digikey](https://www.digikey.nl/en/products/detail/vishay-siliconix/DG4157EDN-T1-GE4/7422955) |

_183 further qualifying parts not shown; `sourcing-report` states the full count and what rejected the rest._

## `mcu`

Cortex-M7 >=250 MHz, >=1 MB contiguous DMA-capable SRAM, SAI, SDMMC, 2 MB flash

210 of 1646 swept candidates qualify. Currently preferred: `STM32H7A3ZIT6`.

Still to confirm by hand for whichever part is chosen: `sram_bytes`, `flash_bytes`.

| # | MPN | manufacturer | EU src | stock | MOQ | EUR @1 | where |
|---|---|---|---:|---:|---:|---:|---|
| 1 | `STM32H7A3IIT6Q` | STMicroelectronics | 2 | 4109 | 1 | 17.56 | [digikey](https://www.digikey.nl/en/products/detail/stmicroelectronics/STM32H7A3IIT6Q/11591229) · [mouser](https://cz.mouser.com/en/ProductDetail/STMicroelectronics/STM32H7A3IIT6Q?qs=xZ%2FP%252Ba9zWqZ1%252B8qVQuQm5g%3D%3D) · [tme](https://www.tme.eu/en/details/stm32h7a3iit6q/) |
| 2 | `STM32H743IIT6` | STMicroelectronics | 2 | 2417 | 1 | 15.07 | [digikey](https://www.digikey.nl/en/products/detail/stmicroelectronics/STM32H743IIT6/7915904) · [tme](https://www.tme.eu/en/details/stm32h743iit6/) |
| 3 | `STM32H7S3Z8T6` | STMicroelectronics | 2 | 919 | 1 | 7.14 | [digikey](https://www.digikey.nl/en/products/detail/stmicroelectronics/STM32H7S3Z8T6/22485821) · [tme](https://www.tme.eu/en/details/stm32h7s3z8t6/) |
| 4 | `STM32H7R3V8T6` | STMicroelectronics | 2 | 596 | 1 | 6.06 | [digikey](https://www.digikey.nl/en/products/detail/stmicroelectronics/STM32H7R3V8T6/22486039) · [tme](https://www.tme.eu/en/details/stm32h7r3v8t6/) |
| 5 | `STM32H750VBT6` | STMicroelectronics | 2 | 319 | 1 | 7.22 | [digikey](https://www.digikey.nl/en/products/detail/stmicroelectronics/STM32H750VBT6/9453371) · [tme](https://www.tme.eu/en/details/stm32h750vbt6/) |
| 6 | `STM32H7S7I8T6` | STMicroelectronics | 2 | 287 | 1 | 10.99 | [digikey](https://www.digikey.nl/en/products/detail/stmicroelectronics/STM32H7S7I8T6/22485846) · [tme](https://www.tme.eu/en/details/stm32h7s7i8t6/) |
| 7 | `STM32H7R7I8T6` | STMicroelectronics | 2 | 265 | 1 | 7.26 | [digikey](https://www.digikey.nl/en/products/detail/stmicroelectronics/STM32H7R7I8T6/22485804) · [tme](https://www.tme.eu/en/details/stm32h7r7i8t6/) |
| 8 | `STM32H753IIT6` | STMicroelectronics | 2 | 34 | 1 | 14.11 | [digikey](https://www.digikey.nl/en/products/detail/stmicroelectronics/STM32H753IIT6/8257853) · [tme](https://www.tme.eu/en/details/stm32h753iit6/) |
| 9 | `STM32H753ZIT6` | STMicroelectronics | 1 | 5365 | 1 | 18.86 | [digikey](https://www.digikey.nl/en/products/detail/stmicroelectronics/STM32H753ZIT6/8257856) · [tme](https://www.tme.eu/en/details/stm32h753zit6/) |
| 10 | `STM32H753AII6` | STMicroelectronics | 1 | 3314 | 1 | 18.23 | [digikey](https://www.digikey.nl/en/products/detail/stmicroelectronics/STM32H753AII6/10326800) |

_200 further qualifying parts not shown; `sourcing-report` states the full count and what rejected the rest._

## `microsd`

microSD socket, push-push or push-pull, card-detect switch, SMD, >=5000 insertion cycles

173 of 601 swept candidates qualify. **Nothing is picked yet for this requirement.**

Still to confirm by hand for whichever part is chosen: `card_detect_switch`, `insertion_cycles`, `sd_logo_not_required`.

| # | MPN | manufacturer | EU src | stock | MOQ | EUR @1 | where |
|---|---|---|---:|---:|---:|---:|---|
| 1 | `1140084168` | AMPHENOL COMMUNICATIONS SOLUTIONS | 3 | 86238 | 1 | 1.53 | [digikey](https://www.digikey.nl/en/products/detail/amphenol-cs-commercial-products/1140084168/2187086) · [mouser](https://cz.mouser.com/en/ProductDetail/Amphenol-Commercial-Products/1140084168?qs=tcdQdce0DUrgPsHyuD5Law%3D%3D) · [tme](https://www.tme.eu/en/details/1140084168/) |
| 2 | `2908-05WB-MG` ~~RULED OUT~~<br>_8 contacts with no ninth: no card-detect switch, so firmware would have to poll the bus to…_ | 3M | 3 | 73578 | 1 | 4.23 | [digikey](https://www.digikey.nl/en/products/detail/3m/2908-05WB-MG/1242497) · [mouser](https://cz.mouser.com/en/ProductDetail/3M-Electronic-Solutions-Division/2908-05WB-MG?qs=vnwGVgFuQiZPJoOt79ALlw%3D%3D) · [tme](https://www.tme.eu/en/details/290805wbmg-3m-0/) |
| 3 | `MEM2080-00-128-00-A` | GCT | 3 | 23887 | 1 | 1.23 | [digikey](https://www.digikey.nl/en/products/detail/gct/MEM2080-00-128-00-A/16681105) · [mouser](https://cz.mouser.com/en/ProductDetail/GCT/MEM2080-00-128-00-A?qs=vvQtp7zwQdPJXzpf1SnYug%3D%3D) · [tme](https://www.tme.eu/en/details/mem2080-00-128-00a/) |
| 4 | `105027-0001` ~~RULED OUT~~<br>_MOQ 2000. 150000 in stock at two distributors and unbuyable for a five-board prototype run…_ | MOLEX | 2 | 150000 | 1 | 1.51 | [digikey](https://www.digikey.nl/en/products/detail/waldom/105027-0001/28730120) · [mouser](https://cz.mouser.com/en/ProductDetail/Molex/105027-0001?qs=39WuQm6e6OkSeL34qdqRiA%3D%3D) · [tme](https://www.tme.eu/en/details/1050270001-mol/) |
| 5 | `10067099-200LF` | AMPHENOL | 2 | 41021 | 1 | 0.86 | [digikey](https://www.digikey.nl/en/products/detail/amphenol-cs-fci/10067099-200LF/4238739) · [mouser](https://cz.mouser.com/en/ProductDetail/Amphenol-FCI/10067099-200LF?qs=i4V1y6YhAeneA%2F%2FxXHdZ7w%3D%3D) · [tme](https://www.tme.eu/en/details/10067099200lf-amp/) |
| 6 | `MEM2075-00-140-01-A` | GCT | 2 | 25021 | 1 | 1.56 | [digikey](https://www.digikey.nl/en/products/detail/gct/MEM2075-00-140-01-A/9859614) · [tme](https://www.tme.eu/en/details/mem2075-00-140-01a/) |
| 7 | `DM3D-SF` | HIROSE | 2 | 24323 | 1 | 1.43 | [digikey](https://www.digikey.nl/en/products/detail/hirose-electric-co-ltd/DM3D-SF/1786510) · [tme](https://www.tme.eu/en/details/dm3d-sf/c/) |
| 8 | `MEM2061-01-188-00-A` | GCT | 2 | 20106 | 1 | 0.93 | [digikey](https://www.digikey.nl/en/products/detail/gct/MEM2061-01-188-00-A/9859612) · [tme](https://www.tme.eu/en/details/mem2061-01-188-00a/) |
| 9 | `DM3BT-DSF-PEJS` | HIROSE | 2 | 18263 | 1 | 2.58 | [digikey](https://www.digikey.nl/en/products/detail/hirose-electric-co-ltd/DM3BT-DSF-PEJS/1982749) · [tme](https://www.tme.eu/en/details/dm3bt-dsf-pejs/c/) |
| 10 | `DM3D-SF(41)` | HIROSE | 2 | 15571 | 1 | 1.88 | [digikey](https://www.digikey.nl/en/products/detail/hirose-electric-co-ltd/DM3D-SF-41/4283097) · [tme](https://www.tme.eu/en/details/dm3dsf41-hir-0/) |

_163 further qualifying parts not shown; `sourcing-report` states the full count and what rejected the rest._

## `battery`

1S Li-Po, >=1000 mAh, finished cell with its own PCM, connectorised, fits a 35 x 59 x 7.1 mm bay, per-model UN 38.3 + IEC 62133-2 docs available

31 of 627 swept candidates qualify. **Nothing is picked yet for this requirement.**

Still to confirm by hand for whichever part is chosen: `un38_3_report`, `iec62133_2_report`, `watt_hours`, `thickness_mm`, `width_mm`, `length_mm`.

| # | MPN | manufacturer | EU src | stock | MOQ | EUR @1 | where |
|---|---|---|---:|---:|---:|---:|---|
| 1 | `AKY0107` | AKYGA | 2 | 2873 | 1 | 6.71 | [digikey](https://www.digikey.nl/en/products/detail/akyga/AKY0107/26848639) · [tme](https://www.tme.eu/en/details/aky-lp503759/) |
| 2 | `AKY0393` | AKYGA BATTERY | 1 | 1545 | 1 | 8.39 | [tme](https://www.tme.eu/en/details/aky-lp903759/) |
| 3 | `AKY-LP623255` | AKYGA BATTERY | 1 | 1298 | 1 | 4.61 | [tme](https://www.tme.eu/en/details/aky-lp623255/) |
| 4 | `AKY0571` | AKYGA BATTERY | 1 | 1124 | 1 | 6.36 | [tme](https://www.tme.eu/en/details/aky-lp803040/) |
| 5 | `AKY0662` | AKYGA BATTERY | 1 | 905 | 1 | 6.02 | [tme](https://www.tme.eu/en/details/aky-lp103035/) |
| 6 | `AKY0638` | AKYGA BATTERY | 1 | 896 | 1 | 6.31 | [tme](https://www.tme.eu/en/details/aky-lp603450/) |
| 7 | `AKY0627` | AKYGA BATTERY | 1 | 854 | 1 | 11.34 | [tme](https://www.tme.eu/en/details/aky-lp675568/) |
| 8 | `AKY0535` | AKYGA BATTERY | 1 | 517 | 1 | 6.98 | [tme](https://www.tme.eu/en/details/aky-lp405063/) |
| 9 | `AKY0622` | AKYGA BATTERY | 1 | 471 | 1 | 5.56 | [tme](https://www.tme.eu/en/details/aky-lp883440/) |
| 10 | `AKY1377` | AKYGA BATTERY | 1 | 381 | 1 | 8.06 | [tme](https://www.tme.eu/en/details/aky-lp613381/) |

_21 further qualifying parts not shown; `sourcing-report` states the full count and what rejected the rest._

## `bluetooth`

pre-qualified BT module (inheritable QDID + EU radio cert), SBC, no vendor NDA — DO NOT POPULATE on the shipped wired-only board

324 of 805 swept candidates qualify. **Nothing is picked yet for this requirement.**

Still to confirm by hand for whichever part is chosen: `bt_sig_qualified`, `radio_certified_eu`, `requires_vendor_nda`, `codec_sbc`.

| # | MPN | manufacturer | EU src | stock | MOQ | EUR @1 | where |
|---|---|---|---:|---:|---:|---:|---|
| 1 | `ESP32-WROOM-32UE-N16` | ESPRESSIF | 2 | 17253 | 1 | 5.22 | [tme](https://www.tme.eu/en/details/esp32-wroom-32ue16/) |
| 2 | `ESP32-WROVER-E-N8R8` | ESPRESSIF | 2 | 12680 | 1 | 5.22 | [tme](https://www.tme.eu/en/details/esp32-wrover-e-8/) |
| 3 | `ESP32-WROVER-IE-N8R8` | ESPRESSIF | 2 | 7331 | 1 | 5.22 | [tme](https://www.tme.eu/en/details/esp32-wrover-ie-8/) |
| 4 | `RN4871U-V/RM118` | MICROCHIP TECHNOLOGY | 2 | 7310 | 1 | 6.14 | [tme](https://www.tme.eu/en/details/rn4871u-v/rm118/) |
| 5 | `ESP32-S3-MINI-1-N8` | ESPRESSIF | 2 | 7192 | 1 | 4.82 | [tme](https://www.tme.eu/en/details/esp32s3-mini-1-8/) |
| 6 | `ESP32-WROOM-32UE-N8` | ESPRESSIF | 2 | 7116 | 1 | 4.82 | [tme](https://www.tme.eu/en/details/esp32-wroom-32ue-8/) |
| 7 | `ATWILC3000-MR110CA` | MICROCHIP TECHNOLOGY | 2 | 6810 | 1 | 14.32 | [tme](https://www.tme.eu/en/details/wilc3000-mr110ca/) |
| 8 | `NINA-B306-00B` | u-blox | 2 | 6581 | 1 | 8.81 | [tme](https://www.tme.eu/en/details/nina-b306-00b/) |
| 9 | `ESP32-C3-WROOM-02-N4` | ESPRESSIF | 2 | 6502 | 1 | 3.00 | [tme](https://www.tme.eu/en/details/esp32c3-wroom-02n4/) |
| 10 | `ESP32-S3-WROOM-1U-N16R2` | ESPRESSIF | 2 | 5864 | 1 | 5.95 | [tme](https://www.tme.eu/en/details/esp32s3-wrm1u-16r2/) |

_314 further qualifying parts not shown; `sourcing-report` states the full count and what rejected the rest._
