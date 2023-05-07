use core::fmt;
use embedded_hal::blocking::serial as bserial;
use embedded_hal::serial;
use heapless::Vec;
use usbd_serial::SerialPort;

pub struct WritableSerialPort<'a, T: usb_device::bus::UsbBus> {
    pub serial_port: SerialPort<'a, T>,
    read_buf: Vec<u8, 64>,
    read_ptr: usize,
}

impl<'a, T: usb_device::bus::UsbBus> WritableSerialPort<'a, T> {
    pub fn new(serial: SerialPort<'a, T>) -> WritableSerialPort<T> {
        WritableSerialPort {
            serial_port: serial,
            read_buf: Vec::new(),
            read_ptr: 0,
        }
    }

    pub fn write_to_read_buff(&mut self, data: &[u8]) {
        self.read_buf = Vec::from_slice(data).unwrap();
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

impl<'a, T: usb_device::bus::UsbBus> serial::Write<u8> for WritableSerialPort<'a, T> {
    type Error = core::fmt::Error;

    fn write(&mut self, s: u8) -> nb::Result<(), Self::Error> {
        match self.serial_port.write(&[s]) {
            Ok(_) => Ok(()),
            Err(_) => Err(nb::Error::Other(fmt::Error)),
        }
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        match self.serial_port.flush() {
            Ok(_) => Ok(()),
            Err(_) => Err(nb::Error::Other(fmt::Error)),
        }
    }
}

impl<'a, T: usb_device::bus::UsbBus> bserial::write::Default<u8> for WritableSerialPort<'a, T> {}

impl<'a, T: usb_device::bus::UsbBus> serial::Read<u8> for WritableSerialPort<'a, T> {
    type Error = core::fmt::Error;

    fn read(&mut self) -> nb::Result<u8, Self::Error> {
        if self.read_ptr < self.read_buf.len() {
            let byte = self.read_buf[self.read_ptr];
            self.read_ptr += 1;
            Ok(byte)
        } else {
            match self.serial_port.read(&mut self.read_buf) {
                Ok(_) => {
                    self.read_ptr = 0;
                    self.read()
                }
                Err(_) => Err(nb::Error::Other(fmt::Error)),
            }
        }
    }
}
