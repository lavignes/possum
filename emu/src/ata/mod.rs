//! Class 1 Compact Flash emulation (8-bit ATA (LBA) mode only)

use std::{error, ops::IndexMut};

use crate::bus::{Device, DeviceBus};

struct Status;
impl Status {
    /// Last operation was an error
    const ERR: u8 = 0x01;

    /// A data error was corrected
    const CORR: u8 = 0x04;

    /// Requesting data from host
    const DRQ: u8 = 0x08;

    /// Compact flash ready
    const DSC: u8 = 0x10;

    /// Write fault
    const DWF: u8 = 0x20;

    /// Ready to accept first command
    const RDY: u8 = 0x40;

    /// Busy executing command
    const BUSY: u8 = 0x80;
}

struct Error;
impl Error {
    /// Address mark not found (generic r/w bit)
    const AMNF: u8 = 0x01;

    /// Aborted (unsupported operation)
    const ABRT: u8 = 0x04;

    /// ID not found (bad lba)
    const IDNF: u8 = 0x10;

    /// Uncorrectable (data error)
    const UNC: u8 = 0x40;

    /// Bad block (bad sector)
    const BBK: u8 = 0x80;
}

#[derive(Debug)]
enum CommandState {
    None,
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
struct Card<M> {
    mmap: M,
    device_info: Vec<u8>,
    interrupt: bool,
    interrupt_enabled: bool,
    is_8_bit: bool,

    state: CommandState,
    error: u8,
    status: u8,
    lba: u32,
    sector_offset: usize,
}

#[derive(Debug, Default)]
struct SharedRegisters {
    feature: u8,
    sector_count: u8,
    sector_number: u8,
    cylinder_low: u8,
    cylinder_high: u8,
    drive_head: u8,
}

impl<M: MemoryMap> Card<M> {
    const SECTOR_SIZE: usize = 512;

