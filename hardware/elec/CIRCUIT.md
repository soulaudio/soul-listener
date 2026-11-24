# Circuit — SoulAudio DAP
**Generated** from `hardware/elec/src/**/*.ato` by `tools/ato-diagram.py`. Do not edit.
This is a reading of the SOURCES, not a netlist. It shows connections as written, knows nothing about footprints, designators or placement, and draws one edge for an interface-to-interface connection that carries many signals. For the authoritative pinout, power tree and 3D model, sign in (`ato auth login`) and run `ato build -b default` — see `docs/hardware/ATOPILE-TOOLCHAIN.md` for why that needs a person.

## Board
```mermaid
flowchart LR
  mcu["mcu<br/><i>MCU</i>"]
  audio["audio<br/><i>Audio</i>"]
  power["power<br/><i>Power</i>"]
  display["display<br/><i>Display</i>"]
  bluetooth["bluetooth<br/><i>Bluetooth</i>"]
  memory["memory<br/><i>Memory</i>"]
  input["input<br/><i>Input</i>"]
  clock["clock<br/><i>HseClock</i>"]
  swd_main["swd_main<br/><i>SWDPort</i>"]
  swd_wb["swd_wb<br/><i>SWDPort</i>"]
  audio ---|audio_i2c, audio_i2s| mcu
  audio ---|amp_rail, power_3v3| power
  bluetooth ---|bt_uart| mcu
  bluetooth ---|power_3v3| power
  bluetooth ---|swd_wb| swd_wb
  clock ---|hse_clk| mcu
  clock ---|power_3v3| power
  display ---|display_ctrl, display_spi| mcu
  display ---|power_3v3| power
  input ---|encoder| mcu
  input ---|power_3v3| power
  mcu ---|fmc, quadspi| memory
  mcu ---|amp_en, pmic_i2c, power_3v3, +1| power
  mcu ---|swd| swd_main
  memory ---|power_3v3| power
```
`SoulAudioDAP` instantiates 10 modules with 22 connection statements between them.

## Modules

### `Audio`
<sub>hardware/elec/src/audio.ato</sub>
14 part(s), 26 passive(s), 14 assert(s)
```mermaid
flowchart LR
  power_3v3(["power_3v3"])
  i2s(["i2s"])
  i2c(["i2c"])
  amp_rail(["amp_rail"])
  avdd(["avdd"])
  pvdd(["pvdd"])
  pvss(["pvss"])
  headphone_out["headphone_out<br/><i>AnalogAudioOut</i>"]
  agnd["agnd<br/><i>Electrical</i>"]
  ldo_avdd["ldo_avdd<br/><i>LT3042EMSE</i>"]
  dac["dac<br/><i>ES9038Q2M</i>"]
  ldo_pvdd["ldo_pvdd<br/><i>LT3045EMSE</i>"]
  ldo_pvss["ldo_pvss<br/><i>LT3094EMSE</i>"]
  amp["amp<br/><i>TPA6120A2</i>"]
  power_3v3 --- ldo_avdd
  ldo_avdd --- avdd
  power_3v3 --- avdd
  power_3v3 --- dac
  avdd --- dac
  i2s --- dac
  i2c --- dac
  amp_rail --- ldo_pvdd
  ldo_pvdd --- pvdd
  pvdd --- amp_rail
  amp_rail --- ldo_pvss
  ldo_pvss --- pvss
  pvss --- amp_rail
  pvdd --- amp
  pvss --- amp
  dac --- amp
  amp --- headphone_out
  headphone_out --- avdd
```

<details><summary>26 passives (collapsed)</summary>

- `c_amp_l_neg` — Capacitor
- `c_amp_l_pos` — Capacitor
- `c_amp_r_neg` — Capacitor
- `c_amp_r_pos` — Capacitor
- `c_avdd_bulk` — Capacitor
- `c_avdd_hf` — Capacitor
- `c_byp` — Capacitor = 10uF +/- 20%
- `c_dac_1` — Capacitor
- `c_dac_2` — Capacitor
- `c_pvdd_bulk` — Capacitor
- `c_pvdd_hf` — Capacitor
- `c_pvss_bulk` — Capacitor
- `c_pvss_hf` — Capacitor
- `r_fb_l` — Resistor = 1.5kohm +/- 1%
- `r_fb_r` — Resistor = 1.5kohm +/- 1%
- `r_ilim` — Resistor = 100kohm +/- 5%
- `r_iref` — Resistor = 1.21kohm +/- 1%
- `r_mute` — Resistor = 10kohm +/- 5%
- `r_pg` — Resistor = 100kohm +/- 5%
- `r_pvdd_ilim` — Resistor = 2.0kohm +/- 5%
- `r_pvdd_pg` — Resistor = 100kohm +/- 5%
- `r_pvdd_set` — Resistor = 49.9kohm +/- 1%
- `r_pvss_ilim` — Resistor = 2.0kohm +/- 5%
- `r_pvss_pg` — Resistor = 100kohm +/- 5%
- `r_pvss_set` — Resistor = 49.9kohm +/- 1%
- `r_set` — Resistor = 33kohm +/- 1%

