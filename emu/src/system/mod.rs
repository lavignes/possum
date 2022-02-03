use crate::{
    bus::{Bus, Device, DeviceBus},
    cpu::Cpu,
    dma::Dma,
};

pub struct System {
    cpu: Cpu,
    dma: Dma,
    ram: Vec<u8>,
    hd0: Option<Box<dyn Device>>,
    hd1: Option<Box<dyn Device>>,
    pipe: Box<dyn Device>,
}

struct CpuView<'a> {
    dma: &'a mut Dma,
    ram: &'a mut Vec<u8>,
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
        if (port & 1) == 1 {
            self.pipe.read(port)
        } else {
            self.dma.read(port)
        }
    }

    fn output(&mut self, port: u16, data: u8) {
        if (port & 1) == 1 {
            self.pipe.write(port, data)
        } else {
            self.dma.write(port, data)
        }
    }
}

struct DmaView<'a> {
    cpu: &'a mut Cpu,
    ram: &'a mut Vec<u8>,
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
        if (port & 1) == 1 {
            self.pipe.read(port)
        } else {
            0
        }
    }

    fn output(&mut self, port: u16, data: u8) {
        if (port & 1) == 1 {
            self.pipe.write(port, data);
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
    pub fn new(pipe: Box<dyn Device>, hd0: Option<Box<dyn Device>>) -> Self {
        Self {
            cpu: Cpu::default(),
            dma: Dma::default(),
            ram: vec![0; 65536],
            hd0,
            hd1: None,
            pipe,
        }
    }

    pub fn step(&mut self) {
        let System {
            cpu,
            dma,
            ram,
            pipe,
            ..
        } = self;
        let cycles = cpu.step(&mut CpuView {
            dma,
            ram,
            pipe: pipe.as_mut(),
        });

        let mut reti = cpu.returned_from_interrupt();
        for _ in 0..cycles {
            // note: only 1 DMA device can run at a time.
            //   If I want to add support for more, then I need to put them in the daisy chain
            //   and only run the first DMA that is wishing to tick.
            dma.tick(&mut DmaView {
                cpu,
                ram,
                pipe: pipe.as_mut(),
                reti,
            });

            // clear reti since it should only impact us for 1 cycle, right?
            // TODO: I think the accurate impl would be to only check reti on final cycle
            reti = false;
        }
    }

    pub fn write_ram(&mut self, data: &[u8], offset: usize) {
        for (i, b) in data.iter().enumerate() {
            self.ram[offset + i] = *b;
        }
    }
}
