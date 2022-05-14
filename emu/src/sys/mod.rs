//! The whole system tied together. Implements the shared bus.

use crate::{
    bus::{Bus, Device, DeviceBus, InterruptBus, NullBus},
    cpu::Cpu,
    dma::Dma,
    vdc::{Framebuffer, Vdc},
};

const BANK_SIZE: usize = 0x10000;
const BANK_MAX: usize = 0x1F;

pub struct System {
    cpu: Cpu,
    bank: BankSelect,
    dma: Dma,
    ram: Vec<u8>,
    hd: Option<Box<dyn Device>>,
    vdc: Vdc,
    pipe: Box<dyn Device>,
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
        self.ram[addr as usize + self.bank.ram_offset()]
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.ram[addr as usize + self.bank.ram_offset()] = data
    }

    fn input(&mut self, port: u16) -> u8 {
        match port & 0xF0 {
            0x00 => self.dma.read(port),

            0x10 => self.bank.bank(),

            0x80 => match self.hd {
                Some(hd) => hd.read(port),
                _ => 0,
            },

            0x90 => self.vdc.read(port),

            0xF0 => self.pipe.read(port),

            _ => 0,
        }
    }

    fn output(&mut self, port: u16, data: u8) {
        match port & 0xF0 {
            0x00 => self.dma.write(port, data),

            0x10 => self.bank.select(data),

            0x80 => {
                if let Some(hd) = self.hd {
                    hd.write(port, data)
                }
            }

            0x90 => self.vdc.write(port, data),

            0xF0 => self.pipe.write(port, data),

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

    fn ack_interrupt(&mut self) -> u8 {
        if self.dma.interrupting() {
            return self.dma.ack_interrupt();
        }
        match self.hd {
            Some(hd) if hd.interrupting() => return hd.ack_interrupt(),
            _ => {}
        }
        if self.vdc.interrupting() {
            return self.vdc.ack_interrupt();
        }
        if self.pipe.interrupting() {
            return self.pipe.ack_interrupt();
        }
        0
    }

    fn ret_interrupt(&mut self) {
        if self.dma.interrupt_pending() {
            return self.dma.ret_interrupt();
        }
        match self.hd {
            Some(hd) if hd.interrupt_pending() => return hd.ret_interrupt(),
            _ => {}
        }
        if self.vdc.interrupt_pending() {
            return self.vdc.ret_interrupt();
        }
        if self.pipe.interrupt_pending() {
            return self.pipe.ret_interrupt();
        }
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
        self.ram[addr as usize + self.bank.ram_offset()]
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.ram[addr as usize + self.bank.ram_offset()] = data
    }

    fn input(&mut self, port: u16) -> u8 {
        match port & 0xF0 {
            0x10 => self.bank.bank(),

            0x80 => match self.hd {
                Some(hd) => hd.read(port),
                _ => 0,
            },

            0x90 => self.vdc.read(port),

            0xF0 => self.pipe.read(port),

            _ => 0,
        }
    }

    fn output(&mut self, port: u16, data: u8) {
        match port & 0xF0 {
            0x10 => self.bank.select(data),

            0x80 => {
                if let Some(hd) = self.hd {
                    hd.write(port, data)
                }
            }

            0x90 => self.vdc.write(port, data),

            0xF0 => self.pipe.write(port, data),

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
