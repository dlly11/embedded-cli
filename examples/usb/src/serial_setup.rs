use core::fmt;
use embedded_hal::serial::{Read, Write};
use usbd_serial::{SerialPort, UsbError};

pub struct WritableSerialPort<'a, T: usb_device::bus::UsbBus> {
    pub serial_port: SerialPort<'a, T>,
}

impl<'a, T: usb_device::bus::UsbBus> WritableSerialPort<'a, T> {
    pub fn new(serial: SerialPort<'a, T>) -> WritableSerialPort<T> {
        WritableSerialPort {
            serial_port: serial,
        }
    }
}

impl<'a, T: usb_device::bus::UsbBus> fmt::Write for WritableSerialPort<'a, T> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        match self.serial_port.write(s.as_bytes()) {
            Ok(_) => Ok(()),
            Err(_) => Err(fmt::Error),
        }
    }
}

impl<'a, T: usb_device::bus::UsbBus> Read<u8> for WritableSerialPort<'a, T> {
    type Error = UsbError;
    fn read(&mut self) -> Result<u8, nb::Error<UsbError>> {
        // Call the read function from the Read trait
        Read::read(&mut self.serial_port)
    }
}

impl<'a, T: usb_device::bus::UsbBus> Write<u8> for WritableSerialPort<'a, T> {
    type Error = UsbError;
    fn write(&mut self, word: u8) -> Result<(), nb::Error<UsbError>> {
        // Call the write function from the Write trait
        Write::write(&mut self.serial_port, word)
    }
    fn flush(&mut self) -> Result<(), nb::Error<UsbError>> {
        // Call the flush function from the Write trait
        Write::flush(&mut self.serial_port)
    }
}
