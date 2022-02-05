pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;

    fn write(&mut self, addr: u16, data: u8);

    fn input(&mut self, port: u16) -> u8;

    fn output(&mut self, port: u16, data: u8);
}

pub trait InterruptHandler: Bus {
    fn interrupted(&mut self) -> bool;

    fn interrupt_vector(&mut self) -> u8;

    fn ack_interrupt(&mut self);
}

pub trait DeviceBus: Bus {
    fn reti(&self) -> bool;
}

pub trait Device {
    fn tick(&mut self, bus: &mut dyn DeviceBus);

    fn read(&mut self, port: u16) -> u8;

    fn write(&mut self, port: u16, data: u8);

    fn interrupting(&self) -> bool;

    fn interrupt_vector(&self) -> u8;

    fn ack_interrupt(&mut self);
}

#[cfg(test)]
pub struct TestBus {
    mem: Vec<u8>,
    io: Vec<u8>,
    reti: bool,
}

#[cfg(test)]
impl TestBus {
    pub fn new() -> Self {
        Self {
            mem: vec![0; 65536],
            io: vec![0; 65536],
            reti: false,
        }
    }

    pub fn with_mem(mem: Vec<u8>) -> Self {
        let mut result = Self {
            mem,
            io: vec![0; 65536],
            reti: false,
        };
        // Pad out the full memory size
        result.mem.resize(65536, 0);
        result
    }

    pub fn mem(&self) -> &[u8] {
        &self.mem
    }

    pub fn mem_mut(&mut self) -> &mut [u8] {
        &mut self.mem
    }

    pub fn io(&self) -> &[u8] {
        &self.io
    }

    pub fn io_mut(&mut self) -> &mut [u8] {
        &mut self.io
    }
}

#[cfg(test)]
impl Bus for TestBus {
    fn read(&mut self, addr: u16) -> u8 {
        self.mem[addr as usize]
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.mem[addr as usize] = data
    }

    fn input(&mut self, port: u16) -> u8 {
        self.io[port as usize]
    }

    fn output(&mut self, port: u16, data: u8) {
        self.io[port as usize] = data
    }
}

#[cfg(test)]
impl DeviceBus for TestBus {
    fn reti(&self) -> bool {
        self.reti
    }
}
