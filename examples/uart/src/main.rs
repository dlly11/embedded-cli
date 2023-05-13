//! Blinks the LED on a Pico board
//!
//! This will blink an LED attached to GP25, which is the pin the Pico uses for the on-board LED.
#![no_std]
#![no_main]

use bsp::entry;
use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

use heapless::String;

//mod serial_setup;

use fugit::RateExtU32;

// Provide an alias for our BSP so we can switch targets quickly.
// Uncomment the BSP you included in Cargo.toml, the rest of the code does not need to change.
use rp_pico as bsp;
// use sparkfun_pro_micro_rp2040 as bsp;

use bsp::hal::{
    clocks::{init_clocks_and_plls, Clock},
    pac,
    sio::Sio,
    uart::{DataBits, StopBits, UartConfig},
    watchdog::Watchdog,
};

use cli::Cli;
use embedded_cli as cli;

fn printer_demo<'a>(
    writer: Option<&mut (dyn core::fmt::Write + 'a)>,
) -> Result<cli::ReturnCode, cli::CommandProcessorError> {
    if let Some(writer) = writer {
        writeln!(writer, "Hello").map_err(|_| cli::CommandProcessorError::WriteError)?;
    }
    Ok(cli::ReturnCode::Success)
}

#[entry]
fn main() -> ! {
    info!("Program start");
    let mut pac = pac::Peripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let sio = Sio::new(pac.SIO);

    // External high-speed crystal on the pico board is 12Mhz
    let external_xtal_freq_hz = 12_000_000u32;
    let clocks = init_clocks_and_plls(
        external_xtal_freq_hz,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let pins = bsp::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let uart_pins = (
        // UART TX (characters sent from RP2040) on pin 1 (GPIO0)
        pins.gpio0.into_mode::<bsp::hal::gpio::FunctionUart>(),
        // UART RX (characters received by RP2040) on pin 2 (GPIO1)
        pins.gpio1.into_mode::<bsp::hal::gpio::FunctionUart>(),
    );

    // Make a UART on the given pins
    let mut uart = bsp::hal::uart::UartPeripheral::new(pac.UART0, uart_pins, &mut pac.RESETS)
        .enable(
            UartConfig::new(115200.Hz(), DataBits::Eight, None, StopBits::One),
            clocks.peripheral_clock.freq(),
        )
        .unwrap();

    let mut cli = Cli::<8, 32>::new();

    let result = cli.add_command(
        String::<32>::from("test"),
        printer_demo,
        Some(String::<32>::from("Test Help")),
    );

    match result {
        Ok(()) => (),
        Err(_) => info!("Error adding command"),
    };

    match cli.init(&mut uart) {
        Ok(()) => (),
        Err(_) => info!("Error initializing CLI"),
    }

    info!("Entering loop");
    loop {
        // Check for new data
        cli.run(&mut uart);
    }
}

// End of file