    fn device_info(disk_size: usize) -> Vec<u8> {
        let mut info = vec![0; Self::SECTOR_SIZE];

        let cf_card_sig = 0x848A_u16;
        for (i, b) in cf_card_sig.to_le_bytes().iter().enumerate() {
            info[0 + i] = *b;
        }

        let sector_count = (disk_size / Self::SECTOR_SIZE) as u32;
        for (i, b) in sector_count.to_le_bytes().iter().enumerate() {
            info[14 + i] = *b;
        }

        let serial_number = b"0-12345-67890-123456";
        for (i, b) in serial_number.iter().enumerate() {
            info[20 + i] = *b; // note: it is right-justified ending at 39
        }

        info[44] = 0x04; // defined by spec

        let firmware_revision = b"POSSUM01";
        for (i, b) in firmware_revision.iter().enumerate() {
            info[46 + i] = *b;
        }

        let model_number = b"POSSUM-CF-CARD-EMULATOR-01              ";
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

    fn new(mmap: M) -> Self {
        let device_info = Self::device_info(mmap.len());
        Self {
            mmap,
            device_info,

            interrupt: false,
            interrupt_enabled: false,
            is_8_bit: false,

            state: CommandState::None,
            error: 0,
            status: Status::RDY | Status::DSC, // assume always ready
            lba: 0,
            sector_offset: 0,
        }
    }

    fn read_data(&mut self) -> u8 {
        if (self.status & Status::BUSY) != 0 {
            return 0;
        }

        match self.state {
            CommandState::None => 0,

            CommandState::IdentifyDevice => {
                let data = self.device_info[self.sector_offset];
                self.sector_offset += 1;
                if self.sector_offset == Self::SECTOR_SIZE {
                    self.error = 0;
                    self.status &= !Status::DRQ;
                    self.state = CommandState::None;
                }
                data
            }

            CommandState::ReadSectors => {
                let offset = ((self.lba as usize) * Self::SECTOR_SIZE) + self.sector_offset;
                let data = self.mmap[offset];
                self.sector_offset += 1;
                if self.sector_offset == Self::SECTOR_SIZE {
                    self.status &= !Status::DRQ;
                    self.state = CommandState::None;
                }
                data
            }

            CommandState::WriteSectors => 0,
        }
    }

    fn write_data(&mut self, data: u8) {
        if (self.status & Status::BUSY) != 0 {
            return;
        }

        match self.state {
            CommandState::None => {}

            CommandState::IdentifyDevice => {}

            CommandState::ReadSectors => {}

            CommandState::WriteSectors => {
                let offset = ((self.lba as usize) * Self::SECTOR_SIZE) + self.sector_offset;
                self.mmap[offset] = data;

                self.sector_offset += 1;
                if self.sector_offset == Self::SECTOR_SIZE {
                    if self.mmap.flush().is_err() {
                        self.error |= Error::AMNF | Error::BBK;
                        self.status &= !(Status::BUSY | Status::DRQ);
                        self.status |= Status::ERR;
                    }
                }
            }
        }
    }

    fn write_command(&mut self, registers: &SharedRegisters, data: u8) {
        if (self.status & Status::BUSY) != 0 {
            return;
        }

        // Latch the LBA (and sector count if we want to support it)
        #[rustfmt::skip]
        {
            self.lba = (self.lba & 0xFFFFFF00) | ((registers.sector_number as u32) << 0);        // [0:7]
            self.lba = (self.lba & 0xFFFF00FF) | ((registers.cylinder_low as u32) << 8);         // [8:15]
            self.lba = (self.lba & 0xFF00FFFF) | ((registers.cylinder_high as u32) << 16);       // [16:23]
            self.lba = (self.lba & 0x00FFFFFF) | (((registers.drive_head & 0x0F) as u32) << 24); // [24:27]
        }

        // Clear errors :-)
        self.error = 0;
        self.status &= !Status::ERR;

        match data {
            // Execute drive diagnostic
            0x90 => {
                // 1 means no error detected
                self.error = 0x01;
            }

            // Erase sectors
            0xC0 => {
                // check for LBA mode
                if (registers.drive_head & 0x40) == 0 {
                    self.status |= Status::ERR;
                    self.error |= Error::AMNF | Error::IDNF | Error::ABRT;
                    return;
                }

                let offset = (self.lba as usize) * Self::SECTOR_SIZE;
                for i in 0..Self::SECTOR_SIZE {
                    self.mmap[offset + i] = 0xFF;
                }
                if self.mmap.flush().is_err() {
                    self.error |= Error::AMNF | Error::BBK;
                    self.status |= Status::ERR;
                }
            }

            // Identify device
            0xEC => {
                self.interrupt = true;
                self.sector_offset = 0;
                self.status |= Status::DRQ;
                self.state = CommandState::IdentifyDevice;
            }

            // Nop
            0x00 => {
                // Always aborts
                self.status |= Status::ERR;
                self.error |= Error::AMNF | Error::ABRT;
            }

            // Read sectors
            0x20 | 0x21 => {
                // check for LBA mode
                if (registers.drive_head & 0x40) == 0 {
                    self.status |= Status::ERR;
                    self.error |= Error::AMNF | Error::IDNF | Error::ABRT;
                    return;
                }

                self.interrupt = true;
                self.sector_offset = 0;

                let offset = ((self.lba as usize) * Self::SECTOR_SIZE) + self.sector_offset;
                if self.mmap.len() < offset {
                    self.error |= Error::AMNF | Error::IDNF;
                    self.status |= Status::ERR;
                    return;
                }

                self.status |= Status::DRQ;
                self.state = CommandState::ReadSectors;
            }

            // Read verify sectors
            0x40 | 0x41 => {
                // check for LBA mode
                if (registers.drive_head & 0x40) == 0 {
                    self.status |= Status::ERR;
                    self.error |= Error::AMNF | Error::IDNF | Error::ABRT;
                    return;
                }

                // Does nothing. We assume the disk is fine :-)
                self.interrupt = true;
            }

            // Request sense
            0x03 => todo!(),

            // Set features
            0xEF => {
                match registers.feature {
                    // Enable 8-bit
                    0x01 => self.is_8_bit = true,

                    // Disable 8-bit
                    0x02 => self.is_8_bit = false,

                    _ => {
                        self.status |= Status::ERR;
                        self.error |= Error::AMNF | Error::ABRT;
                    }
                }
            }

            // Write sectors
            0x30 | 0x31 | 0x38 | 0x3C => {
                // check for LBA mode
                if (registers.drive_head & 0x40) == 0 {
                    self.status |= Status::ERR;
                    self.error |= Error::AMNF | Error::IDNF | Error::ABRT;
                    return;
                }

                self.sector_offset = 0;
                self.status |= Status::DRQ;
                self.state = CommandState::WriteSectors;
            }

            // Invalid command
            _ => {
                self.status |= Status::ERR;
                self.error |= Error::AMNF | Error::ABRT
            }
        }
    }
}

#[derive(Debug)]
pub struct CardBus<M> {
    card0: Card<M>,
    card1: Option<Card<M>>,
    registers: SharedRegisters,
}

impl<M: MemoryMap> CardBus<M> {
    #[inline]
    pub fn single(mmap: M) -> Self {
        Self::new(mmap, None)
    }

