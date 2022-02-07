use crate::{
    bus::{Bus, Device, DeviceBus, InterruptHandler, NullBus},
    cpu::Cpu,
    dma::Dma,
    vdc::Vdc,
};

pub struct System {
    cpu: Cpu,
    dma: Dma,
    ram: Vec<u8>,
    hd: Option<Box<dyn Device>>,
    vdc: Vdc,
    pipe: Box<dyn Device>,

    vblank: bool,
}

struct CpuView<'a> {
    dma: &'a mut Dma,
    ram: &'a mut Vec<u8>,
    hd: &'a mut Option<&'a mut Box<dyn Device>>,
    vdc: &'a mut Vdc,
    pipe: &'a mut dyn Device,
}

impl<'a> Bus for CpuView<'a> {
    fn read(&mut self, addr: u16) -> u8 {
        self.ram[addr as usize]
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.ram[addr as usize] = data
    }

    fn input(&mut self, port: u16) -> u8 {
        match port & 0xF0 {
            0x00 => self.dma.read(port),

            0x80 => match self.hd {
                Some(hd) => hd.read(port),
                _ => 0,
            },

            0x90 | 0x91 => self.vdc.read(port),

            0xF0 => self.pipe.read(port),

            _ => 0,
        }
    }

    fn output(&mut self, port: u16, data: u8) {
        match port & 0xF0 {
            0x00 => self.dma.write(port, data),

            0x80 => {
                if let Some(hd) = self.hd {
                    hd.write(port, data)
                }
            }

            0x90 | 0x91 => self.vdc.write(port, data),

            0xF0 => self.pipe.write(port, data),

            _ => {}
        }
    }
}

// This impl handles the well-documented z80 interrupt daisy-chain
impl<'a> InterruptHandler for CpuView<'a> {
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

    fn interrupt_vector(&mut self) -> u8 {
        if self.dma.interrupting() {
            return self.dma.interrupt_vector();
        }
        match self.hd {
            Some(hd) if hd.interrupting() => return hd.interrupt_vector(),
            _ => {}
        }
        if self.vdc.interrupting() {
            return self.vdc.interrupt_vector();
        }
        if self.pipe.interrupting() {
            return self.pipe.interrupt_vector();
        }
        0
    }

    fn ack_interrupt(&mut self) {
        if self.dma.interrupting() {
            return self.dma.ack_interrupt();
        }
        match self.hd {
            Some(hd) if hd.interrupting() => hd.ack_interrupt(),
            _ => {}
        }
        if self.vdc.interrupting() {
            return self.vdc.ack_interrupt();
        }
        if self.pipe.interrupting() {
            return self.pipe.ack_interrupt();
        }
    }
}

struct DmaView<'a> {
    ram: &'a mut Vec<u8>,
    hd: &'a mut Option<&'a mut Box<dyn Device>>,
    vdc: &'a mut Vdc,
    pipe: &'a mut dyn Device,
    reti: bool,
}

impl<'a> Bus for DmaView<'a> {
    fn read(&mut self, addr: u16) -> u8 {
        self.ram[addr as usize]
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.ram[addr as usize] = data
    }

    fn input(&mut self, port: u16) -> u8 {
        match port & 0xF0 {
            0x80 => match self.hd {
                Some(hd) => hd.read(port),
                _ => 0,
            },

            0x90 | 0x91 => self.vdc.read(port),

            0xF0 => self.pipe.read(port),

            _ => 0,
        }
    }

    fn output(&mut self, port: u16, data: u8) {
        match port & 0xF0 {
            0x80 => {
                if let Some(hd) = self.hd {
                    hd.write(port, data)
                }
            }

            0x90 | 0x91 => self.vdc.write(port, data),

            0xF0 => self.pipe.write(port, data),

            _ => {}
        }
    }
}

impl<'a> DeviceBus for DmaView<'a> {
    fn reti(&self) -> bool {
        self.reti
    }
}

impl System {
    // TODO: builder?
    pub fn new(pipe: Box<dyn Device>, hd: Option<Box<dyn Device>>) -> Self {
        Self {
            cpu: Cpu::default(),
            dma: Dma::default(),
            ram: vec![0; 65536],
            hd,
            vdc: Vdc::new(),
            pipe,
            vblank: false,
        }
    }

    pub fn step(&mut self) -> usize {
        let Self {
            cpu,
            dma,
            ram,
            hd,
            vdc,
            pipe,
            ..
        } = self;

        let cycles = cpu.step(&mut CpuView {
            dma,
            ram,
            hd: &mut hd.as_mut(),
            vdc,
            pipe: pipe.as_mut(),
        });

        let mut reti = cpu.returned_from_interrupt();
        let mut vblank = false;
        for _ in 0..cycles {
            // note: only 1 DMA device can run at a time.
            //   If I want to add support for more, then I need to put them in the daisy chain
            //   and only run the first DMA that is wishing to tick.
            dma.tick(&mut DmaView {
                ram,
                hd: &mut hd.as_mut(),
                vdc,
                pipe: pipe.as_mut(),
                reti,
            });

            vdc.tick(&mut NullBus {});
            if vdc.vblank() {
                vblank = true;
            }

            // clear reti since it should only impact us for 1 cycle, right?
            // TODO: I think the accurate impl would be to only check reti on final cycle
            reti = false;
        }
        self.vblank = vblank;
        cycles
    }

    pub fn halted(&self) -> bool {
        self.cpu.halted()
    }

    pub fn vblank(&self) -> bool {
        self.vblank
    }

    pub fn framebuffer(&self) -> &[u32] {
        self.vdc.framebuffer()
    }

    pub fn write_ram(&mut self, data: &[u8], offset: usize) {
        for (i, b) in data.iter().enumerate() {
            self.ram[offset + i] = *b;
        }
    }
}
