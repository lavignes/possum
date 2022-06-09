//! z8410 DMA emulation

#[cfg(test)]
mod tests;

use crate::bus::{Device, DeviceBus};

bitflags::bitflags! {
    struct RR0Mask: u8 {
        const TRANSFER_OCCURRED = 0x01;
        const READY = 0x02;
        const INTERRUPT_PENDING = 0x08;
        const MATCH_NOT_FOUND = 0x10;
        const NOT_END_OF_BLOCK = 0x20;
    }
}

bitflags::bitflags! {
    struct WR0Mask: u8 {
        const SELECT_MASK = 0b1000_0000;
        const SELECT_BITS = 0b0000_0000;

        const SEARCH_OR_TRANSFER = 0x03;
        const DIRECTION = 0x04;
    }
}

bitflags::bitflags! {
    struct WR1Mask: u8 {
        const SELECT_MASK = 0b1000_0111;
        const SELECT_BITS = 0b0000_0100;

        const MEMORY_OR_IO = 0x08;
        const INCREMENT_DECREMENT_MODE = 0x30;

        const CYCLE_LENGTH = 0x03;
    }
}

bitflags::bitflags! {
    struct WR2Mask: u8 {
        const SELECT_MASK = 0b1000_0111;
        const SELECT_BITS = 0b0000_0000;

        const MEMORY_OR_IO = 0x08;
        const INCREMENT_DECREMENT_MODE = 0x30;

        const CYCLE_LENGTH = 0x03;
    }
}

bitflags::bitflags! {
    struct WR3Mask: u8 {
        const SELECT_MASK = 0b1000_0011;
        const SELECT_BITS = 0b1000_0000;

        const STOP_ON_MATCH = 0x04;
        const INTERRUPT_ENABLE = 0x20;
        const DMA_ENABLE = 0x40;
    }
}

bitflags::bitflags! {
    struct WR4Mask: u8 {
        const SELECT_MASK = 0b1000_0011;
        const SELECT_BITS = 0b1000_0001;

        const ACCESS_MODE = 0x06;

        const INTERRUPT_ON_MATCH = 0x01;
        const INTERRUPT_AT_END_OF_BLOCK = 0x02;
        const PULSE_GENERATED = 0x04;

        const INTERRUPT_ON_READY = 0x40;
        const STATUS_AFFECTS_VECTOR = 0x20;
    }
}

bitflags::bitflags! {
    struct WR5Mask: u8 {
        const SELECT_MASK = 0b1100_0111;
        const SELECT_BITS = 0b1000_0010;

        const CHIP_ENABLE_ONLY_WAIT = 0x10;
        const STOP_RESTART_ON_END_OF_BLOCK = 0x20;
    }
}

