#![feature(io_error_other)]

mod mmap;

use std::{
    collections::VecDeque,
    fs::{File, OpenOptions},
    io::{self, Read, Stdout, Write},
    path::PathBuf,
};

use clap::Parser;
use possum_emu::{CardBus, Device, DeviceBus, System};
use sdl2::{event::Event, EventPump, Sdl};

use crate::mmap::MemoryMapWrapper;

/// ASCII Parallel Keyboard Emulation
struct AsciiKeyboard {
    event_pump: EventPump,
    buffer: VecDeque<u8>,
}

impl AsciiKeyboard {
    fn new(event_pump: EventPump) -> Self {
        Self {
            event_pump,
            buffer: VecDeque::new(),
        }
    }
}

impl Device for AsciiKeyboard {
    fn tick(&mut self, _: &mut dyn DeviceBus) {}

    fn read(&mut self, _: u16) -> u8 {
        match self.event_pump.poll_event() {
            Some(Event::TextInput { text, .. }) => self.buffer.extend(text.bytes()),
            _ => {}
        }
        self.buffer.pop_front().unwrap_or(0)
    }

    fn write(&mut self, _: u16, data: u8) {
        // TODO: This is a basic output for debugging. Obviously in reality
        //   you can't write to your keyboard :-P
        io::stdout().write(&[data]).unwrap();
    }

    fn interrupting(&self) -> bool {
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

    let hd: Option<Box<dyn Device>> = if let Some(path) = args.hd0 {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let mmap = MemoryMapWrapper::new(file)?;
        Some(Box::new(CardBus::single(mmap)))
    } else {
        None
    };

    let sdl = sdl2::init().map_err(io::Error::other)?;
    let event_pump = sdl.event_pump().map_err(io::Error::other)?;
    let video = sdl.video().map_err(io::Error::other)?;
    video.text_input().start();

    let mut system = System::new(Box::new(AsciiKeyboard::new(event_pump)), hd);
    system.write_ram(&rom, 0);

    let window = video
        .window("possum-emu", 720, 486)
        .allow_highdpi()
        .position_centered()
        .build()
        .map_err(io::Error::other);

    while !system.halted() {
        system.step();
    }

    Ok(())
}
