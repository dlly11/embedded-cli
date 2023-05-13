//! Blinks the LED on a Pico board
//!
//! This will blink an LED attached to GP25, which is the pin the Pico uses for the on-board LED.
#![no_std]
#![no_main]

use bsp::entry;
use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

// USB Device support
use usb_device::{class_prelude::*, prelude::*};

// USB Communications Class Device support
use usbd_serial::{SerialPort, USB_CLASS_CDC};

use heapless::String;

mod serial_setup;

// Provide an alias for our BSP so we can switch targets quickly.
// Uncomment the BSP you included in Cargo.toml, the rest of the code does not need to change.
use rp_pico as bsp;
// use sparkfun_pro_micro_rp2040 as bsp;

use bsp::hal::{clocks::init_clocks_and_plls, pac, usb::UsbBus, watchdog::Watchdog};

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

    // Set up the USB driver
    let usb_bus = UsbBusAllocator::new(UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    ));

    // Set up the USB Communications Class Device driver
    let binding = SerialPort::new(&usb_bus);
    let mut serial = serial_setup::WritableSerialPort::new(binding);

    // Create a USB device with a fake VID and PID
    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .manufacturer("Fake company")
        .product("Serial port")
        .serial_number("TEST")
        .device_class(USB_CLASS_CDC) // from: https://www.usb.org/defined-class-codes
        .build();

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

    match cli.init(&mut serial) {
        Ok(()) => (),
        Err(_) => info!("Error initializing CLI"),
    }

    info!("Entering loop");
    loop {
        if usb_dev.poll(&mut [&mut serial.serial_port]) {
            cli.run(&mut serial);
        }
    }
}

// End of file
