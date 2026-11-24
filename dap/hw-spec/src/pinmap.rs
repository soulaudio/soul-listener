//! Pin assignments, GENERATED from the atopile sources. Do not edit.
//!
//! Regenerate with `tools/ato-pinmap.py`; `the_pinmap_is_current` in
//! `dap-arch-tests` fails the build when this file and
//! `hardware/elec/src/**/*.ato` disagree.
//!
//! # Why the firmware reads a generated table
//!
//! A component's pin assignment is a schematic fact — `signal CS_NEG ~ pin 1`
//! is what becomes a pad connection in the netlist and a pin on the footprint.
//! Declaring it a second time in Rust would create a copy that does not drive
//! the board, and the two would drift the first time a pin moved. So the `.ato`
//! definition is written by a person against a datasheet and this table is
//! derived from it.
//!
//! The firmware must not name a pin any other way:
//! `the_firmware_core_names_no_pin_or_peripheral` forbids pin literals in
//! `dap-core`, and a driver in the board crate takes its pins as parameters.
//!
//! Pin designators are `&str`, not integers, because a BGA pin is `A1`.
//!
//! 22 component(s), 336 pin assignment(s).

/// One component's pin assignment, as its `.ato` definition states it.
#[derive(Debug, Clone, Copy)]
pub struct ComponentPins {
    /// The `.ato` block name (`component W25Q128JVSIQ`).
    pub component: &'static str,
    /// From a vendored part's `is_atomic_part` trait, when it has one.
    pub manufacturer: Option<&'static str>,
    pub mpn: Option<&'static str>,
    /// Where it is defined, so a failure points at a file to open.
    pub source: &'static str,
    /// `(signal, pin)`, ordered by pin.
    pub pins: &'static [(&'static str, &'static str)],
}

impl ComponentPins {
    /// The pin a signal is on.
    #[must_use]
    pub fn pin(&self, signal: &str) -> Option<&'static str> {
        let mut i = 0;
        while i < self.pins.len() {
            if str_eq(self.pins[i].0, signal) {
                return Some(self.pins[i].1);
            }
            i += 1;
        }
        None
    }
}

/// `str::eq` is not `const`-friendly here and this crate carries no deps; a
/// byte compare keeps the module usable from `no_std` without `core::str`
/// niceties that pull in formatting machinery.
fn str_eq(a: &str, b: &str) -> bool {
    a.as_bytes() == b.as_bytes()
}