</details>

<details><summary>design rules (asserts)</summary>

- `avdd.voltage within 3.2V to 3.4V  # LT3042 output tolerance`
- `r_set.resistance within 32kohm to 34kohm`
- `r_ilim.resistance within 90kohm to 110kohm`
- `c_byp.capacitance within 8uF to 12uF`
- `power_3v3.voltage within 3.0V to 3.6V`
- `avdd.voltage within 3.2V to 3.4V`
- `r_mute.resistance within 8kohm to 12kohm`
- `r_iref.resistance within 1.19kohm to 1.23kohm`
- `r_pvdd_set.resistance within 49.4kohm to 50.4kohm`
- `r_pvss_set.resistance within 49.4kohm to 50.4kohm`
- `r_fb_l.resistance within 1.35kohm to 1.65kohm`
- `r_fb_r.resistance within 1.35kohm to 1.65kohm`
- `pvdd.voltage within 4.8V to 5.2V`
- `pvss.voltage within 4.8V to 5.2V`

</details>

> These are NOT checked by the offline build — they are solver constraints evaluated after part picking.

### `Bluetooth`
<sub>hardware/elec/src/bluetooth.ato</sub>
7 part(s), 9 passive(s), 4 assert(s)
```mermaid
flowchart LR
  power_3v3(["power_3v3"])
  uart(["uart"])
  swd_wb(["swd_wb"])
  wb55["wb55<br/><i>STM32WB55RGV6</i>"]
  hse["hse<br/><i>Crystal</i>"]
  lse["lse<br/><i>Crystal</i>"]
  ant["ant<br/><i>Ant2450AT18A100E</i>"]
  power_3v3 --- wb55
  uart --- wb55
  swd_wb --- wb55
  hse --- wb55
  lse --- wb55
  wb55 --- ant
  ant --- power_3v3
```

<details><summary>9 passives (collapsed)</summary>

- `c_hse_1` — Capacitor
- `c_hse_2` — Capacitor
- `c_lse_1` — Capacitor
- `c_lse_2` — Capacitor
- `c_vdd_1` — Capacitor
- `c_vdd_2` — Capacitor
- `c_vdd_3` — Capacitor
- `c_vdd_4` — Capacitor
- `c_vdd_bulk` — Capacitor

</details>

<details><summary>design rules (asserts)</summary>

- `power_3v3.voltage within 1.7V to 3.6V  # WB55 supply range`
- `hse.frequency within 31MHz to 33MHz`
- `lse.frequency within 32kHz to 33768Hz`
- `c_vdd_bulk.capacitance within 8uF to 12uF`

</details>

> These are NOT checked by the offline build — they are solver constraints evaluated after part picking.

### `Display`
<sub>hardware/elec/src/display.ato</sub>
9 part(s), 17 passive(s), 1 assert(s)
```mermaid
flowchart LR
  power_3v3(["power_3v3"])
  spi["spi<br/><i>SPIDevice</i>"]
  ctrl["ctrl<br/><i>DisplayCtrl</i>"]
  fpc["fpc<br/><i>FH12_24S</i>"]
  q1["q1<br/><i>Si1308EDL</i>"]
  d1["d1<br/><i>MBR0530</i>"]
  d2["d2<br/><i>MBR0530</i>"]
  tvs1["tvs1<br/><i>PESD5V0L5UY</i>"]
  tvs2["tvs2<br/><i>PESD5V0L5UY</i>"]
  power_3v3 --- fpc
  fpc --- spi
  fpc --- ctrl
  fpc --- q1
  d1 --- fpc
  d2 --- fpc
  power_3v3 --- tvs1
  power_3v3 --- tvs2
  tvs1 --- spi
  tvs1 --- ctrl
  tvs2 --- ctrl
```

<details><summary>17 passives (collapsed)</summary>

