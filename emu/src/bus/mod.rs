//! Common traits exposed to devices on the system bus.

pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;

    fn write(&mut self, addr: u16, data: u8);

    fn input(&mut self, port: u16) -> u8;

    fn output(&mut self, port: u16, data: u8);
}

pub trait InterruptBus: Bus {
    fn interrupted(&mut self) -> bool;
}

pub trait DeviceBus: Bus {}

pub trait Device {
    fn tick(&mut self, bus: &mut dyn DeviceBus);

    fn read(&mut self, port: u16) -> u8;

    fn write(&mut self, port: u16, data: u8);

    fn interrupting(&self) -> bool;
}

pub struct NullBus;

impl Bus for NullBus {
    fn read(&mut self, _: u16) -> u8 {
        0
    }

    fn write(&mut self, _: u16, _: u8) {}

    fn input(&mut self, _: u16) -> u8 {
        0
    }

    fn output(&mut self, _: u16, _: u8) {}
}

impl DeviceBus for NullBus {}

#[cfg(test)]
pub struct TestBus {
    mem: Vec<u8>,
    io: Vec<u8>,
}

#[cfg(test)]
impl TestBus {
    pub fn new() -> Self {
        Self {
            mem: vec![0; 65536],
            io: vec![0; 65536],
        }
    }

    pub fn with_mem(mem: Vec<u8>) -> Self {
        let mut result = Self {
            mem,
            io: vec![0; 65536],
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
impl DeviceBus for TestBus {}

#[cfg(test)]
impl InterruptBus for TestBus {
    fn interrupted(&mut self) -> bool {
        false
    }

    fn ack_interrupt(&mut self) -> u8 {
        0
    }

    fn ret_interrupt(&mut self) {}
}
