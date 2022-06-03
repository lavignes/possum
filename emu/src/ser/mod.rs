//! 16550A UART Emulation

use crate::{Device, DeviceBus};

pub struct Uart {
    tx_fifo: Vec<u8>,
    rx_fifo: Vec<u8>,
}

impl Uart {
    #[inline]
    pub fn new() -> Self {
        Self {
            tx_fifo: Vec::new(),
            rx_fifo: Vec::new(),
        }
    }
}

impl Device for Uart {
    fn tick(&mut self, bus: &mut dyn DeviceBus) {
        todo!()
    }

    fn read(&mut self, port: u16) -> u8 {
        todo!()
    }

    fn write(&mut self, port: u16, data: u8) {
        todo!()
    }

    fn interrupting(&self) -> bool {
        todo!()
    }

    fn interrupt_pending(&self) -> bool {
        todo!()
    }

    fn ack_interrupt(&mut self) -> u8 {
        todo!()
    }

    fn ret_interrupt(&mut self) {
        todo!()
    }
}
