//! MOS 8563 VDC Emulation

use crate::{Device, DeviceBus};

bitflags::bitflags! {
    struct Status: u8 {
        // VDC hardware version bits
        const VER2 = 0x01;
        const VER1 = 0x02;
        const VER0 = 0x04;

        const VBLANK = 0x20;

        // Light pen position updated
        const LP = 0x40;

        // Register a11y status
        const STATUS = 0x80;
    }
}

pub struct Vdc {
    ram: Vec<u8>,

    status: u8,
    register_select: u8,

    horiz_total: u8,
    horiz_displayed: u8,
    horiz_sync: u8,
    sync_width: u8,
    vert_total: u8,
    vert_adjust: u8,
    vert_displayed: u8,
    vert_sync: u8,
    interlace_mode: u8,
    char_total_vertical: u8,
    cursor_mode_start_scan: u8,
    cursor_end_scan_line: u8,
    display_start: u16,
    cursor_pos: u16,
    update_addr: u16,
    attr_addr: u16,
    char_total_display_horiz: u8,
    char_display_vert: u8,
    vert_scroll: u8,
    horiz_scroll: u8,
    fg_bg_color: u8,
    addr_inc: u8,
    char_base: u8,
    word_count: u8,
    block_start: u16,
    disp_enable: u16,
}

impl Vdc {
    pub fn new() -> Self {
        Self {
            ram: vec![0; 16384],

            status: 0,
            register_select: 0,

            horiz_total: 0,
            horiz_displayed: 0,
            horiz_sync: 0,
            sync_width: 0,
            vert_total: 0,
            vert_adjust: 0,
            vert_displayed: 0,
            vert_sync: 0,
            interlace_mode: 0,
            char_total_vertical: 0,
            cursor_mode_start_scan: 0,
            cursor_end_scan_line: 0,
            display_start: 0,
            cursor_pos: 0,
            update_addr: 0,
            attr_addr: 0,
            char_total_display_horiz: 0,
            char_display_vert: 0,
            vert_scroll: 0,
            horiz_scroll: 0,
            fg_bg_color: 0,
            addr_inc: 0,
            char_base: 0,
            word_count: 0,
            block_start: 0,
            disp_enable: 0,
        }
    }
}

impl Device for Vdc {
    fn tick(&mut self, _: &mut dyn DeviceBus) {
        // todo!()
    }

    fn read(&mut self, port: u16) -> u8 {
        match port & 0x01 {
            // read status
            0 => self.status,

            // read data
            1 => {
                todo!()
            }

            _ => unreachable!(),
        }
    }

    fn write(&mut self, port: u16, data: u8) {
        match port & 0x01 {
            // select register
            0 => self.register_select = data,

            1 => {
                todo!()
            }

            _ => unreachable!(),
        }
    }

    fn interrupting(&self) -> bool {
        false
    }

    fn interrupt_vector(&self) -> u8 {
        0
    }

    fn ack_interrupt(&mut self) {}
}
