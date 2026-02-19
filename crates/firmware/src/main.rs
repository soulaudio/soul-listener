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
use embassy_time::{Duration, Timer};
use platform::DisplayDriver;

use firmware::input::builder::InputBuilder;
use firmware::input::hardware::spawn_input_task;
use firmware::ui::{SplashScreen, TestPattern};
use firmware::{DapDisplay, Ssd1677Display, FRAMEBUFFER_SIZE};

// Panic handler
use panic_probe as _;

// Framebuffer stored in AXI SRAM (large buffer region)
#[link_section = ".axisram"]
static mut FRAMEBUFFER: [u8; FRAMEBUFFER_SIZE] = [0xFF; FRAMEBUFFER_SIZE];

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize Embassy
    defmt::info!("SoulAudio DAP Firmware v0.1.0");
    defmt::info!("Initializing STM32H743ZI...");

    let p = embassy_stm32::init(Default::default());

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
    let busy = Input::new(p.PB3, Pull::None); // Busy status

    // Create display driver
    defmt::info!("Creating SSD1677 display driver");
    let mut display = Ssd1677Display::new(spi, dc, cs, rst, busy);

    // Initialize display
    defmt::info!("Initializing display...");
    match display.init().await {
        Ok(_) => defmt::info!("Display initialized successfully"),
        Err(e) => {
            defmt::error!("Display initialization failed: {:?}", e);
            loop {
                Timer::after(Duration::from_secs(1)).await;
            }
        }
    }

    // Show splash screen
    defmt::info!("Rendering splash screen");
    if let Err(e) = SplashScreen::render(&mut display) {
        defmt::error!("Failed to render splash screen: {:?}", e);
    }

    // Trigger full refresh to show splash screen
    if let Err(e) = display.refresh_full().await {
        defmt::error!("Failed to refresh display: {:?}", e);
    }

    defmt::info!("Splash screen displayed");

    // -----------------------------------------------------------------------
    // Wire input task
    //
    // Pin assignments:
    //   PE9  = Encoder CLK (A) — EXTI9 rising-edge interrupt
    //   PE11 = Encoder DT  (B) — GPIO input only
    //   PD0  = Play/Pause  — active-low, internal pull-up
    //   PD1  = Next        — active-low, internal pull-up
    //   PD2  = Previous    — active-low, internal pull-up
    //   PD3  = Menu        — active-low, internal pull-up
    //   PD4  = Back        — active-low, internal pull-up
    //   PD5  = Select      — active-low, internal pull-up
    // -----------------------------------------------------------------------
    defmt::info!("Spawning input task...");

    // Log builder config at startup so debounce values are visible in RTT.
    let enc_config = InputBuilder::rotary().debounce_ms(20);
    let btn_config = InputBuilder::button(firmware::input::Button::Play).debounce_ms(50);
    defmt::info!(
        "Input: encoder debounce={}ms  button debounce={}ms",
        enc_config.debounce(),
        btn_config.debounce()
    );

    // Build ExtiInput pins: Input::new().degrade() + EXTI channel.degrade()
    // gives ExtiInput<'static, AnyPin> compatible with the task signature.
    let enc_clk: ExtiInput<'static, AnyPin> =
        ExtiInput::new(Input::new(p.PE9, Pull::None).degrade(), p.EXTI9.degrade());
    let enc_dt: Input<'static, AnyPin> = Input::new(p.PE11, Pull::None).degrade();

    let btn_play: ExtiInput<'static, AnyPin> =
        ExtiInput::new(Input::new(p.PD0, Pull::Up).degrade(), p.EXTI0.degrade());
    let btn_next: ExtiInput<'static, AnyPin> =
        ExtiInput::new(Input::new(p.PD1, Pull::Up).degrade(), p.EXTI1.degrade());
    let btn_prev: ExtiInput<'static, AnyPin> =
        ExtiInput::new(Input::new(p.PD2, Pull::Up).degrade(), p.EXTI2.degrade());
    let btn_menu: ExtiInput<'static, AnyPin> =
        ExtiInput::new(Input::new(p.PD3, Pull::Up).degrade(), p.EXTI3.degrade());
    let btn_back: ExtiInput<'static, AnyPin> =
        ExtiInput::new(Input::new(p.PD4, Pull::Up).degrade(), p.EXTI4.degrade());
    let btn_select: ExtiInput<'static, AnyPin> =
        ExtiInput::new(Input::new(p.PD5, Pull::Up).degrade(), p.EXTI5.degrade());

    spawn_input_task(
        &spawner, enc_clk, enc_dt, btn_play, btn_next, btn_prev, btn_menu, btn_back, btn_select,
    );
    defmt::info!("Input task spawned");

    // Wait 3 seconds
    Timer::after(Duration::from_secs(3)).await;

    // Show test pattern
    defmt::info!("Rendering test pattern");
    if let Err(e) = TestPattern::render(&mut display) {
        defmt::error!("Failed to render test pattern: {:?}", e);
    }

    // Trigger full refresh
    if let Err(e) = display.refresh_full().await {
        defmt::error!("Failed to refresh display: {:?}", e);
    }

    defmt::info!("Test pattern displayed");

    // Main loop - heartbeat
    defmt::info!("Entering main loop");
    let mut counter = 0u32;

    loop {
        Timer::after(Duration::from_secs(1)).await;
        counter += 1;
        defmt::debug!("Heartbeat: {}", counter);
    }
}