- `c_boost_in` — Capacitor = 4.7uF +/- 20%
- `c_rst` — Capacitor
- `c_vci_bulk` — Capacitor
- `c_vci_hf` — Capacitor
- `c_vcom` — Capacitor
- `c_vdd` — Capacitor
- `c_vddio` — Capacitor
- `c_vgh` — Capacitor
- `c_vgl` — Capacitor
- `c_vsh1` — Capacitor
- `c_vsh2` — Capacitor
- `c_vsl` — Capacitor
- `l1` — Inductor = 47uH +/- 20%
- `r_bias` — Resistor = 1Mohm +/- 5%
- `r_bs1` — Resistor
- `r_busy` — Resistor = 10kohm +/- 5%
- `r_sense` — Resistor = 2.2ohm +/- 5%

</details>

<details><summary>design rules (asserts)</summary>

- `power_3v3.voltage within 2.7V to 3.6V`

</details>

> These are NOT checked by the offline build — they are solver constraints evaluated after part picking.

### `HseClock`
<sub>hardware/elec/src/clock.ato</sub>
3 part(s), 2 passive(s), 3 assert(s)
```mermaid
flowchart LR
  power_3v3(["power_3v3"])
  clk(["clk"])
  xtal["xtal<br/><i>Crystal</i>"]
  xtal --- clk
```

<details><summary>2 passives (collapsed)</summary>

- `c_osc_in` — Capacitor = 18pF +/- 5%
- `c_osc_out` — Capacitor = 18pF +/- 5%

</details>

<details><summary>design rules (asserts)</summary>

- `xtal.frequency within 24MHz to 26MHz`
- `c_osc_in.capacitance  within 15pF to 22pF`
- `c_osc_out.capacitance within 15pF to 22pF`

</details>

> These are NOT checked by the offline build — they are solver constraints evaluated after part picking.

### `Input`
<sub>hardware/elec/src/input.ato</sub>
10 part(s), 20 passive(s), 20 assert(s)
```mermaid
flowchart LR
  power_3v3(["power_3v3"])
  encoder(["encoder"])
  enc["enc<br/><i>EC11</i>"]
  btn_play["btn_play<br/><i>PTS526</i>"]
  btn_prev["btn_prev<br/><i>PTS526</i>"]
  btn_next["btn_next<br/><i>PTS526</i>"]
  btn_menu["btn_menu<br/><i>PTS526</i>"]
  btn_back["btn_back<br/><i>PTS526</i>"]
  btn_sel["btn_sel<br/><i>PTS526</i>"]
  btn_pwr["btn_pwr<br/><i>PTS526</i>"]
  enc --- encoder
  enc --- power_3v3
  btn_play --- encoder
  btn_play --- power_3v3
  btn_prev --- encoder
  btn_prev --- power_3v3
  btn_next --- encoder
  btn_next --- power_3v3
  btn_menu --- encoder
  btn_menu --- power_3v3
  btn_back --- encoder
  btn_back --- power_3v3
  btn_sel --- encoder
  btn_sel --- power_3v3
  btn_pwr --- encoder
  btn_pwr --- power_3v3
```

<details><summary>20 passives (collapsed)</summary>

- `c_back` — Capacitor
- `c_enc_a` — Capacitor
- `c_enc_b` — Capacitor
- `c_enc_sw` — Capacitor
- `c_menu` — Capacitor
- `c_next` — Capacitor
- `c_play` — Capacitor
- `c_prev` — Capacitor
- `c_pwr` — Capacitor
- `c_sel` — Capacitor
- `r_back` — Resistor
- `r_enc_a` — Resistor
- `r_enc_b` — Resistor
- `r_enc_sw` — Resistor
- `r_menu` — Resistor
- `r_next` — Resistor
- `r_play` — Resistor
- `r_prev` — Resistor
- `r_pwr` — Resistor
- `r_sel` — Resistor

</details>

<details><summary>design rules (asserts)</summary>

- `power_3v3.voltage within 2.7V to 3.6V`
- `r_enc_a.resistance within 9kohm to 11kohm`
- `r_enc_b.resistance within 9kohm to 11kohm`
- `r_enc_sw.resistance within 9kohm to 11kohm`
- `r_play.resistance within 9kohm to 11kohm`
- `r_prev.resistance within 9kohm to 11kohm`
- `r_next.resistance within 9kohm to 11kohm`
- `r_menu.resistance within 9kohm to 11kohm`
- `r_back.resistance within 9kohm to 11kohm`
- `r_sel.resistance  within 9kohm to 11kohm`
- `r_pwr.resistance  within 9kohm to 11kohm`
- `c_enc_a.capacitance within 80nF to 120nF`
- `c_enc_b.capacitance within 80nF to 120nF`
- `c_enc_sw.capacitance within 80nF to 120nF`
- `c_play.capacitance within 80nF to 120nF`
- `c_prev.capacitance within 80nF to 120nF`
- `c_next.capacitance within 80nF to 120nF`
- `c_menu.capacitance within 80nF to 120nF`
- `c_back.capacitance within 80nF to 120nF`
- `c_sel.capacitance  within 80nF to 120nF`