    #[inline]
    pub fn dual(mmap0: M, mmap1: M) -> Self {
        Self::new(mmap0, Some(mmap1))
    }

    #[inline]
    fn new(mmap0: M, mmap1: Option<M>) -> Self {
        Self {
            card0: Card::new(mmap0),
            card1: mmap1.map(|mmap| Card::new(mmap)),
            registers: SharedRegisters::default(),
        }
    }
}

impl<M: MemoryMap> Device for CardBus<M> {
    fn tick(&mut self, _: &mut dyn DeviceBus) {}

    fn read(&mut self, port: u16) -> u8 {
        match port & 0x07 {
            // Data port
            0 => {
                if (self.registers.drive_head & 0x10) == 0 {
                    self.card0.read_data()
                } else {
                    match self.card1.as_mut() {
                        Some(card1) => card1.read_data(),
                        _ => 0,
                    }
                }
            }

            // Error Code
            1 => {
                if (self.registers.drive_head & 0x10) == 0 {
                    self.card0.error
                } else {
                    match self.card1.as_ref() {
                        Some(card1) => card1.error,
                        _ => 0,
                    }
                }
            }

            // Sector Count
            2 => self.registers.sector_count,

            // a.k.a. Logical Block Address (LBA) [0:7]
            3 => self.registers.sector_number,

            // a.k.a LBA [8:15]
            4 => self.registers.cylinder_low,

            // a.k.a LBA [16:23]
            5 => self.registers.cylinder_high,

            // a.k.a. LBA [24:27]
            6 => self.registers.drive_head,

            // Status
            7 => {
                // Reading the status clears the interrupts apparently
                if (self.registers.drive_head & 0x10) == 0 {
                    self.card0.interrupt = false;
                    self.card0.status
                } else {
                    match self.card1.as_mut() {
                        Some(card1) => {
                            card1.interrupt = false;
                            card1.status
                        }
                        _ => 0,
                    }
                }
            }

            _ => unreachable!(),
        }
    }

    fn write(&mut self, port: u16, data: u8) {
        match port & 0x07 {
            // Data port
            0 => {
                if (self.registers.drive_head & 0x10) == 0 {
                    self.card0.write_data(data)
                } else {
                    match self.card1.as_mut() {
                        Some(card1) => card1.write_data(data),
                        _ => {}
                    }
                }
            }

            // Feature
            1 => self.registers.feature = data,

            // Sector Count
            2 => self.registers.sector_count = data,

            // a.k.a. Logical Block Address (LBA) [0:7]
            3 => self.registers.sector_number = data,

            // a.k.a LBA [8:15]
            4 => self.registers.cylinder_low = data,

            // a.k.a LBA [16:23]
            5 => self.registers.cylinder_high = data,

            // a.k.a. LBA [24:27]
            6 => self.registers.drive_head = data,

            // Command
            7 => {
                let Self {
                    card0,
                    card1,
                    registers,
                    ..
                } = self;
                if (registers.drive_head & 0x10) == 0 {
                    card0.write_command(&registers, data)
                } else {
                    match card1.as_mut() {
                        Some(card1) => card1.write_command(&registers, data),
                        _ => {}
                    }
                }
            }

            _ => unreachable!(),
        }
    }

    fn interrupting(&self) -> bool {
        if self.card0.interrupt_enabled && self.card0.interrupt {
            return true;
        }
        match self.card1.as_ref() {
            Some(card1) => card1.interrupt_enabled && card1.interrupt,
            _ => false,
        }
    }
}
