mod mmap;

use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Stdin, Stdout, Write},
    path::PathBuf,
};

use clap::Parser;
use possum_emu::{CFCard, Device, DeviceBus, System};
use termion::raw::{IntoRawMode, RawTerminal};

use crate::mmap::MemoryMapWrapper;

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
    /// Path to ROM file to load
    #[clap(parse(from_os_str), value_name = "ROM")]
    file: PathBuf,

    /// Path to disk image for the primary drive
    #[clap(parse(from_os_str), long)]
    hd0: Option<PathBuf>,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let mut rom = Vec::new();
    File::open(args.file)?.read_to_end(&mut rom)?;

    let hd0: Option<Box<dyn Device>> = if let Some(path) = args.hd0 {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let mmap = MemoryMapWrapper::new(file)?;
        Some(Box::new(CFCard::primary(mmap)))
    } else {
        None
    };

    let mut system = System::new(Box::new(Stdio::new()?), hd0);
    system.write_ram(&rom, 0);

    loop {
        system.step();
    }
}
