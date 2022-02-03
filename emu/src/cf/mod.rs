//! Class 1 Compact Flash emulation (8-bit ATA (LBA) mode only)

use std::{error, ops::IndexMut};

use crate::bus::{Device, DeviceBus};

bitflags::bitflags! {
    struct Status: u8 {
        const ERR = 0x01;
        const CORR = 0x04;
        const DRQ = 0x08;
        const DSC = 0x10;
        const DWF = 0x20;
        const RDY = 0x40;
        const BUSY = 0x80;
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
    None,
    EraseSectors,
    IdentifyDevice,
    ReadSectors,
    WriteSectors,
}

pub trait MemoryMap: IndexMut<usize, Output = u8> {
    type Error: error::Error;

    fn flush(&mut self) -> Result<(), Self::Error>;

    fn len(&self) -> usize;
}

#[derive(Debug)]
pub struct CFCard<M> {
    mmap: M,
    is_drive_1: bool,
    device_info: Vec<u8>,

    interrupt: bool,
    interrupt_enabled: bool,
    interrupt_vector: u8,
    is_8_bit: bool,

    state: CommandState,
    feature: u8,
    error: u8,
    status: u8,
    sector_count: u8,
    sector_number: u8,
    cylinder_low: u8,
    cylinder_high: u8,
    drive_head: u8,

    lba_latch: u32,
    sector_offset: usize,
}

impl<M: MemoryMap> CFCard<M> {
    pub fn primary(mmap: M) -> Self {
        Self::new(false, mmap)
    }

    pub fn secondary(mmap: M) -> Self {
        Self::new(true, mmap)
    }

    fn device_info(disk_size: usize) -> Vec<u8> {
        let mut info = vec![0; 512];

        let cf_card_sig = 0x848A_u16;
        for (i, b) in cf_card_sig.to_le_bytes().iter().enumerate() {
            info[0 + i] = *b;
        }

        let sector_count = (disk_size / 512) as u32;
        for (i, b) in sector_count.to_le_bytes().iter().enumerate() {
            info[14 + i] = *b;
        }

        let serial_number = b"possum-cf-123456";
        for (i, b) in serial_number.iter().rev().enumerate() {
            info[39 - i] = *b; // note: it is right-justified ending at 39
        }

        info[44] = 0x04; // defined by spec

        let firmware_revision = b"poss01";
        for (i, b) in firmware_revision.iter().enumerate() {
            info[46 + i] = *b;
        }

        let model_number = b"possum-cf-123456";
        for (i, b) in model_number.iter().enumerate() {
            info[54 + i] = *b;
        }

        let max_multiple_sectors = 0x0001_u16;
        for (i, b) in max_multiple_sectors.to_le_bytes().iter().enumerate() {
            info[94 + i] = *b;
        }

        info[99] = 0x02; // LBA supported

        info[118] = 0x01; // sectors per interrupt for multiple read/write (though not enabled)
        info[119] = 0x00; // multiple sector read/writes *NOT* allowed

        for (i, b) in sector_count.to_le_bytes().iter().enumerate() {
            info[120 + i] = *b;
        }

        // I technically don't need to set any capability flags

        info
    }

    fn new(is_drive_1: bool, mmap: M) -> Self {
        let device_info = Self::device_info(mmap.len());
        Self {
            mmap,
            is_drive_1,
            device_info,

            interrupt: false,
            interrupt_enabled: false,
            interrupt_vector: 0,
            is_8_bit: false,

            state: CommandState::None,
            feature: 0,
            error: 0,
            status: Status::RDY.bits() | Status::DSC.bits(), // assume always ready
            sector_count: 0,
            sector_number: 0,
            cylinder_low: 0,
            cylinder_high: 0,
            drive_head: 0,

            lba_latch: 0,
            sector_offset: 0,
        }
    }
}

impl<M: MemoryMap> Device for CFCard<M> {
    fn tick(&mut self, _: &mut dyn DeviceBus) {}

    fn read(&mut self, port: u16) -> u8 {
        // As a special case, if the device is busy and we get a read,
        // only the status is available.
        if (self.status | Status::BUSY.bits()) != 0 {
            if port & 0x03 == 7 {
                return self.status;
            }
            return 0;
        }

        match port & 0x03 {
            // Data port
            0 => match self.state {
                CommandState::None => 0,

                CommandState::EraseSectors => 0,

                CommandState::IdentifyDevice => {
                    let data = self.device_info[self.sector_offset];
                    self.sector_offset += 1;
                    if self.sector_offset == 512 {
                        self.error = 0;
                        self.status &= !(Status::BUSY.bits() | Status::DRQ.bits());
                    }
                    data
                }

                CommandState::ReadSectors => {
                    let offset = ((self.lba_latch as usize) * 512) + self.sector_offset;
                    let data = self.mmap[offset];

                    self.sector_offset += 1;
                    if self.sector_offset == 512 {
                        self.status &= !(Status::BUSY.bits() | Status::DRQ.bits());
                    }
                    data
                }

                CommandState::WriteSectors => 0,
            },

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
            7 => {
                self.interrupt = false;
                self.status
            }

            _ => unreachable!(),
        }
    }

    fn write(&mut self, port: u16, data: u8) {
        if (self.status | Status::BUSY.bits()) != 0 {
            return;
        }

        match port & 0x03 {
            // Data port
            0 => match self.state {
                CommandState::None => {}

                CommandState::EraseSectors => {}

                CommandState::IdentifyDevice => {}

                CommandState::ReadSectors => {}

                CommandState::WriteSectors => {
                    let offset = ((self.lba_latch as usize) * 512) + self.sector_offset;
                    self.mmap[offset] = data;

                    self.sector_offset += 1;
                    if self.sector_offset == 512 {
                        if self.mmap.flush().is_err() {
                            self.error |=
                                Error::ADDRESS_MARK_NOT_FOUND.bits() | Error::BAD_BLOCK.bits();
                            self.status &= !(Status::BUSY.bits() | Status::DRQ.bits());
                            self.status |= Status::ERR.bits();
                        }
                    }
                }
            },

            // Feature
            1 => self.feature = data,

            // Sector Count
            2 => self.sector_count = data,

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
                // TODO: Move this check into commands that have the LBA bit.
                if (self.drive_head & 0x40) == 0 {
                    unimplemented!("Attempted to write a command ({}) not in LBA mode (drive head bit 6). This emulation only supports LBA mode.", data);
                }

                // Latch the LBA and sector count
                #[rustfmt::skip]
                {
                    self.lba_latch = (self.lba_latch & 0xFFFFFF00) | ((self.sector_number as u32) << 0);  // [0:7]
                    self.lba_latch = (self.lba_latch & 0xFFFF00FF) | ((self.cylinder_low as u32) << 8);   // [8:15]
                    self.lba_latch = (self.lba_latch & 0xFF00FFFF) | ((self.cylinder_high as u32) << 16); // [16:23]
                    self.lba_latch = (self.lba_latch & 0xF0FFFFFF) | ((self.drive_head as u32) << 24);    // [24:27]
                }

                // Clear errors :-)
                self.error = 0;
                self.status &= !Status::ERR.bits();

                match data {
                    // Check power mode
                    0xE5 | 0x98 => unimplemented!("Idle and sleep modes are not supported"),

                    // Execute drive diagnostic
                    0x90 => {
                        // 1 means no error detected
                        self.error = 0x01;
                        self.status |= Status::RDY.bits();
                    }

                    // Erase sectors
                    0xC0 => {
                        self.sector_offset = 0;
                        self.status |= Status::RDY.bits();
                        self.state = CommandState::EraseSectors;
                    }

                    // Flush cache
                    0xE7 => {
                        // Not supported
                        self.status |= Status::RDY.bits() | Status::ERR.bits();
                        self.error |= Error::ADDRESS_MARK_NOT_FOUND.bits() | Error::ABORT.bits();
                    }

                    // Format track
                    0x50 => unimplemented!("Format track command not supported"),

                    // Identify device
                    0xEC => {
                        self.interrupt = true;
                        self.sector_offset = 0;
                        self.status |= Status::RDY.bits() | Status::DRQ.bits();
                        self.state = CommandState::IdentifyDevice;
                    }

                    // Initialize drive parameters
                    0x91 => unimplemented!(
                        "Attempted to initialize drive parameters (A non-LBA) command"
                    ),

                    // Nop
                    0x00 => {
                        // Always aborts
                        self.status |= Status::RDY.bits() | Status::ERR.bits();
                        self.error |= Error::ADDRESS_MARK_NOT_FOUND.bits() | Error::ABORT.bits();
                    }

                    // Read DMA
                    0xC8 => {
                        // DMA is not supported by 8-bit mode
                        if self.is_8_bit {
                            self.status |= Status::RDY.bits() | Status::ERR.bits();
                            self.error |=
                                Error::ADDRESS_MARK_NOT_FOUND.bits() | Error::ABORT.bits();
                        }
                        unimplemented!("DMA isn't supported because only 8-bit mode is supported");
                    }

                    // Read long sector
                    0x22 | 0x23 => unimplemented!("Read long sector is not supported"),

                    // Read sectors
                    0x20 | 0x21 => {
                        self.interrupt = true;
                        self.sector_offset = 0;
                        self.status |= Status::RDY.bits() | Status::DRQ.bits();
                        self.state = CommandState::ReadSectors;

                        let offset = ((self.lba_latch as usize) * 512) + self.sector_offset;
                        if self.mmap.len() < offset {
                            self.error |=
                                Error::ADDRESS_MARK_NOT_FOUND.bits() | Error::ID_NOT_FOUND.bits();
                            self.status &= !(Status::BUSY.bits() | Status::DRQ.bits());
                            self.status |= Status::ERR.bits();
                        }
                    }

                    // Read verify sectors
                    0x40 | 0x41 => {
                        // Does nothing. We assume the disk is fine :-)
                        self.interrupt = true;
                        self.status |= Status::RDY.bits();
                    }

                    // Recalibrate
                    0x10..=0x1F => {
                        // It acts like a nop
                        self.status |= Status::RDY.bits() | Status::ERR.bits();
                        self.error |= Error::ADDRESS_MARK_NOT_FOUND.bits() | Error::ABORT.bits();
                    }

                    // Request sense
                    0x03 => todo!(),

                    // Security disable password
                    0xF6 => unimplemented!("Security features are not supported"),

                    // Security erase prepare
                    0xF3 => unimplemented!("Security features are not supported"),

                    // Security erase unit
                    0xF4 => unimplemented!("Security features are not supported"),

                    // Security freeze lock
                    0xF5 => unimplemented!("Security features are not supported"),

                    // Security set password
                    0xF1 => unimplemented!("Security features are not supported"),

                    // Security unlock
                    0xF2 => unimplemented!("Security features are not supported"),

                    // Set features
                    0xEF => {
                        match self.feature {
                            // Enable 8-bit
                            0x01 => self.is_8_bit = true,

                            // Disable 8-bit
                            0x02 => self.is_8_bit = false,

                            _ => {
                                self.status |= Status::RDY.bits() | Status::ERR.bits();
                                self.error |=
                                    Error::ADDRESS_MARK_NOT_FOUND.bits() | Error::ABORT.bits();
                            }
                        }
                    }

                    // Set sleep mode
                    0x99 | 0xE6 => unimplemented!("Sleep mode is not supported"),

                    // Standby
                    0x96 | 0xE2 => unimplemented!("Standby mode is not supported"),

                    // Standby immediate
                    0x94 | 0xE0 => unimplemented!("Standby mode is not supported"),

                    // Translate sector
                    0x87 => unimplemented!("Translate sector is not supported"),

                    // Write DMA
                    0xCA => {
                        // DMA is not supported by 8-bit mode
                        if self.is_8_bit {
                            self.status |= Status::RDY.bits() | Status::ERR.bits();
                            self.error |=
                                Error::ADDRESS_MARK_NOT_FOUND.bits() | Error::ABORT.bits();
                        }
                        unimplemented!("DMA isn't supported because only 8-bit mode is supported");
                    }

                    // Write long sector
                    0x32 | 0x33 => unimplemented!("Write long sector is not supported"),

                    // Write sectors
                    0x30 | 0x31 | 0x38 | 0x3C => {
                        self.sector_offset = 0;
                        self.status |= Status::RDY.bits() | Status::DRQ.bits();
                        self.state = CommandState::WriteSectors;
                    }

                    // Invalid command
                    _ => {
                        self.status |= Status::RDY.bits() | Status::ERR.bits();
                        self.error |= Error::ADDRESS_MARK_NOT_FOUND.bits() | Error::ABORT.bits()
                    }
                }
            }

            _ => unreachable!(),
        }
    }

    fn interrupt(&self) -> bool {
        if self.interrupt_enabled {
            self.interrupt
        } else {
            false
        }
    }

    fn interrupt_vector(&self) -> u8 {
        // AFAICT, interrupts are low-level and won't populate the data bus.
        self.interrupt_vector
    }

    fn ack_interrupt(&mut self) {
        self.interrupt = false;
    }
}
