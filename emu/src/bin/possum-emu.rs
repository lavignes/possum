use std::{
    fs::File,
    io::{self, Read, Stdin, Stdout, Write},
    path::PathBuf,
};

use clap::Parser;
use possum_emu::{Device, DeviceBus, System};
use termion::raw::{IntoRawMode, RawTerminal};

struct Stdio {
    stdin: Stdin,
    stdout: RawTerminal<Stdout>,
}

impl Stdio {
    fn new() -> io::Result<Self> {
        Ok(Self {
            stdin: io::stdin(),
            stdout: io::stdout().into_raw_mode()?,
        })
    }
}

impl Device for Stdio {
    fn tick(&mut self, _: &mut dyn DeviceBus) {}

    fn read(&mut self, _: u16) -> u8 {
        let mut buf = [0; 1];
        match self.stdin.read(&mut buf) {
            Ok(1) => buf[0],
            _ => 0,
        }
    }

    fn write(&mut self, _: u16, data: u8) {
        self.stdout.write(&[data]).unwrap();
    }

    fn interrupt(&self) -> bool {
        false
    }

    fn interrupt_vector(&self) -> u8 {
        0
    }

    fn ack_interrupt(&mut self) {}
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to ROM file to load.
    #[clap(parse(from_os_str), value_name = "ROM")]
    file: PathBuf,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let mut file = File::open(args.file)?;
    let mut rom = Vec::new();
    file.read_to_end(&mut rom)?;

    let mut system = System::new(Box::new(Stdio::new()?));
    system.write_ram(&rom, 0);

    loop {
        system.step();
    }
}
