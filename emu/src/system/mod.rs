use crate::{
    cpu::Cpu,
    bus::Bus,
    dma::Dma
};
use crate::bus::{Device, DeviceBus};

struct System {
    cpu: Cpu,
    dma: Dma,
    ram: Vec<u8>,
}

struct CpuView<'a> {
    dma: &'a mut Dma,
    ram: &'a mut Vec<u8>,
}

impl<'a> Bus for CpuView<'a> {
    fn read(&mut self, addr: u16) -> u8 {
        self.ram[addr as usize]
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.ram[addr as usize] = data
    }

    fn input(&mut self, port: u16) -> u8 {
        self.dma.read(port)
    }

    fn output(&mut self, port: u16, data: u8) {
        self.dma.write(port, data)
    }
}

struct DmaView<'a> {
    cpu: &'a mut Cpu,
    ram: &'a mut Vec<u8>,
    reti: bool,
}

impl<'a> Bus for DmaView<'a> {
    fn read(&mut self, addr: u16) -> u8 {
        self.ram[addr as usize]
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.ram[addr as usize] = data
    }

    fn input(&mut self, _: u16) -> u8 {
        unimplemented!()
    }

    fn output(&mut self, _: u16, _: u8) {
        unimplemented!()
    }
}

impl<'a> DeviceBus for DmaView<'a> {
    fn reti(&self) -> bool {
        self.reti
    }
}

impl System {
    fn step(&mut self) {
        let System { cpu, dma, ram, .. } = self;
        let cycles = cpu.step(&mut CpuView { dma, ram });

        let mut reti = cpu.returned_from_interrupt();
        for _ in 0..cycles {
            // note: only 1 DMA device can run at a time.
            //   If I want to add support for more, then I need to put them in the daisy chain
            //   and only run the first DMA that is wishing to tick.
            dma.tick(&mut DmaView { cpu, ram, reti });

            // clear reti since it should only impact us for 1 cycle, right?
            // TODO: I think the accurate impl would be to only check reti on final cycle
            reti = false;
        }
    }
}