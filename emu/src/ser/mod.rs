//! 16550A UART Emulation

use std::collections::VecDeque;

use crate::{Device, DeviceBus};

bitflags::bitflags! {
    struct InterruptFlag: u8 {
        // Enable rx ready interrupt
        const RX = 0x01;

        // Enable tx empty interrupt
        const TX = 0x02;

        // Enable rx line status interrupt
        const RX_STATUS = 0x04;

        // Enable modem status register interrupt
        const MODEM_STATUS = 0x08;
    }
}

bitflags::bitflags! {
    struct FifoControl: u8 {
        const FIFO_ENABLE = 0x01;

        // Clears the rx fifo
        const RX_FIFO_RESET = 0x02;

        // Clears the tx fifo
        const TX_FIFO_RESET = 0x04;

        const DMA_MODE_SELECT = 0x08;
    }
}

pub struct Uart {
    tx_fifo: VecDeque<u8>,
    rx_fifo: VecDeque<u8>,
    tx: Option<u8>,
    rx: Option<u8>,
    isr: u8,
}

impl Uart {
    #[inline]
    pub fn new() -> Self {
        Self {
            tx_fifo: VecDeque::new(),
            rx_fifo: VecDeque::new(),
            tx: None,
            rx: None,
            isr: 0,
        }
    }
}

impl Device for Uart {
    fn tick(&mut self, bus: &mut dyn DeviceBus) {
        todo!()
    }

    fn read(&mut self, port: u16) -> u8 {
        match port & 0x07 {
            0 => self.rx.unwrap_or(0),

            2 => todo!(),

            _ => todo!(),
        }
    }

    fn write(&mut self, port: u16, data: u8) {
        match port & 0x07 {
            0 => self.tx = Some(data),

            _ => todo!(),
        }
    }

    fn interrupting(&self) -> bool {
        todo!()
    }
}