</details>

> These are NOT checked by the offline build — they are solver constraints evaluated after part picking.

### `MCU`
<sub>hardware/elec/src/mcu.ato</sub>
18 part(s), 19 passive(s), 2 assert(s)
```mermaid
flowchart LR
  power_3v3(["power_3v3"])
  audio_i2s(["audio_i2s"])
  audio_i2c(["audio_i2c"])
  pmic_i2c(["pmic_i2c"])
  sdcard(["sdcard"])
  bt_uart(["bt_uart"])
  swd(["swd"])
  fmc(["fmc"])
  quadspi(["quadspi"])
  encoder(["encoder"])
  hse_clk(["hse_clk"])
  gnd["gnd<br/><i>Electrical</i>"]
  display_spi["display_spi<br/><i>SPIDevice</i>"]
  display_ctrl["display_ctrl<br/><i>DisplayCtrl</i>"]
  usb["usb<br/><i>USBDiffPair</i>"]
  amp_en["amp_en<br/><i>Electrical</i>"]
  mcu["mcu<br/><i>STM32H743ZIT6</i>"]
  lse_xtal["lse_xtal<br/><i>Crystal</i>"]
  power_3v3 --- mcu
  hse_clk --- mcu
  audio_i2s --- mcu
  audio_i2c --- mcu
  pmic_i2c --- mcu
  display_spi --- mcu
  display_ctrl --- mcu
  sdcard --- mcu
  bt_uart --- mcu
  usb --- mcu
  swd --- mcu
  fmc --- mcu
  quadspi --- mcu
  amp_en --- mcu
  encoder --- mcu
```

<details><summary>19 passives (collapsed)</summary>

- `c_lse_1` — Capacitor
- `c_lse_2` — Capacitor
- `c_vcap1` — Capacitor
- `c_vcap2` — Capacitor
- `c_vdd_1` — Capacitor
- `c_vdd_10` — Capacitor
- `c_vdd_11` — Capacitor
- `c_vdd_2` — Capacitor
- `c_vdd_3` — Capacitor
- `c_vdd_4` — Capacitor
- `c_vdd_5` — Capacitor
- `c_vdd_6` — Capacitor
- `c_vdd_7` — Capacitor
- `c_vdd_8` — Capacitor
- `c_vdd_9` — Capacitor
- `c_vdd_bulk` — Capacitor = 10uF +/- 20%
- `c_vdd_usb` — Capacitor
- `c_vdda_1` — Capacitor
- `c_vdda_2` — Capacitor

</details>

<details><summary>design rules (asserts)</summary>

- `power_3v3.voltage within 3.0V to 3.6V`
- `lse_xtal.frequency within 32kHz to 33768Hz     # 32.768 kHz LSE — RTC`

</details>

> These are NOT checked by the offline build — they are solver constraints evaluated after part picking.

### `Memory`
<sub>hardware/elec/src/memory.ato</sub>
5 part(s), 9 passive(s), 6 assert(s)
```mermaid
flowchart LR
  power_3v3(["power_3v3"])
  fmc(["fmc"])
  quadspi(["quadspi"])
  sdram["sdram<br/><i>W9825G6KH6</i>"]
  nor["nor<br/><i>W25Q128JVSIQ</i>"]
  power_3v3 --- sdram
  fmc --- sdram
  power_3v3 --- nor
  quadspi --- nor
```

<details><summary>9 passives (collapsed)</summary>

- `c_nor_bulk` — Capacitor
- `c_nor_hf` — Capacitor
- `c_sdram_1` — Capacitor
- `c_sdram_2` — Capacitor
- `c_sdram_bulk` — Capacitor
- `r_io0` — Resistor
- `r_io1` — Resistor
- `r_io2` — Resistor
- `r_io3` — Resistor

</details>

<details><summary>design rules (asserts)</summary>

- `power_3v3.voltage within 2.7V to 3.6V`
- `r_io0.resistance within 30ohm to 36ohm`
- `r_io1.resistance within 30ohm to 36ohm`
- `r_io2.resistance within 30ohm to 36ohm`
- `r_io3.resistance within 30ohm to 36ohm`
- `c_nor_bulk.capacitance within 8uF to 12uF`

