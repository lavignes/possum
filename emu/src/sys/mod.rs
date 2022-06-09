//! The whole system tied together. Implements the shared bus.

use crate::{
    bus::{Bus, Device, DeviceBus, InterruptBus, NullBus},
    cpu::Cpu,
    dma::Dma,
    vdc::{Framebuffer, Vdc},
};

const BANK_SIZE: usize = 0x10000;
const BANK_MAX: usize = 0x1F;
const BANK_SHADOW_SIZE: u16 = 0x0400;

struct IOAddr {}

impl IOAddr {
    const DMA: u16 = 0x00;
    const BANK: u16 = 0x10;
    const HD: u16 = 0x80;
    const VDC: u16 = 0x90;
    const PIPE: u16 = 0xF0;
}

pub struct System {
    cpu: Cpu,
    bank: BankSelect,
    dma: Dma,
    ram: Vec<u8>,
    hd: Option<Box<dyn Device>>,
    vdc: Vdc,
    pipe: Box<dyn Device>,
}

#[inline]
fn read(ram: &[u8], offset: usize, addr: u16) -> u8 {
    if addr < BANK_SHADOW_SIZE {
        ram[addr as usize]
    } else {
        ram[addr as usize + offset]
    }
}

#[inline]
fn write(ram: &mut [u8], offset: usize, addr: u16, data: u8) {
    if addr < BANK_SHADOW_SIZE {
        ram[addr as usize] = data
    } else {
        ram[addr as usize + offset] = data
    }
}

#[derive(Default)]
pub struct BankSelect {
    bank: usize,
    offset: usize,
}

impl BankSelect {
    #[inline]
    pub fn select(&mut self, bank: u8) {
        self.bank = (bank as usize) & BANK_MAX;
        self.offset = self.bank * BANK_SIZE;
    }

    pub fn bank(&self) -> u8 {
        self.bank as u8
    }

    #[inline]
    pub fn ram_offset(&self) -> usize {
        self.offset
    }
}

struct CpuView<'a> {
    bank: &'a mut BankSelect,
    dma: &'a mut Dma,
    ram: &'a mut Vec<u8>,
    hd: &'a mut Option<&'a mut Box<dyn Device>>,
    vdc: &'a mut Vdc,
    pipe: &'a mut dyn Device,
}

impl<'a> Bus for CpuView<'a> {
    fn read(&mut self, addr: u16) -> u8 {
        read(&self.ram, self.bank.ram_offset(), addr)
    }

    fn write(&mut self, addr: u16, data: u8) {
        write(&mut self.ram, self.bank.ram_offset(), addr, data);
    }

    fn input(&mut self, port: u16) -> u8 {
        match port & 0xF0 {
            IOAddr::DMA => self.dma.read(port),

            IOAddr::BANK => self.bank.bank(),

            IOAddr::HD => match self.hd {
                Some(hd) => hd.read(port),
                _ => 0,
            },

            IOAddr::VDC => self.vdc.read(port),

            IOAddr::PIPE => self.pipe.read(port),

            _ => 0,
        }
    }

    fn output(&mut self, port: u16, data: u8) {
        match port & 0xF0 {
            IOAddr::DMA => self.dma.write(port, data),

            IOAddr::BANK => self.bank.select(data),

            IOAddr::HD => {
                if let Some(hd) = self.hd {
                    hd.write(port, data)
                }
            }

            IOAddr::VDC => self.vdc.write(port, data),

            IOAddr::PIPE => self.pipe.write(port, data),

            _ => {}
        }
    }
}

// This impl handles the well-documented z80 interrupt daisy-chain
impl<'a> InterruptBus for CpuView<'a> {
    fn interrupted(&mut self) -> bool {
        if self.dma.interrupting() {
            return true;
        }
        match self.hd {
            Some(hd) if hd.interrupting() => return true,
            _ => {}
        }
        if self.vdc.interrupting() {
            return true;
        }
        if self.pipe.interrupting() {
            return true;
        }
        false
    }
}

struct DmaView<'a> {
    bank: &'a mut BankSelect,
    ram: &'a mut Vec<u8>,
    hd: &'a mut Option<&'a mut Box<dyn Device>>,
    vdc: &'a mut Vdc,
    pipe: &'a mut dyn Device,
}

impl<'a> Bus for DmaView<'a> {
    fn read(&mut self, addr: u16) -> u8 {
        read(&self.ram, self.bank.ram_offset(), addr)
    }

    fn write(&mut self, addr: u16, data: u8) {
        write(&mut self.ram, self.bank.ram_offset(), addr, data);
    }

    fn input(&mut self, port: u16) -> u8 {
        match port & 0xF0 {
            IOAddr::BANK => self.bank.bank(),

            IOAddr::HD => match self.hd {
                Some(hd) => hd.read(port),
                _ => 0,
            },

            IOAddr::VDC => self.vdc.read(port),

            IOAddr::PIPE => self.pipe.read(port),

            _ => 0,
        }
    }

    fn output(&mut self, port: u16, data: u8) {
        match port & 0xF0 {
            IOAddr::BANK => self.bank.select(data),

            IOAddr::HD => {
                if let Some(hd) = self.hd {
                    hd.write(port, data)
                }
            }

            IOAddr::VDC => self.vdc.write(port, data),

            IOAddr::PIPE => self.pipe.write(port, data),

            _ => {}
        }
    }
}

impl<'a> DeviceBus for DmaView<'a> {}

impl System {
    // TODO: builder?
    pub fn new(pipe: Box<dyn Device>, hd: Option<Box<dyn Device>>) -> Self {
        Self {
            cpu: Cpu::default(),
            bank: BankSelect::default(),
            dma: Dma::default(),
            ram: vec![0; 0x10000 * 0x20],
            hd,
            vdc: Vdc::new(),
            pipe,
        }
    }

    pub fn step(&mut self) -> usize {
        let Self {
            cpu,
            bank,
            dma,
            ram,
            hd,
            vdc,
            pipe,
            ..
        } = self;

        let cycles = cpu.step(&mut CpuView {
            bank,
            dma,
            ram,
            hd: &mut hd.as_mut(),
            vdc,
            pipe: pipe.as_mut(),
        });

        for _ in 0..cycles {
            // note: only 1 DMA device can run at a time.
            //   If I want to add support for more, then I need to put them in the daisy chain
            //   and only run the first DMA that is wishing to tick.
            dma.tick(&mut DmaView {
                bank,
                ram,
                hd: &mut hd.as_mut(),
                vdc,
                pipe: pipe.as_mut(),
            });

            vdc.tick(&mut NullBus {});
        }
        cycles
    }

    #[inline]
    pub fn halted(&self) -> bool {
        self.cpu.halted()
    }

    #[inline]
    pub fn framebuffer_ready(&self) -> bool {
        self.vdc.framebuffer_ready()
    }

    #[inline]
    pub fn framebuffer(&self) -> &Framebuffer {
        self.vdc.framebuffer()
    }

    pub fn write_ram(&mut self, data: &[u8], offset: usize) {
        for (i, b) in data.iter().enumerate() {
            self.ram[offset + i] = *b;
        }
    }
}
