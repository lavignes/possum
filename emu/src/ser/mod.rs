//! 16550A UART Emulation

use std::collections::VecDeque;

use crate::{Device, DeviceBus};

struct InterruptFlag;
impl InterruptFlag {
    /// Enable rx ready interrupt
    const RX: u8 = 0x01;

    /// Enable tx empty interrupt
    const TX: u8 = 0x02;

    /// Enable rx line status interrupt
    const RX_STATUS: u8 = 0x04;

    /// Enable modem status register interrupt
    const MODEM_STATUS: u8 = 0x08;
}

struct FifoControl;
impl FifoControl {
    const FIFO_ENABLE: u8 = 0x01;

    /// Clears the rx fifo
    const RX_FIFO_RESET: u8 = 0x02;

    /// Clears the tx fifo
    const TX_FIFO_RESET: u8 = 0x04;

    const DMA_MODE_SELECT: u8 = 0x08;
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