</details>

> These are NOT checked by the offline build — they are solver constraints evaluated after part picking.

### `Power`
<sub>hardware/elec/src/power.ato</sub>
12 part(s), 33 passive(s), 15 assert(s)
```mermaid
flowchart LR
  pmic_i2c(["pmic_i2c"])
  power_3v3(["power_3v3"])
  power_sys(["power_sys"])
  amp_rail(["amp_rail"])
  usb_in["usb_in<br/><i>USBDiffPair</i>"]
  amp_en["amp_en<br/><i>Electrical</i>"]
  gnd["gnd<br/><i>Electrical</i>"]
  pmic["pmic<br/><i>BQ25895RTWT</i>"]
  batt_conn["batt_conn<br/><i>S2B_PH_K_S</i>"]
  ldo_3v3["ldo_3v3<br/><i>AP2112K33TRG1</i>"]
  boost["boost<br/><i>TPS61023DRLR</i>"]
  inv["inv<br/><i>TPS63700DRCR</i>"]
  power_sys --- pmic
  pmic_i2c --- pmic
  usb_in --- pmic
  batt_conn --- pmic
  batt_conn --- power_sys
  power_sys --- ldo_3v3
  ldo_3v3 --- power_3v3
  power_3v3 --- power_sys
  power_sys --- boost
  amp_en --- boost
  amp_rail --- power_sys
  power_sys --- inv
```

<details><summary>33 passives (collapsed)</summary>

- `c_3v3_bulk` — Capacitor
- `c_3v3_hf` — Capacitor
- `c_boost_in` — Capacitor = 10uF +/- 20%
- `c_boost_o1` — Capacitor
- `c_boost_o2` — Capacitor
- `c_btst` — Capacitor
- `c_inv_bias` — Capacitor = 100nF +/- 20%
- `c_inv_comp` — Capacitor = 4.7nF +/- 20%
- `c_inv_o1` — Capacitor
- `c_inv_o2` — Capacitor
- `c_inv_seq` — Capacitor = 100nF +/- 20%
- `c_pi_neg_in` — Capacitor = 10uF +/- 20%
- `c_pi_neg_out` — Capacitor = 10uF +/- 20%
- `c_pi_pos_in` — Capacitor = 10uF +/- 20%
- `c_pi_pos_out` — Capacitor = 10uF +/- 20%
- `c_pmid` — Capacitor
- `c_regn` — Capacitor
- `c_sys_bulk` — Capacitor
- `c_sys_hf` — Capacitor
- `fb_neg` — Resistor = 1ohm +/- 99%
- `fb_pos` — Resistor = 1ohm +/- 99%
- `l_boost` — Inductor = 1.5uH +/- 20%
- `l_inv` — Inductor = 1.0uH +/- 20%
- `l_sw` — Inductor = 4.7uH +/- 20%
- `r_boost_bot` — Resistor = 100kohm +/- 1%
- `r_boost_top` — Resistor = 357kohm +/- 1%
- `r_ilim` — Resistor = 100ohm +/- 5%
- `r_inv_bias` — Resistor = 10ohm +/- 5%
- `r_inv_bot` — Resistor = 100kohm +/- 1%
- `r_inv_seq` — Resistor = 100kohm +/- 5%
- `r_inv_top` — Resistor = 470kohm +/- 1%
- `r_stat` — Resistor
- `r_ts` — Resistor = 5.1kohm +/- 5%

</details>

<details><summary>design rules (asserts)</summary>

- `power_3v3.voltage within 3.2V to 3.4V`
- `power_sys.voltage within 3.0V to 4.35V  # LiPo operating range`
- `r_ilim.resistance within 90ohm to 110ohm`
- `c_btst.capacitance within 50nF to 150nF`
- `c_sys_bulk.capacitance within 15uF to 30uF`
- `l_sw.inductance within 3.5uH to 6uH`
- `r_boost_top.resistance within 350kohm to 364kohm`
- `r_boost_bot.resistance within 99kohm to 101kohm`
- `l_boost.inductance within 1.2uH to 1.8uH`
- `r_inv_top.resistance within 465kohm to 475kohm`
- `r_inv_bot.resistance within 99kohm to 101kohm`
- `l_inv.inductance within 0.8uH to 1.2uH`
- `c_inv_comp.capacitance within 3.5nF to 6nF`
- `r_inv_seq.resistance within 90kohm to 110kohm`
- `c_inv_seq.capacitance within 80nF to 120nF`

</details>

> These are NOT checked by the offline build — they are solver constraints evaluated after part picking.
