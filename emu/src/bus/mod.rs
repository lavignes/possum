pub trait Bus {
    /// Read data from memory.
    fn read(&mut self, addr: u16) -> u8;

    /// Write data to memory.
    fn write(&mut self, addr: u16, data: u8);

    /// Read data from io device.
    fn input(&mut self, port: u16) -> u8;

    /// Send data to io device.
    fn output(&mut self, port: u16, data: u8);
}

pub trait DeviceBus: Bus {
    fn reti(&self) -> bool;
}

pub trait Device {
    fn tick(&mut self, bus: &mut impl DeviceBus);

    fn read(&mut self, port: u16) -> u8;

    fn write(&mut self, port: u16, data: u8);
}