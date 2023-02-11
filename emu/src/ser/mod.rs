//! 16550A UART Emulation

use std::{
    collections::VecDeque,
    io::{Read, Write},
};

use crate::{Device, DeviceBus};

struct InterruptEnable;
impl InterruptEnable {
    /// Enable rx ready interrupt
    const RX_READY: u8 = 0x01;

    /// Enable tx empty interrupt
    const TX_EMPTY: u8 = 0x02;

    /// Enable rx line status interrupt
    const RX_STATUS: u8 = 0x04;

    /// Enable modem status register interrupt
    const MODEM_STATUS: u8 = 0x08;
}

struct InterruptSource;
impl InterruptSource {
    const MODEM_STATUS: u8 = 0x00;

    const NONE: u8 = 0x01;

    const TX_EMPTY: u8 = 0x02;

    const RX_READY: u8 = 0x04;

    const RX_STATUS: u8 = 0x06;
}

pub struct Uart<T> {
    handle: T,
    tx_fifo: VecDeque<u8>,
    rx_fifo: VecDeque<u8>,
    interrupt_enable: u8,
    interrupt_status: u8,
    fifo_control: u8,
    line_control: u8,
    modem_control: u8,
    line_status: u8,
    modem_status: u8,
    divisor_latch: u16,
}

impl<T> Uart<T> {
    #[inline]
    pub fn new(handle: T) -> Self {
        Self {
            handle,
            tx_fifo: VecDeque::with_capacity(16),
            rx_fifo: VecDeque::with_capacity(16),
            interrupt_enable: 0,
            interrupt_status: 0,
            fifo_control: 0,
            line_control: 0,
            modem_control: 0,
            line_status: 0,
            modem_status: 0,
            divisor_latch: 0,
        }
    }
}

impl<T> Device for Uart<T>
where
    T: Read + Write,
{
    fn tick(&mut self, _: &mut dyn DeviceBus) {
        if self.rx_fifo.is_empty() {
            let mut buf = [0; 16];
            let read = self.handle.read(&mut buf).unwrap_or_default();
            for i in 0..read {
                self.rx_fifo.push_back(buf[i]);
            }
        }

        if let Some(data) = self.tx_fifo.pop_front() {
            self.handle.write_all(&[data]).unwrap_or_default();
        }
    }

    fn read(&mut self, port: u16) -> u8 {
        match port & 0x07 {
            0 => {
                if (self.line_control & 0x80) == 0 {
                    self.rx_fifo.pop_front().unwrap_or_default()
                } else {
                    self.divisor_latch as u8
                }
            }

            1 => {
                if (self.line_control & 0x80) == 0 {
                    self.interrupt_enable
                } else {
                    (self.divisor_latch >> 8) as u8
                }
            }

            2 => self.interrupt_status,

            3 => self.line_control,

            4 => self.modem_control,

            5 => self.line_status,

            6 => self.modem_status,

            _ => 0,
        }
    }

    fn write(&mut self, port: u16, data: u8) {
        match port & 0x07 {
            0 => {
                if (self.line_control & 0x80) == 0 {
                    self.tx_fifo.push_back(data);
                } else {
                    self.divisor_latch = (self.divisor_latch & 0xFF00) | data as u16;
                }
            }

            1 => {
                if (self.line_control * 0x80) == 0 {
                    self.interrupt_enable = data;
                } else {
                    self.divisor_latch = (self.divisor_latch & 0x00FF) | ((data as u16) << 8);
                }
            }

            2 => self.fifo_control = data,

            3 => self.line_control = data,

            4 => self.modem_control = data,

            5 => {}

            6 => {}

            _ => {}
        }
    }

    fn interrupting(&self) -> bool {
        todo!()
    }
}