/// Every component the atopile design defines, with its pins.
pub const COMPONENTS: &[ComponentPins] = &[
    ComponentPins {
        component: "AP2112K33TRG1",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/AP2112K33TRG1/AP2112K33TRG1.ato",
        pins: &[("VIN", "1"), ("GND", "2"), ("EN", "3"), ("NC", "4"), ("VOUT", "5")],
    },
    ComponentPins {
        component: "Ant2450AT18A100E",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/0433AT62A0100E/0433AT62A0100E.ato",
        pins: &[("RF", "1"), ("GND", "2")],
    },
    ComponentPins {
        component: "BQ25895RTWT",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/BQ25895RTWT/BQ25895RTWT.ato",
        pins: &[("PGND_1", "1"), ("VBUS_1", "2"), ("VBUS_2", "3"), ("DM", "4"), ("DP", "5"), ("STAT", "6"), ("INT", "7"), ("SCL", "8"), ("SDA", "9"), ("OTG", "10"), ("CE", "11"), ("ILIM", "12"), ("TS", "13"), ("BTST", "14"), ("SW_1", "15"), ("SW_2", "16"), ("REGN", "17"), ("SYS_1", "18"), ("SYS_2", "19"), ("BAT_1", "20"), ("BAT_2", "21"), ("PMID_1", "22"), ("PMID_2", "23"), ("PGND_2", "24"), ("EP", "25")],
    },
    ComponentPins {
        component: "EC11",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/EC11/EC11.ato",
        pins: &[("ENC_A", "1"), ("GND_ENC", "2"), ("ENC_B", "3"), ("SW_1", "4"), ("SW_2", "5")],
    },
    ComponentPins {
        component: "ES9038Q2M",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/ES9038Q2M/ES9038Q2M.ato",
        pins: &[("DVDD", "1"), ("DVSS", "2"), ("SCL", "3"), ("SDA", "4"), ("MUTE", "5"), ("IREF", "6"), ("AGND_1", "7"), ("AVDD_1", "9"), ("AOUT_R_NEG", "11"), ("AOUT_R_POS", "12"), ("AOUT_L_NEG", "13"), ("AOUT_L_POS", "14"), ("AVSS_1", "15"), ("AVDD_3", "17"), ("AGND_3", "19"), ("IREF_2", "20"), ("DVSS_2", "21"), ("DVDD_2", "22"), ("XO", "23"), ("MCLK", "24"), ("BCLK", "25"), ("LRCK", "26"), ("DATA", "27"), ("FMT", "28")],
    },
    ComponentPins {
        component: "FH12_24S",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/FH12_24S/FH12_24S.ato",
        pins: &[("NC1", "1"), ("GDR", "2"), ("RESE", "3"), ("NC4", "4"), ("VSH2", "5"), ("NC6", "6"), ("NC7", "7"), ("BS1", "8"), ("BUSY", "9"), ("RST", "10"), ("DC", "11"), ("CS", "12"), ("SCL", "13"), ("SDA", "14"), ("VDDIO", "15"), ("VCI", "16"), ("VSS", "17"), ("VDD", "18"), ("VPP", "19"), ("VSH1", "20"), ("VGH", "21"), ("VSL", "22"), ("VGL", "23"), ("VCOM", "24")],
    },
    ComponentPins {
        component: "LT3042EMSE",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/LT3042EMSE/LT3042EMSE.ato",
        pins: &[("IN", "1"), ("EN", "2"), ("SET", "3"), ("ILIM", "4"), ("BYP", "5"), ("PG", "6"), ("GND", "7"), ("OUT", "8"), ("EP", "9")],
    },
    ComponentPins {
        component: "LT3045EMSE",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/LT3045EMSE/LT3045EMSE.ato",
        pins: &[("IN", "1"), ("IN_2", "2"), ("VIOC", "3"), ("EN_UV", "4"), ("PG", "5"), ("ILIM", "6"), ("PGFB", "7"), ("SET", "8"), ("GND", "9"), ("OUTS", "10"), ("OUT", "11"), ("OUT_2", "12"), ("EP", "13")],
    },
    ComponentPins {
        component: "LT3094EMSE",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/LT3094EMSE/LT3094EMSE.ato",
        pins: &[("IN", "1"), ("IN_2", "2"), ("EN_UV", "3"), ("PG", "4"), ("PGFB", "5"), ("ILIM", "6"), ("VIOC", "7"), ("SET", "8"), ("GND", "9"), ("OUTS", "10"), ("OUT", "11"), ("OUT_2", "12"), ("EP", "13")],
    },
    ComponentPins {
        component: "MBR0530",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/MBR0530/MBR0530.ato",
        pins: &[("K", "1"), ("A", "2")],
    },
    ComponentPins {
        component: "PESD5V0L5UY",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/PESD5V0L5UY/PESD5V0L5UY.ato",
        pins: &[("IO1", "1"), ("GND", "2"), ("IO2", "3"), ("IO3", "4"), ("IO4", "5"), ("IO5", "6")],
    },
    ComponentPins {
        component: "PTS526",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/PTS526/PTS526.ato",
        pins: &[("A1", "1"), ("B1", "2")],
    },
    ComponentPins {
        component: "S2B_PH_K_S",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/S2B_PH_K_S/S2B_PH_K_S.ato",
        pins: &[("BAT_POS", "1"), ("BAT_NEG", "2")],
    },
    ComponentPins {
        component: "STM32H743ZIT6",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/STM32H743ZIT6/STM32H743ZIT6.ato",
        pins: &[("SAI1_MCLK_A", "1"), ("LCD_BUSY", "2"), ("SAI1_LRCLK_A", "3"), ("SAI1_BCLK_A", "4"), ("SAI1_SD_A", "5"), ("VBAT", "6"), ("BTN_POWER", "7"), ("FMC_A0", "10"), ("FMC_A1", "11"), ("FMC_A2", "12"), ("FMC_A3", "13"), ("FMC_A4", "14"), ("FMC_A5", "15"), ("VSS_1", "16"), ("VDD_1", "17"), ("QUADSPI_BK1_IO2", "19"), ("NRST", "25"), ("FMC_SDWE", "26"), ("FMC_SDNE0", "28"), ("FMC_SDCKE0", "29"), ("VDD_2", "30"), ("VSSA", "31"), ("VREF_PLUS", "32"), ("VDDA", "33"), ("BTN_PLAY", "34"), ("BTN_NEXT", "35"), ("BTN_PREV", "36"), ("ENC_B", "37"), ("VSS_2", "38"), ("VDD_3", "39"), ("SPI1_NSS", "40"), ("SPI1_SCK", "41"), ("SPI1_MISO", "42"), ("SPI1_MOSI", "43"), ("AMP_EN", "44"), ("LCD_DC", "46"), ("LCD_RST", "47"), ("QUADSPI_CLK", "48"), ("FMC_SDRAS", "49"), ("FMC_A6", "50"), ("VSS_3", "51"), ("VDD_4", "52"), ("FMC_A7", "53"), ("FMC_A8", "54"), ("FMC_A9", "55"), ("FMC_A10", "56"), ("FMC_A11", "57"), ("FMC_D4", "58"), ("FMC_D5", "59"), ("FMC_D6", "60"), ("VSS_4", "61"), ("VDD_5", "62"), ("FMC_D7", "63"), ("FMC_D8", "64"), ("FMC_D9", "65"), ("FMC_D10", "66"), ("FMC_D11", "67"), ("FMC_D12", "68"), ("PH0_OSC_IN", "69"), ("PH1_OSC_OUT", "70"), ("VCAP1", "71"), ("VDD_6", "72"), ("FMC_D13", "77"), ("FMC_D14", "78"), ("FMC_D15", "79"), ("QUADSPI_BK1_IO0", "80"), ("QUADSPI_BK1_IO1", "81"), ("QUADSPI_BK1_IO3", "82"), ("VSS_5", "83"), ("VDD_7", "84"), ("FMC_D0", "85"), ("FMC_D1", "86"), ("FMC_A12", "87"), ("FMC_BA0", "89"), ("FMC_BA1", "90"), ("FMC_SDCLK", "93"), ("VSS_6", "94"), ("VDD33_USB", "95"), ("SDMMC1_D0", "98"), ("SDMMC1_D1", "99"), ("ENC_A", "100"), ("UART1_TX", "101"), ("UART1_RX", "102"), ("USB_OTG_HS_DM", "103"), ("USB_OTG_HS_DP", "104"), ("SWDIO", "105"), ("VCAP2", "106"), ("VSS_7", "107"), ("VDD_8", "108"), ("SWDCLK", "109"), ("SDMMC1_D2", "111"), ("SDMMC1_D3", "112"), ("SDMMC1_CLK", "113"), ("FMC_D2", "114"), ("FMC_D3", "115"), ("SDMMC1_CMD", "116"), ("BTN_MENU", "117"), ("BTN_BACK", "118"), ("BTN_SEL", "119"), ("VSS_8", "120"), ("VDD_9", "121"), ("VSS_9", "130"), ("VDD_10", "131"), ("FMC_SDCAS", "132"), ("SWO", "133"), ("ENC_SW", "135"), ("QUADSPI_BK1_NCS", "136"), ("BOOT0", "138"), ("I2C1_SCL", "139"), ("I2C1_SDA", "140"), ("FMC_NBL0", "141"), ("FMC_NBL1", "142"), ("VDD_11", "144")],
    },
    ComponentPins {
        component: "STM32WB55RGV6",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/STM32WB55RGV6/STM32WB55RGV6.ato",
        pins: &[("VBAT", "1"), ("PC14_OSC32_IN", "3"), ("PC15_OSC32_OUT", "4"), ("NRST_WB", "8"), ("VREFPLUS", "13"), ("VDDA", "14"), ("UART1_TX", "24"), ("VDD", "30"), ("RF_ANT", "31"), ("VSSRF", "32"), ("VDDRF", "33"), ("OSC_OUT", "34"), ("OSC_IN", "35"), ("VFBSMPS", "41"), ("VSSSMPS", "42"), ("VLXSMPS", "43"), ("VDDSMPS", "44"), ("VDD_2", "45"), ("UART1_RX", "51"), ("SWDIO_WB", "54"), ("VDDUSB", "55"), ("SWDCLK_WB", "56"), ("VDD_3", "68"), ("EP", "69")],
    },
    ComponentPins {
        component: "Si1308EDL",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/Si1308EDL/Si1308EDL.ato",
        pins: &[("G", "1"), ("S", "2"), ("D", "3")],
    },
    ComponentPins {
        component: "TEXAS_INSTRUMENTS_SN74LVC1G3157DCKR_package",
        manufacturer: Some("TEXAS INSTRUMENTS"),
        mpn: Some("SN74LVC1G3157DCKR"),
        source: "hardware/elec/src/parts/TEXAS_INSTRUMENTS_SN74LVC1G3157DCKR/TEXAS_INSTRUMENTS_SN74LVC1G3157DCKR.ato",
        pins: &[],
    },
    ComponentPins {
        component: "TPA6120A2",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/TPA6120A2/TPA6120A2.ato",
        pins: &[("LIN_POS", "1"), ("LIN_NEG", "2"), ("RIN_NEG", "6"), ("RIN_POS", "7"), ("RVCC_POS", "8"), ("ROUT", "9"), ("RVCC_NEG", "10"), ("LVCC_NEG", "12"), ("LOUT", "13"), ("LVCC_POS", "14"), ("EP", "15")],
    },
    ComponentPins {
        component: "TPS61023DRLR",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/TPS61023DRLR/TPS61023DRLR.ato",
        pins: &[("FB", "1"), ("EN", "2"), ("VIN", "3"), ("GND", "4"), ("SW", "5"), ("VOUT", "6")],
    },
    ComponentPins {
        component: "TPS63700DRCR",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/TPS63700DRCR/TPS63700DRCR.ato",
        pins: &[("COMP", "1"), ("GND", "2"), ("VIN", "3"), ("EN", "4"), ("IN", "5"), ("SW", "6"), ("PS_GND", "7"), ("OUT", "8"), ("FB", "9"), ("VREF", "10"), ("EP", "11")],
    },
    ComponentPins {
        component: "W25Q128JVSIQ",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/W25Q128JVSIQ/W25Q128JVSIQ.ato",
        pins: &[("CS_NEG", "1"), ("IO1", "2"), ("IO2", "3"), ("GND", "4"), ("IO0", "5"), ("CLK", "6"), ("IO3", "7"), ("VCC", "8")],
    },
    ComponentPins {
        component: "W9825G6KH6",
        manufacturer: None,
        mpn: None,
        source: "hardware/elec/src/parts/W9825G6KH6/W9825G6KH6.ato",
        pins: &[("VDD_1", "1"), ("DQ0", "2"), ("VDDQ_1", "3"), ("VSSQ_1", "6"), ("DQ3", "7"), ("DQ6", "11"), ("DQML", "15"), ("WE_NEG", "16"), ("CAS_NEG", "17"), ("RAS_NEG", "18"), ("CS_NEG", "19"), ("BA0", "20"), ("BA1", "21"), ("A0", "23"), ("A3", "26"), ("VSS_1", "28"), ("A6", "31"), ("A9", "34"), ("A12", "36"), ("CKE", "37"), ("CLK", "38"), ("DQMH", "39"), ("NC_1", "40"), ("DQ8", "42"), ("DQ11", "47"), ("VDDQ_4", "49"), ("DQ14", "51"), ("VSSQ_4", "52")],
    },
];

/// Look a component up by its `.ato` block name.
#[must_use]
pub fn component(name: &str) -> Option<&'static ComponentPins> {
    let mut i = 0;
    while i < COMPONENTS.len() {
        if str_eq(COMPONENTS[i].component, name) {
            return Some(&COMPONENTS[i]);
        }
        i += 1;
    }
    None
}
