//! ASCII Parallel Keyboard Emulation

use std::{
    collections::VecDeque,
    io::{self, Write},
};

use possum_emu::{Device, DeviceBus};
use sdl2::{event::Event, EventPump};

pub struct AsciiKeyboard {
    event_pump: EventPump,
    buffer: VecDeque<u8>,
}

impl AsciiKeyboard {
    pub fn new(event_pump: EventPump) -> Self {
        Self {
            event_pump,
            buffer: VecDeque::new(),
        }
    }
}

impl Device for AsciiKeyboard {
    fn tick(&mut self, _: &mut dyn DeviceBus) {}

    fn read(&mut self, _: u16) -> u8 {
        match self.event_pump.poll_event() {
            Some(Event::TextInput { text, .. }) => self.buffer.extend(text.bytes()),
            _ => {}
        }
        self.buffer.pop_front().unwrap_or(0)
    }

    fn write(&mut self, _: u16, data: u8) {
        // TODO: This is a basic output for debugging. Obviously in reality
        //   you can't write to your keyboard :-P
        io::stdout().write(&[data]).unwrap();
    }

    fn interrupting(&self) -> bool {
        false
    }

    fn interrupt_vector(&self) -> u8 {
        0
    }

    fn ack_interrupt(&mut self) {}
}
