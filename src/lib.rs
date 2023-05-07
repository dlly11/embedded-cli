#![cfg_attr(not(test), no_std)]

use embedded_hal::serial::{Read, Write};
use heapless::{HistoryBuffer, String};

pub use command_processor::{
    CommandCallback, CommandCallbackReturn, CommandProcessor, CommandProcessorError, ReturnCode,
};

pub enum CliError {
    CommandProcessorError(CommandProcessorError),
    ReadError,
    WriteError,
    ReadBufferError,
    CommandBufferError,
}

pub struct Cli<'a, const NUM_COMMANDS: usize, const HELP_STR_SIZE: usize> {
    command_processor: CommandProcessor<'a, NUM_COMMANDS, HELP_STR_SIZE>,
    prompt: String<32>,
    read_buffer: String<32>,
    command_buffer: String<32>,
    history_buffer: HistoryBuffer<String<32>, 8>,
    history_buffer_idx: usize,
}

impl<'a, const NUM_COMMANDS: usize, const HELP_STR_SIZE: usize> Default
    for Cli<'a, NUM_COMMANDS, HELP_STR_SIZE>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, const NUM_COMMANDS: usize, const HELP_STR_SIZE: usize>
    Cli<'a, NUM_COMMANDS, HELP_STR_SIZE>
{
    pub fn new() -> Cli<'a, NUM_COMMANDS, HELP_STR_SIZE> {
        Cli {
            command_processor: CommandProcessor::new(),
            prompt: String::from("cli> "),
            read_buffer: String::new(),
            command_buffer: String::new(),
            history_buffer: HistoryBuffer::new(),
            history_buffer_idx: 0,
        }
    }

    pub fn add_command(
        &mut self,
        command: String<32>,
        callback: CommandCallback<'a>,
        help: Option<String<HELP_STR_SIZE>>,
    ) -> Result<(), CommandProcessorError> {
        self.command_processor.add_command(command, callback, help)
    }

    pub fn remove_command(&mut self, command: String<32>) -> Result<(), CommandProcessorError> {
        self.command_processor.remove_command(command)
    }

    fn process_command(
        &mut self,
        writer: Option<&mut (dyn core::fmt::Write + 'a)>,
    ) -> CommandCallbackReturn<'a> {
        self.command_processor
            .process_command(&self.command_buffer, writer)
    }

    pub fn init<T: Read<u8> + Write<u8> + core::fmt::Write + 'a>(
        &mut self,
        serial: &mut T,
    ) -> Result<(), CliError> {
        match write!(serial, "\r\n{}", self.prompt) {
            Ok(_) => (),
            Err(_) => return Err(CliError::WriteError),
        };

        Ok(())
    }

    pub fn run<T: Read<u8> + Write<u8> + core::fmt::Write + 'a>(
        &mut self,
        serial: &mut T,
    ) -> Result<ReturnCode, CliError> {
        let result = self.process_serial_loop(serial);

        match result {
            Err(CliError::ReadError) => (),
            _ => {
                self.read_buffer.clear();
                self.command_buffer.clear();
            }
        }

        result
    }
    fn handle_default_byte<T: Read<u8> + Write<u8> + core::fmt::Write + 'a>(
        &mut self,
        serial: &mut T,
        byte: u8,
    ) -> Result<(), CliError> {
        if self.read_buffer.len() < 32 {
            self.read_buffer
                .push(byte as char)
                .map_err(|_| CliError::ReadBufferError)?;
            serial.write(byte).map_err(|_| CliError::WriteError)?;
        }
        if byte.is_ascii_alphanumeric() {
            self.command_buffer
                .push(byte as char)
                .map_err(|_| CliError::CommandBufferError)?;
        }
        Ok(())
    }

    fn handle_history<T: Read<u8> + Write<u8> + core::fmt::Write + 'a>(
        &mut self,
        serial: &mut T,
        new_idx: usize,
    ) -> Result<(), CliError> {
        if self.history_buffer.len() > 0 {
            let prev = self.history_buffer.get(self.history_buffer_idx);

            self.history_buffer_idx = new_idx;

            if let Some(prev) = prev {
                let current_read_buffer = self.read_buffer.clone();

                self.read_buffer.clear();
                self.command_buffer.clear();
                self.read_buffer
                    .push_str(prev)
                    .map_err(|_| CliError::ReadBufferError)?;
                self.command_buffer
                    .push_str(prev)
                    .map_err(|_| CliError::CommandBufferError)?;

                for char in current_read_buffer.chars() {
                    if char.is_ascii_alphanumeric() {
                        serial.write(b'\x08').map_err(|_| CliError::WriteError)?;
                    }
                }
                serial.write(b' ').map_err(|_| CliError::WriteError)?;
                for char in self.read_buffer.chars() {
                    serial.write(char as u8).map_err(|_| CliError::WriteError)?;
                }
            }
        }

        Ok(())
    }

    fn process_serial_loop<T: Read<u8> + Write<u8> + core::fmt::Write + 'a>(
        &mut self,
        serial: &mut T,
    ) -> Result<ReturnCode, CliError> {
        loop {
            let byte = serial.read().map_err(|_| CliError::ReadError)?;

            match byte {
                // Carriage Return - Time to process the command
                b'\r' => {
                    write!(serial, "\r\n{}", self.prompt).map_err(|_| CliError::WriteError)?;

                    let result = self
                        .process_command(Some(serial))
                        .map_err(CliError::CommandProcessorError)?;

                    self.history_buffer.write(self.command_buffer.clone());
                    self.history_buffer_idx = self.history_buffer.len() - 1;

                    write!(serial, "\r\n{}", self.prompt).map_err(|_| CliError::WriteError)?;

                    return Ok(result);
                }
                b'\n' => write!(serial, "\r\n{}", self.prompt).map_err(|_| CliError::WriteError)?,

                // ASCII Backspace
                b'\x08' => {
                    if self.read_buffer.pop().is_some() {
                        write!(serial, "\x08 \x08").map_err(|_| CliError::WriteError)?;
                    }

                    self.command_buffer.pop();
                }

                // ASCII up arrow
                b'A' => {
                    // Check if last two characters were escape and [

                    let buffer_len = self.read_buffer.len();

                    let last_two = self.read_buffer.get(buffer_len - 2..buffer_len);

                    if last_two.is_some() && last_two == Some("\x1B[") {
                        let new_idx = if self.history_buffer_idx > 0 {
                            self.history_buffer_idx - 1
                        } else {
                            0
                        };
                        self.handle_history(serial, new_idx)?;
                    } else {
                        self.handle_default_byte(serial, byte)?;
                    }
                }

                // ASCII down arrow
                b'B' => {
                    let buffer_len = self.read_buffer.len();

                    let last_two = self.read_buffer.get(buffer_len - 2..buffer_len);

                    if last_two.is_some() && last_two == Some("\x1B[") {
                        let new_idx = if self.history_buffer_idx < self.history_buffer.len() - 1 {
                            self.history_buffer_idx + 1
                        } else {
                            self.history_buffer.len() - 1
                        };
                        self.handle_history(serial, new_idx)?;
                    } else {
                        self.handle_default_byte(serial, byte)?;
                    }
                }

                // Default case is to echo the character back to the terminal
                _ => {
                    self.handle_default_byte(serial, byte)?;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {

    mod serialmock {
        use embedded_hal::serial::{Read, Write};
        use heapless::Vec;

        pub struct SerialMock {
            read_buffer: Vec<u8, 512>,
            write_buffer: Vec<u8, 512>,
            read_ptr: usize,
            write_ptr: usize,
        }

        impl SerialMock {
            pub fn new() -> SerialMock {
                SerialMock {
                    read_buffer: Vec::new(),
                    write_buffer: Vec::new(),
                    read_ptr: 0,
                    write_ptr: 0,
                }
            }

            pub fn get_buffer(&mut self) -> &mut Vec<u8, 512> {
                &mut self.read_buffer
            }

            pub fn read_from_write_buffer(&mut self) -> &mut Vec<u8, 512> {
                &mut self.write_buffer
            }

            pub fn write_to_read_buffer(&mut self, bytes: &[u8]) {
                for byte in bytes {
                    match self.read_buffer.push(*byte) {
                        Ok(_) => (),
                        Err(_) => (),
                    }
                }
            }
        }

        impl Read<u8> for SerialMock {
            type Error = ();

            fn read(&mut self) -> nb::Result<u8, Self::Error> {
                match self.read_buffer.get(self.read_ptr) {
                    Some(byte) => {
                        if self.read_ptr == self.read_buffer.len() - 1 {
                            self.read_ptr = 0;
                        } else {
                            self.read_ptr += 1;
                        }
                        Ok(*byte)
                    }
                    None => Err(nb::Error::WouldBlock),
                }
            }
        }

        impl Write<u8> for SerialMock {
            type Error = ();

            fn write(&mut self, byte: u8) -> nb::Result<(), Self::Error> {
                match self.write_buffer.push(byte) {
                    Ok(_) => {
                        if self.write_ptr == self.write_buffer.len() - 1 {
                            self.write_ptr = 0;
                        } else {
                            self.write_ptr += 1;
                        }
                        Ok(())
                    }
                    Err(_) => Err(nb::Error::WouldBlock),
                }
            }

            fn flush(&mut self) -> nb::Result<(), Self::Error> {
                Ok(())
            }
        }

        impl core::fmt::Write for SerialMock {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                for byte in s.bytes() {
                    match self.write_buffer.push(byte) {
                        Ok(_) => {
                            if self.write_ptr == self.write_buffer.len() - 1 {
                                self.write_ptr = 0;
                            } else {
                                self.write_ptr += 1;
                            }
                            ()
                        }
                        Err(_) => return Err(core::fmt::Error),
                    }
                }

                Ok(())
            }
        }
    }

    use super::*;

    #[test]
    fn test_init() {
        let mut cli = Cli::<8, 32>::new();

        let mut serial = serialmock::SerialMock::new();

        assert!(cli.init(&mut serial).is_ok());
        assert_eq!(
            std::string::String::from_utf8(serial.read_from_write_buffer().to_vec()).unwrap(),
            "\r\ncli> "
        );
    }

    #[test]
    fn test_add_remove_command() {
        let mut cli = Cli::<8, 32>::new();

        cli.add_command(
            String::from("test"),
            |writer| {
                match writer {
                    Some(writer) => {
                        match write!(writer, "Write this to the serial port") {
                            Ok(_) => (),
                            Err(_) => return Err(CommandProcessorError::WriteError),
                        };
                    }
                    None => (),
                };

                Ok(ReturnCode::Success)
            },
            Some(String::from("test command")),
        )
        .unwrap();

        assert!(cli.remove_command(String::from("test")).is_ok());
    }

    #[test]
    fn test_remove_unknown_command() {
        let mut cli = Cli::<8, 32>::new();

        assert!(cli.remove_command(String::from("test")).is_err());
    }

    #[test]
    fn test_add_command_too_many() {
        let mut cli = Cli::<2, 32>::new();

        cli.add_command(
            String::from("test"),
            |writer| {
                match writer {
                    Some(writer) => {
                        match write!(writer, "Write this to the serial port") {
                            Ok(_) => (),
                            Err(_) => return Err(CommandProcessorError::WriteError),
                        };
                    }
                    None => (),
                };

                Ok(ReturnCode::Success)
            },
            Some(String::from("test command")),
        )
        .unwrap();

        cli.add_command(
            String::from("test2"),
            |writer| {
                match writer {
                    Some(writer) => {
                        match write!(writer, "test2") {
                            Ok(_) => (),
                            Err(_) => return Err(CommandProcessorError::WriteError),
                        };
                    }
                    None => (),
                };

                Ok(ReturnCode::Success)
            },
            Some(String::from("test2 command")),
        )
        .unwrap();

        assert!(cli
            .add_command(
                String::from("test3"),
                |writer| {
                    match writer {
                        Some(writer) => {
                            match write!(writer, "test3") {
                                Ok(_) => (),
                                Err(_) => return Err(CommandProcessorError::WriteError),
                            };
                        }
                        None => (),
                    };

                    Ok(ReturnCode::Success)
                },
                Some(String::from("test3 command")),
            )
            .is_err());
    }

    #[test]
    fn test_process_command() {
        let mut cli = Cli::<8, 32>::new();

        cli.add_command(
            String::from("test"),
            |writer| {
                match writer {
                    Some(writer) => {
                        match write!(writer, "Write this to the serial port") {
                            Ok(_) => (),
                            Err(_) => return Err(CommandProcessorError::WriteError),
                        };
                    }
                    None => (),
                };

                Ok(ReturnCode::Success)
            },
            Some(String::from("test command")),
        )
        .unwrap();

        cli.add_command(
            String::from("test2"),
            |writer| {
                match writer {
                    Some(writer) => {
                        match write!(writer, "test2") {
                            Ok(_) => (),
                            Err(_) => return Err(CommandProcessorError::WriteError),
                        };
                    }
                    None => (),
                };

                Ok(ReturnCode::Success)
            },
            Some(String::from("test2 command")),
        )
        .unwrap();

        let mut serial = serialmock::SerialMock::new();

        let test_str = "test\r\ntest2\r\n";

        serial.write_to_read_buffer(test_str.as_bytes());

        assert!(cli.init(&mut serial).is_ok());

        let result = cli.run(&mut serial);

        assert!(result.is_ok());

        let bytes = serial.read_from_write_buffer();

        // Convert bytes to string
        let string = std::string::String::from_utf8(bytes.to_vec()).unwrap();

        assert_eq!(
            string,
            "\r\ncli> test\r\ncli> Write this to the serial port\r\ncli> "
        );

        let bytes = serial.get_buffer();

        // Convert bytes to string
        let string = std::string::String::from_utf8(bytes.to_vec()).unwrap();

        assert_eq!(string, "test\r\ntest2\r\n");
    }

    #[test]
    fn test_history() {
        let mut cli = Cli::<8, 32>::new();

        cli.add_command(
            String::from("test"),
            |writer| {
                match writer {
                    Some(writer) => {
                        match write!(writer, "test") {
                            Ok(_) => (),
                            Err(_) => return Err(CommandProcessorError::WriteError),
                        };
                    }
                    None => (),
                };

                Ok(ReturnCode::Success)
            },
            Some(String::from("test command")),
        )
        .unwrap();

        let mut serial = serialmock::SerialMock::new();

        // test string with ascii up arrow
        let test_str = "test\r\n\x1B[A\r\n";

        serial.write_to_read_buffer(test_str.as_bytes());

        assert!(cli.init(&mut serial).is_ok());

        let result = cli.run(&mut serial);

        assert!(result.is_ok());

        let bytes = serial.read_from_write_buffer();

        // Convert bytes to string
        let string = std::string::String::from_utf8(bytes.to_vec()).unwrap();

        assert_eq!(string, "\r\ncli> test\r\ncli> test\r\ncli> ");

        let bytes = serial.get_buffer();

        // Convert bytes to string
        let string = std::string::String::from_utf8(bytes.to_vec()).unwrap();

        assert_eq!(string, "test\r\n\x1b[A\r\n");
    }

    #[test]
    fn test_backspace() {
        let mut cli = Cli::<8, 32>::new();

        cli.add_command(
            String::from("test"),
            |writer| {
                match writer {
                    Some(writer) => {
                        match write!(writer, "hello") {
                            Ok(_) => (),
                            Err(_) => return Err(CommandProcessorError::WriteError),
                        };
                    }
                    None => (),
                };

                Ok(ReturnCode::Success)
            },
            Some(String::from("test command")),
        )
        .unwrap();

        let test_str = "testt\x08\r\n";

        let mut serial = serialmock::SerialMock::new();

        serial.write_to_read_buffer(test_str.as_bytes());

        assert!(cli.init(&mut serial).is_ok());

        let result = cli.run(&mut serial);

        assert!(result.is_ok());

        let bytes = serial.read_from_write_buffer();

        // Convert bytes to string
        let string = std::string::String::from_utf8(bytes.to_vec()).unwrap();

        assert_eq!(string, "\r\ncli> testt\x08 \x08\r\ncli> hello\r\ncli> ");
    }
}
