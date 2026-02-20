//! SoulAudio DAP Firmware - Main Entry Point
//!
//! Hardware-only entry point for STM32H743ZI.

#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_stm32::exti::{Channel, ExtiInput};
use embassy_stm32::gpio::{AnyPin, Input, Level, Output, Pull, Speed};
use embassy_stm32::spi::{Config as SpiConfig, Spi};
use embassy_stm32::time::Hertz;
use embassy_time::{Delay, Duration, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use platform::DisplayDriver;

use firmware::input::builder::InputBuilder;
use firmware::input::hardware::spawn_input_task;
use firmware::ui::{SplashScreen, TestPattern};
use firmware::{Ssd1677Display, DISPLAY_HEIGHT, DISPLAY_WIDTH, FRAMEBUFFER_SIZE};

// Panic handler
use panic_probe as _;

// Framebuffer stored in AXI SRAM (large buffer region)
#[link_section = ".axisram"]
#[allow(dead_code)] // Reserved memory region; passed to display driver at runtime via raw pointer
static mut FRAMEBUFFER: [u8; FRAMEBUFFER_SIZE] = [0xFF; FRAMEBUFFER_SIZE];

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Step 0: Configure MPU BEFORE embassy_stm32::init() enables D-cache.
    //
    // embassy_stm32::init() enables the Cortex-M7 D-cache on STM32H7. Without
    // MPU configuration first, the cache will serve DMA buffer addresses as
    // cacheable, causing silent data corruption in audio, display, and SD I/O.
    //
    // This call marks AXI SRAM (0x2400_0000, 512 KB) and SRAM4 (0x3800_0000,
    // 64 KB) as non-cacheable before any DMA peripheral is initialised.
    //
    // References: ST AN4838/AN4839, ARM DDI0489F §B3.5.
    // See: firmware::boot::BOOT_SEQUENCE_STEPS for the full ordered sequence.
    firmware::boot::hardware::apply_mpu_config_from_peripherals();

    // Initialize Embassy
    defmt::info!("SoulAudio DAP Firmware v{=str}", "0.1.0");
    defmt::info!("Initializing STM32H743ZI — Cortex-M7 @ 480 MHz");

    let p = embassy_stm32::init(firmware::boot::build_embassy_config());

    // Step 1: Initialize IWDG (Independent Watchdog).
    //
    // The IWDG must be fed every WATCHDOG_TIMEOUT_MS milliseconds or the MCU
    // resets. This catches Embassy task deadlocks and runaway panic loops.
    //
    // The watchdog uses the 32 kHz LSI clock and is independent of the main
    // PLL. Once unleashed, it CANNOT be stopped — the main loop MUST call
    // watchdog.pet() at least once per WATCHDOG_TIMEOUT_MS interval.
    //
    // See: firmware::boot::WATCHDOG_TIMEOUT_MS (8 seconds)
    let mut watchdog = embassy_stm32::wdg::IndependentWatchdog::new(
        p.IWDG1,
        firmware::boot::init_watchdog_config(),
    );
    watchdog.unleash(); // Start watchdog — cannot be stopped after this point
    defmt::info!(
        "IWDG watchdog armed: timeout={=u32}ms",
        firmware::boot::WATCHDOG_TIMEOUT_MS
    );

    // Step 3: Initialize external SDRAM via FMC
    // TODO: call firmware::boot::init_sdram_stub() when FMC API is available.
    // The SDRAM at 0xC0000000 is needed for library cache + audio decode scratch.
    // Sequence: CLK_EN → PALL → AUTO_REFRESH × 2 → LMR → SET_REFRESH_RATE (761)
    // See: crates/firmware/src/boot.rs::init_sdram_stub()
    // See: crates/platform/src/sdram.rs::SdramInitSequence::w9825g6kh6()

    // TODO Step 4: Initialize SDMMC1 for microSD card access.
    // See: firmware::boot::SDMMC_INIT_NOTE for pin assignments and DMA config.
    // Clock source: HSI48 (already enabled in build_embassy_config()).
    // Priority: CRITICAL — SD card needed for music library access.
    // #[cfg(feature = "hardware")]
    // let sdmmc = embassy_stm32::sdmmc::Sdmmc::new_4bit(
    //     p.SDMMC1, Irqs,
    //     p.PC12, // CLK
    //     p.PD2,  // CMD
    //     p.PC8, p.PC9, p.PC10, p.PC11, // D0-D3
    //     Default::default(),
    // );

    // TODO Step 5: Initialize QUADSPI for NOR flash (fonts/icons/OTA staging).
    // See: firmware::boot::QSPI_INIT_NOTE for pin assignments and timing config.
    // Base address: 0x90000000 (mapped in memory.x as QSPI region).
    // Priority: MAJOR — fonts needed for display rendering.
    // Embassy-stm32 issue #3149: memory-mapped (XiP) mode requires PAC writes.
    // See platform::qspi_config for individual register field values.
    // #[cfg(feature = "hardware")]
    // // XiP via PAC: QUADSPI.CCR FMODE=0b11, INSTRUCTION=0xEB, DCYC=4

    // TODO Step 6: Initialize SAI1 for audio output (ES9038Q2M DAC).
    // See: firmware::boot::SAI_INIT_NOTE for pin assignments and DMA config.
    // Priority: CRITICAL — must complete before spawning audio_playback_task.
    // Blocked on: PLL3 configuration for 49.152 MHz MCLK (192 kHz / 256 fs).
    // PLL1Q is currently 200 MHz (SDMMC). SAI needs a dedicated PLL3 branch.
    // DMA buffer must be declared in .axisram (non-cacheable, DMA1-accessible).
    // See: platform::audio_config::SaiAudioConfig::es9038q2m_192khz()
    // #[cfg(feature = "hardware")]
    // let sai = Sai::new_asynchronous_with_mclk(
    //     p.SAI1_A, p.PE5, p.PE6, p.PE4, p.PE2,
    //     p.DMA1_CH0, &mut SAI_DMA_BUF, Irqs, SaiConfig::default(),
    // );

    // TODO Step 7: Initialize I2C2 (BQ25895 PMIC) and I2C3 (ES9038Q2M DAC ctrl).
    // See: firmware::boot::I2C_INIT_NOTE for addresses and pin assignments.
    // Priority: CRITICAL — PMIC must init before battery ops, DAC before volume.
    // See: platform::audio_config::I2cAddresses for 7-bit address constants.
    // See: platform::audio_config::I2cBusAssignment for bus assignments.
    // #[cfg(feature = "hardware")]
    // let i2c2 = I2c::new(p.I2C2, p.PF1, p.PF0, Irqs,
    //     p.DMA1_CH4, p.DMA1_CH5, hz(100_000), I2cConfig::default());
    // #[cfg(feature = "hardware")]
    // let i2c3 = I2c::new(p.I2C3, p.PA8, p.PC9, Irqs,
    //     p.DMA1_CH6, p.DMA1_CH7, hz(400_000), I2cConfig::default());

    // Configure SPI1 for display
    // PA5 (SPI1_SCK), PA7 (SPI1_MOSI)
    let mut spi_config = SpiConfig::default();
    spi_config.frequency = Hertz(4_000_000); // 4 MHz

    let spi = Spi::new(
        p.SPI1, p.PA5,      // SCK
        p.PA7,      // MOSI
        p.PA6,      // MISO (not used but required by HAL)
        p.DMA1_CH0, // TX DMA
        p.DMA1_CH1, // RX DMA
        spi_config,
    );

    // Configure GPIO pins
    let dc = Output::new(p.PB0, Level::Low, Speed::VeryHigh); // Data/Command
    let cs = Output::new(p.PB1, Level::High, Speed::VeryHigh); // Chip Select (active low)
    let rst = Output::new(p.PB2, Level::High, Speed::VeryHigh); // Reset (active low)
    let busy = Input::new(p.PE3, Pull::None); // Busy status

    // Wrap raw SPI bus + CS pin into an SpiDevice (manages CS assertion/deassert).
    // Ssd1677 takes SpiDevice (not SpiBus) so it controls transactions atomically.
    // new() asserts CS HIGH immediately; safe since CS is already high from initialization.
    let spi_device = ExclusiveDevice::new(spi, cs, Delay).expect("CS pin init failed");

    // Create display driver: Ssd1677::new(spi, dc, rst, busy, delay)
    defmt::info!("Creating SSD1677 display driver — SPI @ {=u32}MHz", 4);
    let mut display = Ssd1677Display::new(spi_device, dc, rst, busy, Delay);

    // Initialize display
    defmt::info!(
        "Initializing display ({=u32}x{=u32}, {=u8}bpp)...",
        DISPLAY_WIDTH,
        DISPLAY_HEIGHT,
        2
    );
    match display.init().await {
        Ok(_) => defmt::info!(
            "Display ready: {}x{} GDEM0397T81P (SSD1677)",
            DISPLAY_WIDTH,
            DISPLAY_HEIGHT
        ),
        Err(e) => {
            defmt::error!("Display initialization failed: {}", e);
            loop {
                Timer::after(Duration::from_secs(1)).await;
            }
        }
    }

    // Show splash screen
    defmt::info!("Rendering splash screen");
    if let Err(e) = SplashScreen::render(&mut display) {
        defmt::error!("Failed to render splash screen: {}", e);
    }

    // Trigger full refresh to show splash screen
    if let Err(e) = display.refresh_full().await {
        defmt::error!("Failed to refresh display (full): {}", e);
    }

    defmt::info!("Splash screen displayed — full refresh complete");

    // -----------------------------------------------------------------------
    // Wire input task
    //
    // Pin assignments:
    //   PA8  = Encoder CLK (A) — EXTI8 rising-edge interrupt
    //   PA3  = Encoder DT  (B) — GPIO input only
    //   PA0  = Play/Pause  — active-low, internal pull-up (EXTI0)
    //   PA1  = Next        — active-low, internal pull-up (EXTI1)
    //   PA2  = Previous    — active-low, internal pull-up (EXTI2)
    //   PD3  = Menu        — active-low, internal pull-up (EXTI3)
    //   PD4  = Back        — active-low, internal pull-up (EXTI4)
    //   PD5  = Select      — active-low, internal pull-up (EXTI5)
    // -----------------------------------------------------------------------
    defmt::info!("Spawning input task (rotary encoder + 6 buttons)...");

    // Log builder config at startup so debounce values are visible in RTT.
    let enc_config = InputBuilder::rotary().debounce_ms(20);
    let btn_config = InputBuilder::button(firmware::input::Button::Play).debounce_ms(50);
    defmt::info!(
        "Input: encoder debounce={=u32}ms  button debounce={=u32}ms",
        enc_config.debounce(),
        btn_config.debounce()
    );

    // Build ExtiInput pins: Input::new().degrade() + EXTI channel.degrade()
    // gives ExtiInput<'static, AnyPin> compatible with the task signature.
    let enc_clk: ExtiInput<'static, AnyPin> =
        ExtiInput::new(Input::new(p.PA8, Pull::None).degrade(), p.EXTI8.degrade());
    let enc_dt: Input<'static, AnyPin> = Input::new(p.PA3, Pull::None).degrade();

    let btn_play: ExtiInput<'static, AnyPin> =
        ExtiInput::new(Input::new(p.PA0, Pull::Up).degrade(), p.EXTI0.degrade());
    let btn_next: ExtiInput<'static, AnyPin> =
        ExtiInput::new(Input::new(p.PA1, Pull::Up).degrade(), p.EXTI1.degrade());
    let btn_prev: ExtiInput<'static, AnyPin> =
        ExtiInput::new(Input::new(p.PA2, Pull::Up).degrade(), p.EXTI2.degrade());
    let btn_menu: ExtiInput<'static, AnyPin> =
        ExtiInput::new(Input::new(p.PD3, Pull::Up).degrade(), p.EXTI3.degrade());
    let btn_back: ExtiInput<'static, AnyPin> =
        ExtiInput::new(Input::new(p.PD4, Pull::Up).degrade(), p.EXTI4.degrade());
    let btn_select: ExtiInput<'static, AnyPin> =
        ExtiInput::new(Input::new(p.PD5, Pull::Up).degrade(), p.EXTI5.degrade());

    spawn_input_task(
        &spawner, enc_clk, enc_dt, btn_play, btn_next, btn_prev, btn_menu, btn_back, btn_select,
    );
    defmt::info!("Input task spawned — channel depth={=usize}", 16usize);

    // Wait 3 seconds
    Timer::after(Duration::from_secs(3)).await;

    // Show test pattern
    defmt::info!("Rendering test pattern");
    if let Err(e) = TestPattern::render(&mut display) {
        defmt::error!("Failed to render test pattern: {}", e);
    }

    // Trigger full refresh
    if let Err(e) = display.refresh_full().await {
        defmt::error!("Failed to refresh display (full): {}", e);
    }

    defmt::info!("Test pattern displayed — full refresh complete");

    // Main loop - heartbeat
    defmt::info!("Entering main loop");
    let mut counter = 0u32;

    loop {
        Timer::after(Duration::from_secs(1)).await;
        counter += 1;
        defmt::debug!("Heartbeat tick={=u32}", counter);
        // Feed the IWDG watchdog. Must be called at least once every
        // WATCHDOG_TIMEOUT_MS (8 s). This 1-second heartbeat provides
        // comfortable margin. If this loop stalls, the MCU resets.
        watchdog.pet();
    }
}
