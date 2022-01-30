use crate::bus::{Bus, Device, DeviceBus};

bitflags::bitflags! {
    struct RR0Mask: u8 {
        const TRANSFER_OCCURRED = 0x01;
        const ACTIVE_READY = 0x02;
        const INTERRUPT_PENDING = 0x08;
        const MATCH_FOUND = 0x10;
        const END_OF_BLOCK = 0x20;
    }
}

bitflags::bitflags! {
    struct WR0Mask: u8 {
        const SEARCH_OR_TRANSFER = 0x03;
        const DIRECTION = 0x04;
    }
}

bitflags::bitflags! {
    struct WR1Mask: u8 {
        const MEMORY_OR_IO = 0x08;
        const INCREMENT_DECREMENT_MODE = 0x30;

        const CYCLE_LENGTH = 0x03;
    }
}

bitflags::bitflags! {
    struct WR2Mask: u8 {
        const MEMORY_OR_IO = 0x08;
        const INCREMENT_DECREMENT_MODE = 0x30;

        const CYCLE_LENGTH = 0x03;
    }
}

bitflags::bitflags! {
    struct WR3Mask: u8 {
        const STOP_ON_MATCH = 0x04;
        const INTERRUPT_ENABLE = 0x20;
        const DMA_ENABLE = 0x40;
    }
}

bitflags::bitflags! {
    struct WR4Mask: u8 {
        const TRANSFER_MODE = 0x06;

        const INTERRUPT_ON_MATCH = 0x01;
        const INTERRUPT_AT_END_OF_BLOCK = 0x02;
        const PULSE_GENERATED = 0x04;

        const INTERRUPT_ON_READY = 0x20;
        const STATUS_AFFECTS_VECTOR = 0x40;

        const INTERRUPT_VECTOR = 0x06;
    }
}

bitflags::bitflags! {
    struct WR5Mask: u8 {
        const READY_ACTIVE_HIGH_LOW = 0x08;
        const CHIP_ENABLE_ONLY_WAIT = 0x10;
        const STOP_RESTART_ON_END_OF_BLOCK = 0x20;
    }
}

bitflags::bitflags! {
    struct WR6Mask: u8 {
        const STATUS = 0x01;
        const BYTE_COUNTER_LOW = 0x02;
        const BYTE_COUNTER_HIGH = 0x04;
        const PORT_A_ADDRESS_LOW = 0x08;
        const PORT_A_ADDRESS_HIGH = 0x10;
        const PORT_B_ADDRESS_LOW = 0x20;
        const PORT_B_ADDRESS_HIGH = 0x40;
    }
}

#[derive(Copy, Clone, Debug)]
enum Command {
    Reset = 0xC3,
    ResetPortATiming = 0xC7,
    ResetPortBTiming = 0xCD,
    Load = 0xCF,
    Continue = 0xD3,
    DisableInterrupts = 0xAF,
    EnableInterrupts = 0xAB,
    ResetAndDisableInterrupts = 0xA3,
    EnableAfterReti = 0xB7,
    ReadStatusByte = 0xBF,
    ReinitializeStatusByte = 0x8B,
    InitiateReadSequence = 0xA7,
    ForceReady = 0xB3,
    EnableDMA = 0x87,
    DisableDMA = 0x83,
    ReadMaskFollows = 0xBB,
}

pub struct Dma {
    rr: [u8; 6],
    active_read_register: usize,

    wr: [u8; 6],
    active_write_register: usize,
}

impl Device for Dma {
    fn tick(&mut self, bus: &mut impl DeviceBus) {
        todo!()
    }

    fn read(&mut self, _: u16) -> u8 {
        todo!()
    }

    fn write(&mut self, _: u16, data: u8) {
        todo!()
    }
}
