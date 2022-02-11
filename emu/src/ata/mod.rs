//! Class 1 Compact Flash emulation (8-bit ATA (LBA) mode only)

use std::{error, ops::IndexMut};

use crate::bus::{Device, DeviceBus};

bitflags::bitflags! {
    struct Status: u8 {
        // Last operation was an error
        const ERR = 0x01;

        // A data error was corrected
        const CORR = 0x04;

        // Requesting data from host
        const DRQ = 0x08;

        // Compact flash ready
        const DSC = 0x10;

        // Write fault
        const DWF = 0x20;

        // Ready to accept first command
        const RDY = 0x40;

        // Busy executing command
        const BUSY = 0x80;
    }
}

bitflags::bitflags! {
    struct Error: u8 {
        // Address mark not found (generic r/w bit)
        const AMNF = 0x01;

        // Aborted (unsupported operation)
        const ABRT = 0x04;

        // ID not found (bad lba)
        const IDNF = 0x10;

        // Uncorrectable (data error)
        const UNC = 0x40;

        // Bad block (bad sector)
        const BBK = 0x80;
    }
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
    interrupt_pending: bool,
    interrupt_enabled: bool,
    interrupt_vector: u8,
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

        let serial_number = b"0-12345-67890-123456";
        for (i, b) in serial_number.iter().enumerate() {
            info[40 - serial_number.len() + i] = *b; // note: it is right-justified ending at 39
        }

        info[44] = 0x04; // defined by spec

        let firmware_revision = b"POSSUM01";
        for (i, b) in firmware_revision.iter().enumerate() {
            info[46 + i] = *b;
        }

        let model_number = b"POSSUM-CF-CARD-EMULATOR-01";
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
            interrupt_pending: false,
            interrupt_vector: 0,
            is_8_bit: false,

            state: CommandState::None,
            error: 0,
            status: Status::RDY.bits() | Status::DSC.bits(), // assume always ready
            lba: 0,
            sector_offset: 0,
        }
    }

    fn read_data(&mut self) -> u8 {
        if (self.status & Status::BUSY.bits()) != 0 {
            return 0;
        }

        match self.state {
            CommandState::None => 0,

            CommandState::IdentifyDevice => {
                let data = self.device_info[self.sector_offset];
                self.sector_offset += 1;
                if self.sector_offset == 512 {
                    self.error = 0;
                    self.status &= !Status::DRQ.bits();
                    self.state = CommandState::None;
                }
                data
            }

            CommandState::ReadSectors => {
                let offset = ((self.lba as usize) * 512) + self.sector_offset;
                let data = self.mmap[offset];
                self.sector_offset += 1;
                if self.sector_offset == 512 {
                    self.status &= !Status::DRQ.bits();
                    self.state = CommandState::None;
                }
                data
            }

            CommandState::WriteSectors => 0,
        }
    }

    fn write_data(&mut self, data: u8) {
        if (self.status & Status::BUSY.bits()) != 0 {
            return;
        }

        match self.state {
            CommandState::None => {}

            CommandState::IdentifyDevice => {}

            CommandState::ReadSectors => {}

            CommandState::WriteSectors => {
                let offset = ((self.lba as usize) * 512) + self.sector_offset;
                self.mmap[offset] = data;

                self.sector_offset += 1;
                if self.sector_offset == 512 {
                    if self.mmap.flush().is_err() {
                        self.error |= Error::AMNF.bits() | Error::BBK.bits();
                        self.status &= !(Status::BUSY.bits() | Status::DRQ.bits());
                        self.status |= Status::ERR.bits();
                    }
                }
            }
        }
    }

    fn write_command(&mut self, registers: &SharedRegisters, data: u8) {
        if (self.status & Status::BUSY.bits()) != 0 {
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
        self.status &= !Status::ERR.bits();

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
                    self.status |= Status::ERR.bits();
                    self.error |= Error::AMNF.bits() | Error::IDNF.bits() | Error::ABRT.bits();
                    return;
                }

                let offset = (self.lba as usize) * 512;
                for i in 0..512 {
                    self.mmap[offset + i] = 0xFF;
                }
                if self.mmap.flush().is_err() {
                    self.error |= Error::AMNF.bits() | Error::BBK.bits();
                    self.status |= Status::ERR.bits();
                }
            }

            // Identify device
            0xEC => {
                self.interrupt = true;
                self.sector_offset = 0;
                self.status |= Status::DRQ.bits();
                self.state = CommandState::IdentifyDevice;
            }

            // Nop
            0x00 => {
                // Always aborts
                self.status |= Status::ERR.bits();
                self.error |= Error::AMNF.bits() | Error::ABRT.bits();
            }

            // Read sectors
            0x20 | 0x21 => {
                // check for LBA mode
                if (registers.drive_head & 0x40) == 0 {
                    self.status |= Status::ERR.bits();
                    self.error |= Error::AMNF.bits() | Error::IDNF.bits() | Error::ABRT.bits();
                    return;
                }

                self.interrupt = true;
                self.sector_offset = 0;

                let offset = ((self.lba as usize) * 512) + self.sector_offset;
                if self.mmap.len() < offset {
                    self.error |= Error::AMNF.bits() | Error::IDNF.bits();
                    self.status |= Status::ERR.bits();
                    return;
                }

                self.status |= Status::DRQ.bits();
                self.state = CommandState::ReadSectors;
            }

            // Read verify sectors
            0x40 | 0x41 => {
                // check for LBA mode
                if (registers.drive_head & 0x40) == 0 {
                    self.status |= Status::ERR.bits();
                    self.error |= Error::AMNF.bits() | Error::IDNF.bits() | Error::ABRT.bits();
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
                        self.status |= Status::ERR.bits();
                        self.error |= Error::AMNF.bits() | Error::ABRT.bits();
                    }
                }
            }

            // Write sectors
            0x30 | 0x31 | 0x38 | 0x3C => {
                // check for LBA mode
                if (registers.drive_head & 0x40) == 0 {
                    self.status |= Status::ERR.bits();
                    self.error |= Error::AMNF.bits() | Error::IDNF.bits() | Error::ABRT.bits();
                    return;
                }

                self.sector_offset = 0;
                self.status |= Status::DRQ.bits();
                self.state = CommandState::WriteSectors;
            }

            // Invalid command
            _ => {
                self.status |= Status::ERR.bits();
                self.error |= Error::AMNF.bits() | Error::ABRT.bits()
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
    pub fn single(mmap: M) -> Self {
        Self::new(mmap, None)
    }

    pub fn dual(mmap0: M, mmap1: M) -> Self {
        Self::new(mmap0, Some(mmap1))
    }

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

    fn interrupt_pending(&self) -> bool {
        if self.card0.interrupt_enabled {
            return self.card0.interrupt_pending;
        }
        match self.card1.as_ref() {
            Some(card1) if card1.interrupt_enabled => card1.interrupt_pending,
            _ => false,
        }
    }

    fn ack_interrupt(&mut self) -> u8 {
        // AFAICT, interrupts are low-level and won't populate the data bus.
        if self.card0.interrupt_enabled && self.card0.interrupt {
            self.card0.interrupt = false;
            self.card0.interrupt_pending = true;
            return self.card0.interrupt_vector;
        }
        match self.card1.as_mut() {
            Some(card1) if card1.interrupt_enabled && card1.interrupt => {
                card1.interrupt = false;
                card1.interrupt_pending = true;
                card1.interrupt_vector
            }
            _ => 0,
        }
    }

    fn ret_interrupt(&mut self) {
        if self.card0.interrupt_enabled && self.card0.interrupt_pending {
            self.card0.interrupt_pending = false;
        }
        match self.card1.as_mut() {
            Some(card1) if card1.interrupt_enabled && card1.interrupt_pending => {
                card1.interrupt_pending = false;
            }
            _ => {}
        }
    }
}
