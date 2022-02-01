//! Class 1 Compact Flash emulation (8-bit ATA (LBA) mode only)

use crate::bus::{Device, DeviceBus};

bitflags::bitflags! {
    struct Status: u8 {
        const INTERRUPT = 0x02;
        const POWER_DOWN = 0x04;
        const AUDIO = 0x08;
        const POWER_LEVEL_1_ENABLED = 0x10;
        const IO_IS_8_BIT = 0x20;
        const SIGNAL_CHANGE = 0x40;
        const CHANGED = 0x80;
    }
}

bitflags::bitflags! {
    struct Error: u8 {
        const ADDRESS_MARK_NOT_FOUND = 0x01;
        const ABORT = 0x04;
        const ID_NOT_FOUND = 0x10;
        const UNCORRECTABLE = 0x40;
        const BAD_BLOCK = 0x80;
    }
}

#[derive(Debug)]
enum CommandState {
    Ready,
    EraseSectors,
    IdentifyDevice,
}

impl Default for CommandState {
    fn default() -> Self {
        Self::Ready
    }
}

#[derive(Debug, Default)]
pub struct CFCard {
    state: CommandState,
    data: u8,
    error: u8,
    status: u8,
    sector_count: u8,
    sector_number: u8,
    cylinder_low: u8,
    cylinder_high: u8,
    drive_head: u8,

    lba_mode: bool,
    is_drive_1: bool,
    lba_latch: u32,
    sector: usize,
    sector_offset: usize,
}

impl Device for CFCard {
    fn tick(&mut self, _: &mut dyn DeviceBus) {
        if self.interrupt() {
            return;
        }

        match self.state {
            CommandState::Ready => {}

            CommandState::EraseSectors => {}
        }
    }

    fn read(&mut self, port: u16) -> u8 {
        match port & 0x03 {
            // Data port
            0 => self.data,

            // Error Code
            1 => self.error,

            // Sector Count
            2 => self.sector_count,

            // a.k.a. Logical Block Address (LBA) [0:7]
            3 => self.sector_number,

            // a.k.a LBA [8:15]
            4 => self.cylinder_low,

            // a.k.a LBA [16:23]
            5 => self.cylinder_high,

            // a.k.a. LBA [24:27]
            6 => self.drive_head,

            // Status
            7 => self.status,

            _ => unreachable!(),
        }
    }

    fn write(&mut self, port: u16, data: u8) {
        match port & 0x03 {
            // Data port
            0 => self.data = data,

            // Feature
            1 => todo!(),

            // Sector Count
            2 => self.sector_count = data,

            // 3 => self.lba |= ,
            //
            // // LBA [8:15]
            // 4 => self.lba |= (self.lba & 0xFFFF00FF) | ((data as u32) << 8),
            //
            // 5 => self.lba |= (self.lba & 0xFF00FFFF) | ((data as u32) << 16),
            //
            // // LBA [24:27]
            // 6 => self.lba |= (self.lba & 0x00FFFFFF) | ((data as u32) << 24),

            // a.k.a. Logical Block Address (LBA) [0:7]
            3 => self.sector_number = data,

            // a.k.a LBA [8:15]
            4 => self.cylinder_low = data,

            // a.k.a LBA [16:23]
            5 => self.cylinder_high = data,

            // a.k.a. LBA [24:27]
            6 => self.drive_head = data,

            // Command
            7 => {
                // In CF-ATA there can only be 2 drives on the bus.
                // Bit 4 in the drive head register selects the active drive (0 or 1).
                // Interestingly, both drives still get their registers written to,
                // but ignore commands not targeted to them.
                if self.is_drive_1 == ((self.drive_head & 0x10) != 0) {
                    return;
                }

                // Check if LBA is enabled. For now, we require it to always be 1. Some
                // commands don't care about the bit, and we could probably only blow up
                // in cases where it has an affect... But for now, always check for LBA flag.
                if (self.drive_head & 0x40) == 0 {
                    unimplemented!("Attempted to write a command ({}) not in LBA mode (drive head bit 6). This emulation only supports LBA mode.", data);
                }

                // Latch the LBA register
                #[rustfmt::skip]
                {
                    self.lba_latch = (self.lba_latch & 0xFFFFFF00) | ((self.sector_number as u32) << 0);  // [0:7]
                    self.lba_latch = (self.lba_latch & 0xFFFF00FF) | ((self.cylinder_low as u32) << 8);   // [8:15]
                    self.lba_latch = (self.lba_latch & 0xFF00FFFF) | ((self.cylinder_high as u32) << 16); // [16:23]
                    self.lba_latch = (self.lba_latch & 0xF0FFFFFF) | ((self.drive_head as u32) << 24);    // [24:27]
                }

                match data {
                    // Check power mode
                    0xE5 | 0x98 => {
                        // Our card is always in idle mode. (no power saving) :-)
                        self.status |= Status::INTERRUPT.bits();
                        self.sector_count = 0xFF;
                    }

                    // Execute drive diagnostic
                    0x90 => {
                        // 1 means no error detected
                        self.error = 0x01;
                    }

                    // Erase sectors
                    0xC0 => {
                        self.sector = 0;
                        self.sector_offset = 0;
                        self.state = CommandState::EraseSectors;
                    }

                    // Flush cache
                    0xE7 => {
                        // We support the command, but we emulate a write-through cache
                    }

                    // Identify device
                    0xEC => {
                        self.sector_offset = 0;
                        self.state = CommandState::IdentifyDevice;
                    }

                    _ => todo!(),
                }
            }

            _ => unreachable!(),
        }
    }

    fn interrupt(&self) -> bool {
        (self.status & Status::INTERRUPT.bits()) != 0
    }

    fn interrupt_vector(&self) -> u8 {
        // TODO: AFAICT, interrupts are low-level and won't populate the data bus.
        //   We should probably assume that interrupts will drive a register that stores
        //   the interrupt at a fixed value.
        todo!()
    }

    fn ack_interrupt(&mut self) {
        self.status &= !Status::INTERRUPT.bits();
    }
}