bitflags::bitflags! {
    struct WR6Mask: u8 {
        const SELECT_MASK = 0b1000_0011;
        const SELECT_BITS = 0b1000_0011;

        const STATUS = 0x01;
        const BYTE_COUNTER_LOW = 0x02;
        const BYTE_COUNTER_HIGH = 0x04;
        const PORT_A_ADDRESS_LOW = 0x08;
        const PORT_A_ADDRESS_HIGH = 0x10;
        const PORT_B_ADDRESS_LOW = 0x20;
        const PORT_B_ADDRESS_HIGH = 0x40;
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ReadRegister {
    Status,
    ByteCounterLow,
    ByteCounterHigh,
    PortAAddressLow,
    PortAAddressHigh,
    PortBAddressLow,
    PortBAddressHigh,
}

#[derive(Copy, Clone, Debug)]
enum WriteRegister {
    PortAAddressLow,
    PortAAddressHigh,
    BlockLengthLow,
    BlockLengthHigh,

    PortATiming,

    PortBTiming,

    MaskByte,
    MatchByte,

    PortBAddressLow,
    PortBAddressHigh,
    InterruptControl,
    PulseControl,
    InterruptVector,

    ReadMask,
}

#[derive(Copy, Clone, Debug)]
enum Direction {
    PortBToA,
    PortAToB,
}

impl Default for Direction {
    fn default() -> Self {
        Self::PortBToA
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum IncrementMode {
    Decrement,
    Increment,
    Fixed,
}

impl Default for IncrementMode {
    fn default() -> Self {
        Self::Decrement
    }
}

#[derive(Copy, Clone, Debug)]
enum AccessMode {
    Byte,
    Continuous,
    Burst,
}

impl Default for AccessMode {
    fn default() -> Self {
        Self::Byte
    }
}

#[derive(Default, Debug)]
pub struct Dma {
    status: u8,
    byte_counter: u16,
    port_a_counter: u16,
    port_b_counter: u16,
    read_order: Vec<ReadRegister>,

    enabled: bool,
    interrupts_enabled: bool,
    access_mode: AccessMode,
    stop_on_match: bool,
    mask_byte: u8,
    match_byte: u8,

    interrupt_on_match: bool,
    interrupt_at_end_of_block: bool,
    restart_at_end_of_block: bool,
    interrupt_on_ready: bool,
    status_affects_vector: bool,
    interrupt_vector: u8,

    read_mask: u8,

    search: bool,
    transfer: bool,
    direction: Direction,
    port_a_start_address: u16,
    port_b_start_address: u16,
    block_length: u16,

    port_a_is_memory: bool,
    port_a_increment_mode: IncrementMode,

    port_b_is_memory: bool,
    port_b_increment_mode: IncrementMode,

    // NOTE: Its fascinating but reads and writes to internal registers
    // are controlled by a sort of list of registers to write
    // that are popped off a stack. In reality, this is likely done
    // using some sort of shifter in the real DMA.

    // TODO: The implementation of the read and write register order tracking is *too* simple :-)
    //   We do need to emulate the real behavior by using `u8::trailing_zeros` to get the
    //   index of the lowest set bit in a `u8` and use that to look up the correct register.
    //   The added benefit there is that the code below that sets up the read/write stack doesnt
    //   need to be written in reverse order anymore.
    write_order: Vec<WriteRegister>,
}

impl Device for Dma {
    // TODO: The timing for the DMA is probably jank. We are reading and writing on every cycle.
    //   This might actually be how the DMS works, but I need to go through the docs again.
    fn tick(&mut self, bus: &mut dyn DeviceBus) {
        if !self.enabled {
            return;
        }

        // Step 1: Read source byte
        let byte = match self.direction {
            Direction::PortAToB => {
                if self.port_a_is_memory {
                    bus.read(self.port_a_counter)
                } else {
                    bus.input(self.port_a_counter)
                }
            }
            Direction::PortBToA => {
                if self.port_b_is_memory {
                    bus.read(self.port_b_counter)
                } else {
                    bus.input(self.port_b_counter)
                }
            }
        };

        // Step 2: Increment source counter
        match self.direction {
            Direction::PortAToB => match self.port_a_increment_mode {
                IncrementMode::Increment => {
                    self.port_a_counter = self.port_a_counter.wrapping_add(1);
                }
                IncrementMode::Decrement => {
                    self.port_a_counter = self.port_a_counter.wrapping_sub(1);
                }
                _ => {}
            },
            Direction::PortBToA => match self.port_b_increment_mode {
                IncrementMode::Increment => {
                    self.port_b_counter = self.port_b_counter.wrapping_add(1);
                }
                IncrementMode::Decrement => {
                    self.port_b_counter = self.port_b_counter.wrapping_sub(1);
                }
                _ => {}
            },
        }

        // Step 3: Load destination counter
        // At the start of a DMA, we load the counter for the destination port
        // but this can only happen if the increment mode is not fixed...
        // This is a weird documented quirk that makes fixed destination difficult to deal with.
        // Usually, this is resolved by swapping the direction an extra load command.
        if (self.status & RR0Mask::TRANSFER_OCCURRED.bits()) == 0 {
            match self.direction {
                Direction::PortAToB => {
                    if self.port_b_increment_mode != IncrementMode::Fixed {
                        self.port_b_counter = self.port_b_start_address;
                    }
                }
                Direction::PortBToA => {
                    if self.port_a_increment_mode != IncrementMode::Fixed {
                        self.port_a_counter = self.port_a_start_address;
                    }
                }
            }
        }

        // Step 4: Write source byte
        if self.transfer {
            match self.direction {
                Direction::PortAToB => {
                    if self.port_b_is_memory {
                        bus.write(self.port_b_counter, byte)
                    } else {
                        bus.output(self.port_b_counter, byte)
                    }
                }
                Direction::PortBToA => {
                    if self.port_a_is_memory {
                        bus.write(self.port_a_counter, byte)
                    } else {
                        bus.output(self.port_a_counter, byte)
                    }
                }
            }
        }
        // I believe we do this even if there is no transfer to indicate the DMA has started
        self.status |= RR0Mask::TRANSFER_OCCURRED.bits();

        // Step 5: Increment destination counter
        match self.direction {
            Direction::PortAToB => match self.port_b_increment_mode {
                IncrementMode::Increment => {
                    self.port_b_counter = self.port_b_counter.wrapping_add(1);
                }
                IncrementMode::Decrement => {
                    self.port_b_counter = self.port_b_counter.wrapping_sub(1);
                }
                _ => {}
            },
            Direction::PortBToA => match self.port_a_increment_mode {
                IncrementMode::Increment => {
                    self.port_a_counter = self.port_a_counter.wrapping_add(1);
                }
                IncrementMode::Decrement => {
                    self.port_a_counter = self.port_a_counter.wrapping_sub(1);
                }
                _ => {}
            },
        }

        // Step 6: Check for match
        if self.search {
            if (!self.mask_byte & self.match_byte) == byte {
                self.status &= !RR0Mask::MATCH_NOT_FOUND.bits();

                self.enabled = false;
                if self.interrupts_enabled && self.interrupt_on_match {
                    self.status |= RR0Mask::INTERRUPT_PENDING.bits();
                }

                if self.stop_on_match {
                    return;
                }
            }
        }

        // Step 7: Check for end of block
        self.byte_counter = self.byte_counter.wrapping_add(1);
        if self.byte_counter == self.block_length {
            self.status &= !RR0Mask::NOT_END_OF_BLOCK.bits();

            self.enabled = false;
            if self.interrupt_at_end_of_block {
                self.status |= RR0Mask::INTERRUPT_PENDING.bits();
            }

            if self.restart_at_end_of_block {
                // TODO: Do I need to stay enabled when I restart?
                todo!()
            }
        }

        if self.interrupt_on_ready {
            self.status |= RR0Mask::INTERRUPT_PENDING.bits();
        }
    }

    fn read(&mut self, _: u16) -> u8 {
        debug_assert!(self.read_order.len() <= 8);

        match self.read_order.pop() {
            Some(ReadRegister::Status) => self.status,

            Some(ReadRegister::ByteCounterLow) => self.byte_counter as u8,
            Some(ReadRegister::ByteCounterHigh) => (self.byte_counter >> 8) as u8,

            Some(ReadRegister::PortAAddressLow) => self.port_a_counter as u8,
            Some(ReadRegister::PortAAddressHigh) => (self.port_a_counter >> 8) as u8,

            Some(ReadRegister::PortBAddressLow) => self.port_b_counter as u8,
            Some(ReadRegister::PortBAddressHigh) => (self.port_b_counter >> 8) as u8,

            None => 0,
        }
    }

    fn write(&mut self, _: u16, data: u8) {
        debug_assert!(self.write_order.len() <= 8);

        if self.enabled {
            return;
        }

        match self.write_order.pop() {
            // Initial state. Need to figure out which base register we are using.
            // The logic for selecting the bits seems super janky but this is what it is.
            None => {
                // Register 1 => 0XXX X100
                if (data & WR1Mask::SELECT_MASK.bits()) == WR1Mask::SELECT_BITS.bits() {
                    self.port_a_is_memory = (data & WR1Mask::MEMORY_OR_IO.bits()) == 0;

                    self.port_a_increment_mode =
                        match (data & WR1Mask::INCREMENT_DECREMENT_MODE.bits()) >> 4 {
                            0 => IncrementMode::Decrement,
                            1 => IncrementMode::Increment,
                            2 | 3 => IncrementMode::Fixed,
                            _ => unreachable!(),
                        };

                    // Which ports will be written to?
                    if (data & 0x40) != 0 {
                        self.write_order.push(WriteRegister::PortATiming);
                    }
                }
                // Register 2 => 0XXX X000
                else if (data & WR2Mask::SELECT_MASK.bits()) == WR2Mask::SELECT_BITS.bits() {
                    self.port_b_is_memory = (data & WR2Mask::MEMORY_OR_IO.bits()) == 0;

                    self.port_b_increment_mode =
                        match (data & WR2Mask::INCREMENT_DECREMENT_MODE.bits()) >> 4 {
                            0 => IncrementMode::Decrement,
                            1 => IncrementMode::Increment,
                            2 | 3 => IncrementMode::Fixed,
                            _ => unreachable!(),
                        };

                    // Which ports will be written to?
                    if (data & 0x40) != 0 {
                        self.write_order.push(WriteRegister::PortBTiming);
                    }
                }
                // Register 0 => 0XXX XXXX
                else if (data & WR0Mask::SELECT_MASK.bits()) == WR0Mask::SELECT_BITS.bits() {
                    match data & WR0Mask::SEARCH_OR_TRANSFER.bits() {
                        1 => self.transfer = true,
                        2 => self.search = true,
                        3 => {
                            self.transfer = true;
                            self.search = true;
                        }
                        _ => unreachable!(),
                    }

                    if (data & WR0Mask::DIRECTION.bits()) == 0 {
                        self.direction = Direction::PortBToA;
                    } else {
                        self.direction = Direction::PortAToB;
                    }

                    // Which ports will be written to?
                    if (data & 0x40) != 0 {
                        self.write_order.push(WriteRegister::BlockLengthHigh);
                    }
                    if (data & 0x20) != 0 {
                        self.write_order.push(WriteRegister::BlockLengthLow);
                    }
                    if (data & 0x10) != 0 {
                        self.write_order.push(WriteRegister::PortAAddressHigh);
                    }
                    if (data & 0x08) != 0 {
                        self.write_order.push(WriteRegister::PortAAddressLow);
                    }
                }
                // Register 3 => 1XXX XX00
                else if (data & WR3Mask::SELECT_MASK.bits()) == WR3Mask::SELECT_BITS.bits() {
                    self.stop_on_match = (data & WR3Mask::STOP_ON_MATCH.bits()) != 0;

                    // According to the docs, setting this to 0 does not disable interrupts
                    if (data & WR3Mask::INTERRUPT_ENABLE.bits()) != 0 {
                        self.interrupts_enabled = true;
                    }

                    // According to the docs, setting this to 0 does not disable DMA
                    if (data & WR3Mask::DMA_ENABLE.bits()) != 0 {
                        self.enabled = true;
                    }

                    // Which ports will be written to?
                    if (data & 0x10) != 0 {
                        self.write_order.push(WriteRegister::MatchByte);
                    }
                    if (data & 0x08) != 0 {
                        self.write_order.push(WriteRegister::MaskByte);
                    }
                }
                // Register 4 => 1XXX XX01
                else if (data & WR4Mask::SELECT_MASK.bits()) == WR4Mask::SELECT_BITS.bits() {
                    self.access_mode = match (data & WR4Mask::ACCESS_MODE.bits()) >> 5 {
                        0 => AccessMode::Byte,
                        1 => AccessMode::Continuous,
                        3 => AccessMode::Burst,
                        _ => unimplemented!("Attempted to set an impossible DMA access mode: 4"),
                    };

                    // Which ports will be written to?
                    if (data & 0x10) != 0 {
                        self.write_order.push(WriteRegister::InterruptControl);
                    }
                    if (data & 0x08) != 0 {
                        self.write_order.push(WriteRegister::PortBAddressHigh);
                    }
                    if (data & 0x04) != 0 {
                        self.write_order.push(WriteRegister::PortBAddressLow);
                    }
                }
                // Register 5 => 10XX X010
                else if (data & WR5Mask::SELECT_MASK.bits()) == WR5Mask::SELECT_BITS.bits() {
                    if (data & WR5Mask::CHIP_ENABLE_ONLY_WAIT.bits()) != 0 {
                        todo!()
                    }

                    self.restart_at_end_of_block =
                        (data & WR5Mask::STOP_RESTART_ON_END_OF_BLOCK.bits()) != 0;
                }
                // Register 6 => 1XXX XX11
                else if (data & WR6Mask::SELECT_MASK.bits()) == WR6Mask::SELECT_BITS.bits() {
                    // All control bytes disable the DMA... except the enable DMA byte ;-)
                    self.enabled = false;

                    match data {
                        // Reset
                        0xC3 => {
                            // TODO: Do I need to reset other status bits?
                            //   like the found/end and transfer bits?
                            self.interrupts_enabled = false;
                            self.restart_at_end_of_block = false;
                            self.status &= !RR0Mask::INTERRUPT_PENDING.bits();
                        }

                        // Reset Port A Timing
                        0xC7 => todo!(),

                        // Reset Port B Timing
                        0xCB => todo!(),

                        // Load
                        0xCF => {
                            // NOTE: This only loads the source register.
                            // The destination register will get updated during its first
                            // increment!
                            // TODO: Make sure I impl this correctly. Auto-restart is also
                            //   supposed to do this automatically.
                            match self.direction {
                                Direction::PortAToB => {
                                    self.port_a_counter = self.port_a_start_address;
                                }
                                Direction::PortBToA => {
                                    self.port_b_counter = self.port_b_start_address;
                                }
                            }
                            self.byte_counter = 0;
                        }

                        // Continue
                        0xD3 => {
                            self.byte_counter = 0;
                        }

                        // Disable interrupts
                        0xAF => todo!(),

                        // Enable interrupts
                        0xAB => {
                            self.interrupts_enabled = true;
                        }

                        // Reset and disable interrupts
                        0xA3 => todo!(),

                        // Enable after reti
                        0xB7 => todo!(),

                        // Read status byte
                        0xBF => {
                            self.read_order.push(ReadRegister::Status);
                        }

                        // Reinitialize status byte
                        0x8B => {
                            self.status |= RR0Mask::MATCH_NOT_FOUND.bits();
                            self.status |= RR0Mask::NOT_END_OF_BLOCK.bits();
                        }

                        // Read mask follows
                        0xBB => {
                            self.write_order.push(WriteRegister::ReadMask);
                        }

                        // Initiate read sequence
                        0xA7 => {
                            if (self.read_mask & WR6Mask::PORT_B_ADDRESS_HIGH.bits()) != 0 {
                                self.read_order.push(ReadRegister::PortBAddressHigh);
                            }
                            if (self.read_mask & WR6Mask::PORT_B_ADDRESS_LOW.bits()) != 0 {
                                self.read_order.push(ReadRegister::PortBAddressLow);
                            }
                            if (self.read_mask & WR6Mask::PORT_A_ADDRESS_HIGH.bits()) != 0 {
                                self.read_order.push(ReadRegister::PortAAddressHigh);
                            }
                            if (self.read_mask & WR6Mask::PORT_A_ADDRESS_LOW.bits()) != 0 {
                                self.read_order.push(ReadRegister::PortAAddressLow);
                            }
                            if (self.read_mask & WR6Mask::BYTE_COUNTER_LOW.bits()) != 0 {
                                self.read_order.push(ReadRegister::ByteCounterLow);
                            }
                            if (self.read_mask & WR6Mask::BYTE_COUNTER_HIGH.bits()) != 0 {
                                self.read_order.push(ReadRegister::ByteCounterHigh);
                            }
                            if (self.read_mask & WR6Mask::STATUS.bits()) != 0 {
                                self.read_order.push(ReadRegister::Status);
                            }
                        }

                        // Force ready
                        0xB3 => todo!(),

                        // Enable DMA
                        0x87 => {
                            self.enabled = true;
                        }

                        // Disable DMA
                        0x83 => {
                            self.enabled = false;
                        }

                        _ => unimplemented!(
                            "Unrecognized DMA command: {}. It wasn't in the data sheet for z8410.",
                            data
                        ),
                    }
                }
            }

            Some(WriteRegister::PortAAddressLow) => {
                self.port_a_start_address = (self.port_a_start_address & 0xF0) | data as u16;
            }

            Some(WriteRegister::PortAAddressHigh) => {
                self.port_a_start_address =
                    (self.port_a_start_address & 0x0F) | ((data as u16) << 8);
            }

            Some(WriteRegister::BlockLengthLow) => {
                self.block_length = (self.block_length & 0xF0) | data as u16;
            }

            Some(WriteRegister::BlockLengthHigh) => {
                self.block_length = (self.block_length & 0x0F) | ((data as u16) << 8);
            }

            Some(WriteRegister::PortATiming) => todo!(),
            Some(WriteRegister::PortBTiming) => todo!(),

            Some(WriteRegister::PortBAddressLow) => {
                self.port_b_start_address = (self.port_b_start_address & 0xF0) | data as u16;
            }

            Some(WriteRegister::PortBAddressHigh) => {
                self.port_b_start_address =
                    (self.port_b_start_address & 0x0F) | ((data as u16) << 8);
            }

            Some(WriteRegister::InterruptControl) => {
                self.interrupt_on_match = (data & WR4Mask::INTERRUPT_ON_MATCH.bits()) != 0;

                self.interrupt_at_end_of_block =
                    (data & WR4Mask::INTERRUPT_AT_END_OF_BLOCK.bits()) != 0;

                if (data & WR4Mask::PULSE_GENERATED.bits()) != 0 {
                    todo!();
                }

                self.status_affects_vector = (data & WR4Mask::STATUS_AFFECTS_VECTOR.bits()) != 0;

                self.interrupt_on_ready = (data & WR4Mask::INTERRUPT_ON_READY.bits()) != 0;

                // Which ports will be written to?
                if (data & 0x10) != 0 {
                    self.write_order.push(WriteRegister::InterruptVector);
                }
                if (data & 0x08) != 0 {
                    self.write_order.push(WriteRegister::PulseControl);
                }
            }

            Some(WriteRegister::PulseControl) => todo!(),

            Some(WriteRegister::InterruptVector) => {
                self.interrupt_vector = data;
            }

            Some(WriteRegister::ReadMask) => {
                self.read_mask = data;
            }

            Some(WriteRegister::MaskByte) => {
                self.mask_byte = data;
            }

            Some(WriteRegister::MatchByte) => {
                self.match_byte = data;
            }
        }
    }

    fn interrupting(&self) -> bool {
        (self.status & RR0Mask::INTERRUPT_PENDING.bits()) != 0
    }
}
