#![feature(io_error_other)]

mod kb;
mod mmap;

use std::{
    fs::{File, OpenOptions},
    io::{self, Read},
    path::PathBuf,
};

use clap::Parser;
use possum_emu::{CardBus, Device, System};
use sdl2::pixels::PixelFormatEnum;

use crate::{kb::AsciiKeyboard, mmap::MemoryMapWrapper};

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
        // .window("possum-emu", 720, 264 * 2)
        .window("possum-emu", 952, 260 * 2)
        .allow_highdpi()
        .position_centered()
        .resizable()
        .build()
        .map_err(io::Error::other)?;
    let mut canvas = window
        .into_canvas()
        .accelerated()
        .build()
        .map_err(io::Error::other)?;
    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        // .create_texture_streaming(PixelFormatEnum::RGBA32, 720, 264)
        .create_texture_streaming(PixelFormatEnum::RGBA32, 952, 260)
        .map_err(io::Error::other)?;

    while !system.halted() {
        system.step();
        if system.vblank() {
            texture
                .with_lock(None, |pixels, _| {
                    pixels.copy_from_slice(bytemuck::cast_slice(system.framebuffer()));
                })
                .map_err(io::Error::other)?;
            canvas
                .copy(&texture, None, None)
                .map_err(io::Error::other)?;
            canvas.present();
        }
    }

    Ok(())
}
