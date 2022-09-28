//! The whole system tied together. Implements the shared bus.

use crate::{
    bus::{Bus, Device, InterruptBus, NullBus},
    cpu::Cpu,
    vdc::{Framebuffer, Vdc},
};

const BANK_SIZE: usize = 0x10000;
const BANK_MAX: usize = 0x1F;
const BANK_SHADOW_SIZE: u16 = 0x0400;

struct IOAddr;
impl IOAddr {
    const IC: u16 = 0x00;
    const BANK: u16 = 0x01;
    const KB: u16 = 0x02;

    const SER1: u16 = 0x10;
    const SER2: u16 = 0x18;
    const HD: u16 = 0x20;
    const VDC: u16 = 0x40;
}

struct InterruptPriority;
impl InterruptPriority {
    const SER1: u8 = 0x00;
    const SER2: u8 = 0x01;
    const HD: u8 = 0x02;
    const VDC: u8 = 0x03;
}

pub struct System {
    cpu: Cpu,
    bank: BankSelect,
    ram: Vec<u8>,
    hd: Option<Box<dyn Device>>,
    vdc: Vdc,
    kb: Box<dyn Device>,
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

    #[inline]
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
    ram: &'a mut Vec<u8>,
    hd: &'a mut Option<&'a mut Box<dyn Device>>,
    vdc: &'a mut Vdc,
    kb: &'a mut dyn Device,
}

impl<'a> Bus for CpuView<'a> {
    fn read(&mut self, addr: u16) -> u8 {
        read(&self.ram, self.bank.ram_offset(), addr)
    }

    fn write(&mut self, addr: u16, data: u8) {
        write(&mut self.ram, self.bank.ram_offset(), addr, data);
    }

    fn input(&mut self, port: u16) -> u8 {
        // The upper byte or port not something we want
        let port = port & 0xFF; 
        match port & 0xF0 {
            // The lowest devices all mask to the same space
            // as the IC
            IOAddr::IC => match port {
                IOAddr::IC => {
                    if let Some(hd) = self.hd && hd.interrupting() { 
                        return InterruptPriority::HD;
                    }
                    if self.vdc.interrupting() {
                        return InterruptPriority::VDC;
                    }
                    todo!("Read PIC when not in interrupt. Undefined state");
                }

                IOAddr::KB => self.kb.read(port),

                IOAddr::BANK => self.bank.bank(),

                _ => 0,
            },


            IOAddr::HD => match self.hd {
                Some(hd) => hd.read(port),
                _ => 0,
            },

            IOAddr::VDC => self.vdc.read(port),

            _ => 0,
        }
    }

    fn output(&mut self, port: u16, data: u8) {
        // The upper byte or port not something we want
        let port = port & 0xFF; 
        match port & 0xF0 {
            // The lowest ports all mask to the same space
            // as the IC
            IOAddr::IC => match port {
                IOAddr::KB => self.kb.write(port, data),

                IOAddr::BANK => self.bank.select(data),

                _ => {}
            }

            IOAddr::HD => {
                if let Some(hd) = self.hd {
                    hd.write(port, data);
                }
            }

            IOAddr::VDC => self.vdc.write(port, data),

            _ => {}
        }
    }
}

impl<'a> InterruptBus for CpuView<'a> {
    fn interrupted(&mut self) -> bool {
        if let Some(hd) = self.hd && hd.interrupting() { 
            return true;
        }
        if self.vdc.interrupting() {
            return true;
        }
        false
    }
}

impl System {
    pub fn new(kb: Box<dyn Device>, hd: Option<Box<dyn Device>>) -> Self {
        Self {
            cpu: Cpu::default(),
            bank: BankSelect::default(),
            ram: vec![0; 0x10000 * 0x20],
            hd,
            vdc: Vdc::new(),
            kb,
        }
    }

    pub fn step(&mut self) -> usize {
        let Self {
            cpu,
            bank,
            ram,
            hd,
            vdc,
            kb,
            ..
        } = self;

        let cycles = cpu.step(&mut CpuView {
            bank,
            ram,
            hd: &mut hd.as_mut(),
            vdc,
            kb: kb.as_mut(),
        });

        // Process devices that run in parallel with CPU
        // TODO: For DMA devices, they compete with the CPU for the bus.
        //   depending on the transfer mode they may hold the bus or only
        //   take it for ~3/4 of CPU cycles.
        //   Overall I will probably have:
        //   * A DMA device for serial port 1
        //   * A DMA device for serial port 2 (can I share 1?)
        //   * A DMA device for the CF bus
        //   * And maybe an extra for the VDC (though that requires emulating an 8564
        //     for the VDC RDY signal) 
        for _ in 0..cycles {
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

    #[inline]
    pub fn write_ram(&mut self, data: &[u8], offset: usize) {
        for (i, b) in data.iter().enumerate() {
            self.ram[offset + i] = *b;
        }
    }
}
