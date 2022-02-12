#![feature(io_error_other)]

mod kb;
mod mmap;

use std::{
    fs::{File, OpenOptions},
    io::{self, Read},
    mem,
    path::PathBuf,
    time::{Duration, Instant},
};

use clap::Parser;
use possum_emu::{CardBus, Device, System};
use sdl2::{pixels::PixelFormatEnum, rect::Rect};

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
        .window("possum-emu", 752 * 2, 244 * 4)
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
        .create_texture_streaming(PixelFormatEnum::RGBA32, 1024, 1024)
        .map_err(io::Error::other)?;

    let mut start = Instant::now();
    let mut last_frame = Instant::now();
    let mut frames = 0;
    let mut cycles = 0;
    while !system.halted() {
        cycles += system.step();
        let now = Instant::now();

        if system.framebuffer_ready() && now.duration_since(last_frame) > Duration::from_millis(16)
        {
            let framebuffer = system.framebuffer();
            let rect = Rect::new(
                0,
                0,
                framebuffer.width() as u32,
                framebuffer.height() as u32,
            );
            texture
                .update(
                    rect,
                    bytemuck::cast_slice(framebuffer.data()),
                    framebuffer.width() * mem::size_of::<u32>(),
                )
                .map_err(io::Error::other)?;
            canvas
                .copy(&texture, rect, None)
                .map_err(io::Error::other)?;
            canvas.present();
            last_frame = now;
            frames += 1;
        }

        if now.duration_since(start) > Duration::from_secs(1) {
            let mhz = (cycles as f64) / 1_000_000.0;
            canvas
                .window_mut()
                .set_title(&format!("possum-emu :: {mhz:.03} MHz :: {frames} fps"))
                .map_err(io::Error::other)?;
            start = now;
            frames = 0;
            cycles = 0;
        }
    }

    Ok(())
}
