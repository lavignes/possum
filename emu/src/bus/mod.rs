pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;

    fn write(&mut self, addr: u16, data: u8);

    fn input(&mut self, port: u16) -> u8;

    fn output(&mut self, port: u16, data: u8);

    fn reti(&mut self) {}
}