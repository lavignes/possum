//! MOS 8563 VDC Emulation

use std::mem;

use crate::{Device, DeviceBus};

bitflags::bitflags! {
    struct Attribute: u8 {
        const INTENSITY = 0x01;
        const BLUE = 0x02;
        const GREEN = 0x04;
        const RED = 0x08;

        const BLINK = 0x10;
        const UNDERLINE = 0x20;
        const REVERSE = 0x40;
        const ALTERNATE_CHARACTER = 0x80;
    }
}

fn color_lookup(bits: u8) -> u32 {
    match bits & 0x0F {
        0b0000 => 0xFF000000,
        0b0001 => 0xFF555555,
        0b0010 => 0xFF0000AA,
        0b0011 => 0xFF5555FF,
        0b0100 => 0xFF00AA00,
        0b0101 => 0xFF55FF55,
        0b0110 => 0xFF00AAAA,
        0b0111 => 0xFF55FFFF,
        0b1000 => 0xFFAA0000,
        0b1001 => 0xFFFF5555,
        0b1010 => 0xFFAA00AA,
        0b1011 => 0xFFFF55FF,
        0b1100 => 0xFFAA5500,
        0b1101 => 0xFFFFFF55,
        0b1110 => 0xFFAAAAAA,
        0b1111 => 0xFFFFFFFF,
        _ => unreachable!(),
    }
}

bitflags::bitflags! {
    struct Status: u8 {
        // VDC hardware version bits
        const VER2 = 0x01;
        const VER1 = 0x02;
        const VER0 = 0x04;

        const VBLANK = 0x20;

        // lol. light pen
        const LP = 0x40;

        // Register a11y status
        const STATUS = 0x80;
    }
}

#[derive(Debug, Default)]
pub struct Framebuffer {
    pixels: Vec<u32>,
    width: usize,
    height: usize,
}

impl Framebuffer {
    fn resize(&mut self, width: usize, height: usize) {
        self.pixels.resize(width * height, 0);
        self.width = width;
        self.height = height;
    }

    #[inline]
    pub fn data(&self) -> &[u32] {
        &self.pixels
    }

    #[inline]
    pub fn width(&self) -> usize {
        self.width
    }

    #[inline]
    pub fn height(&self) -> usize {
        self.height
    }
}

pub struct Vdc {
    framebuffer_full: bool,
    framebuffer: Framebuffer,
    vram: Vec<u8>,

    // Rendering/CRT state
    parameters_dirty: bool,
    signal_width: usize,
    signal_height: usize,
    top_border_height: usize,
    left_border_width: usize,
    visible_width: usize,
    visible_height: usize,
    right_border_width: usize,
    bottom_border_height: usize,
    hsync_width: usize,
    vsync_height: usize,
    cell_width: usize,
    cell_height: usize,
    cell_visible_width: usize,
    cell_visible_height: usize,
    raster_x: usize,
    raster_y: usize,

    // Registers
    status: u8,
    register_select: u8,

    horiz_total: u8,
    horiz_displayed: u8,
    horiz_sync: u8,
    sync_widths: u8,
    vert_total: u8,
    vert_adjust: u8,
    vert_displayed: u8,
    vert_sync: u8,
    interlace_mode: u8,
    char_total_vert: u8,
    cursor_mode_start_scan: u8,
    cursor_end_scan_line: u8,
    disp_start: u16,
    cursor_pos: u16,
    update_addr: u16,
    attr_start: u16,
    char_total_disp_horiz: u8,
    char_disp_vert: u8,
    vert_scroll_ctrl: u8,
    horiz_scroll_ctrl: u8,
    fg_bg_color: u8,
    addr_inc: u8,
    char_start: u16,
    underline_ctrl: u8,
    word_count: u8,
    block_start: u16,
    disp_enable_end: u8,
    disp_enable_begin: u8,
}

impl Vdc {
    pub fn new() -> Self {
        Self {
            framebuffer_full: false,
            framebuffer: Framebuffer::default(),
            vram: vec![0; 16384],

            parameters_dirty: true,
            signal_width: 0,
            signal_height: 0,
            top_border_height: 0,
            left_border_width: 0,
            visible_width: 0,
            visible_height: 0,
            right_border_width: 0,
            bottom_border_height: 0,
            hsync_width: 0,
            vsync_height: 0,
            cell_width: 0,
            cell_height: 0,
            cell_visible_width: 0,
            cell_visible_height: 0,
            raster_x: 0,
            raster_y: 0,

            status: 0,
            register_select: 0,

            // TODO: remove these hard-codes (stolen from C= 128 docs)
            horiz_total: 126,
            horiz_displayed: 80,
            horiz_sync: 102,
            sync_widths: 0b0100_1001,
            vert_total: 32,
            vert_adjust: 0,
            vert_displayed: 25,
            vert_sync: 29,
            interlace_mode: 0,
            char_total_vert: 7,
            cursor_mode_start_scan: 0,
            cursor_end_scan_line: 7,
            disp_start: 0x0000,
            cursor_pos: 0,
            update_addr: 0,
            attr_start: 0x0800,
            char_total_disp_horiz: 0b0111_1000,
            char_disp_vert: 0b0000_1000,
            vert_scroll_ctrl: 0,
            horiz_scroll_ctrl: 0,
            fg_bg_color: 0,
            addr_inc: 0,
            char_start: 0x2000,
            underline_ctrl: 0,
            word_count: 0,
            block_start: 0,

            // The screen must turn off for some portion of the scan-line in RGBi.
            // These control when that period starts and ends (measured in char columns)
            // TODO: implement
            disp_enable_end: 0,
            disp_enable_begin: 0,
        }
    }

    pub fn framebuffer(&self) -> &Framebuffer {
        &self.framebuffer
    }

    pub fn vblank(&self) -> bool {
        self.framebuffer_full
    }

    fn recompute_parameters(&mut self) {
        self.parameters_dirty = false;

        let cells_x = (self.horiz_total as usize) + 1;
        let cells_y = (self.vert_total as usize) + 1;
        self.cell_width = ((self.char_total_disp_horiz >> 4) as usize) + 1;
        self.cell_height = (self.char_total_vert as usize) + 1;
        self.cell_visible_width = (self.char_total_disp_horiz & 0x0F) as usize;
        self.cell_visible_height = self.char_disp_vert as usize;
        self.signal_width = cells_x * self.cell_width;
        self.signal_height = cells_y * self.cell_height;

        self.visible_width = (self.horiz_displayed as usize) * self.cell_width;
        self.visible_height = (self.vert_displayed as usize) * self.cell_height;

        self.hsync_width = (((self.sync_widths & 0x0F) as usize) - 1) * self.cell_width;
        self.vsync_height = (self.sync_widths >> 4) as usize;

        let vert_sync_pos = (self.vert_sync as usize - 1) * self.cell_height;
        self.top_border_height = self.signal_height - vert_sync_pos - self.vsync_height;
        self.bottom_border_height =
            self.signal_height - self.top_border_height - self.visible_height - self.vsync_height;

        let horiz_sync_pos = self.horiz_sync as usize * self.cell_width;
        self.left_border_width = self.signal_width - horiz_sync_pos - self.hsync_width;
        self.right_border_width =
            self.signal_width - self.left_border_width - self.visible_width - self.hsync_width;

        self.framebuffer_full = false;
        self.framebuffer.resize(
            self.signal_width - self.hsync_width,
            self.signal_height - self.vsync_height,
        );
    }
}

impl Device for Vdc {
    fn tick(&mut self, _: &mut dyn DeviceBus) {
        self.status &= !Status::VBLANK.bits();

        if self.parameters_dirty {
            self.recompute_parameters();

            self.vram[(self.char_start as usize) + 16] = 0b11111110;
            self.vram[(self.char_start as usize) + 17] = 0b10000000;
            self.vram[(self.char_start as usize) + 18] = 0b10000000;
            self.vram[(self.char_start as usize) + 19] = 0b11111110;
            self.vram[(self.char_start as usize) + 20] = 0b10000000;
            self.vram[(self.char_start as usize) + 21] = 0b10000000;
            self.vram[(self.char_start as usize) + 22] = 0b11111110;
            self.vram[(self.char_start as usize) + 23] = 0b00000000;

            self.vram[(self.char_start as usize) + 24] = 0b11111110;
            self.vram[(self.char_start as usize) + 25] = 0b11000110;
            self.vram[(self.char_start as usize) + 26] = 0b11000110;
            self.vram[(self.char_start as usize) + 27] = 0b11000110;
            self.vram[(self.char_start as usize) + 28] = 0b11000110;
            self.vram[(self.char_start as usize) + 29] = 0b11000110;
            self.vram[(self.char_start as usize) + 30] = 0b11111110;
            self.vram[(self.char_start as usize) + 31] = 0b00000000;

            self.vram[(self.disp_start as usize) + 0] = 1;
            self.vram[(self.disp_start as usize) + 1] = 2;

            self.vram[(self.attr_start as usize) + 1] = 0x43;
            self.vram[(self.attr_start as usize) + 2] = 0x02;
        }

        // in hblank
        if self.raster_x == (self.signal_width - self.hsync_width) {
            if self.raster_y < self.top_border_height {
                // top border
                for x in 0..self.framebuffer.width {
                    self.framebuffer.pixels[x + (self.raster_y * self.framebuffer.width)] = 0;
                }
            } else if self.raster_y < (self.top_border_height + self.visible_height) {
                // visible

                // Draw left and right borders first, since we can spill out of them I think
                for x in 0..self.left_border_width {
                    self.framebuffer.pixels[x + (self.raster_y * self.framebuffer.width)] = 0;
                }
                for x in (self.left_border_width + self.visible_width)
                    ..(self.signal_width - self.hsync_width)
                {
                    self.framebuffer.pixels[x + (self.raster_y * self.framebuffer.width)] = 0;
                }

                // lets find what row we are in
                let cell_y = (self.raster_y - self.top_border_height) / self.cell_height;
                let cell_yoffset = (self.raster_y - self.top_border_height) % self.cell_height;
                let cell_stride = self.horiz_displayed as usize;

                // and where it starts in the display memory
                let row_start_addr = (self.disp_start as usize) + (cell_y * cell_stride);

                // now, start drawing...
                let mut x = self.left_border_width;
                for (cell_x, ch) in self.vram[row_start_addr..(row_start_addr + cell_stride)]
                    .iter()
                    .enumerate()
                {
                    // get the attrs for this cell
                    let attr = self.vram[(self.attr_start as usize) + (*ch as usize)];
                    let mut fg_color = color_lookup(attr);
                    let mut bg_color = color_lookup(self.fg_bg_color);

                    if (attr & Attribute::REVERSE.bits()) != 0 {
                        mem::swap(&mut fg_color, &mut bg_color);
                    }

                    // Reverse the video if this is the cursor
                    let is_cursor = (cell_x + (cell_y * cell_stride)) == (self.cursor_pos as usize);
                    if is_cursor {
                        let cursor_start_line = (self.cursor_mode_start_scan & 0x0F) as usize;
                        let cursor_end_line = (self.cursor_end_scan_line as usize) - 1;
                        if cell_yoffset >= cursor_start_line && cell_yoffset <= cursor_end_line {
                            mem::swap(&mut fg_color, &mut bg_color);
                        }
                    }

                    // note: there are 16 rows of 8 bytes for each char
                    // regardless of the character width, only 8 bytes can be used.
                    // And for an 8x8 character, the bottom half is effectively wasted space :-(
                    const PIX_STRIDE: usize = 16;

                    // get the 8 pixels for
                    let mut pix = self.vram
                        [(self.char_start as usize) + ((*ch as usize) * PIX_STRIDE) + cell_yoffset];

                    // for each bit, blit the pixel
                    for _ in 0..self.cell_width {
                        self.framebuffer.pixels[x + (self.raster_y * self.framebuffer.width)] =
                            if (pix & 0x80) != 0 {
                                fg_color
                            } else {
                                bg_color
                            };
                        x += 1;
                        pix <<= 1;
                    }
                }
            } else if self.raster_y < (self.signal_height - self.vsync_height) {
                // bottom border
                for x in 0..self.framebuffer.width {
                    self.framebuffer.pixels[x + (self.raster_y * self.framebuffer.width)] = 0;
                }
            } else {
                // in vblank
                self.status |= Status::VBLANK.bits();
            }
        }

        self.raster_x += 1;
        if self.raster_x == self.signal_width {
            self.raster_x = 0;
            self.raster_y += 1;
            // check if we're ready to present the framebuffer to the outside world
            self.framebuffer_full = self.raster_y == (self.signal_height - self.vsync_height);
            if self.raster_y == self.signal_height {
                self.raster_y = 0;
            }
        }
    }

    fn read(&mut self, port: u16) -> u8 {
        match port & 0x01 {
            // read status
            0 => self.status,

            // read data
            1 => {
                match self.register_select {
                    0x00 => self.horiz_total,

                    0x01 => self.horiz_displayed,

                    0x02 => self.horiz_sync,

                    0x03 => self.sync_widths,

                    0x04 => self.vert_total,

                    0x05 => self.vert_adjust | 0xE0,

                    0x06 => self.vert_displayed,

                    0x07 => self.vert_sync,

                    0x08 => self.interlace_mode | 0xFC,

                    0x09 => self.char_total_vert | 0xE0,

                    0x0A => self.cursor_mode_start_scan | 0x80,

                    0x0B => self.cursor_end_scan_line | 0xE0,

                    0x0C => (self.disp_start >> 8) as u8,
                    0x0D => (self.disp_start >> 0) as u8,

                    0x0E => (self.cursor_pos >> 8) as u8,
                    0x0F => (self.cursor_pos >> 0) as u8,

                    0x12 => (self.update_addr >> 8) as u8,
                    0x13 => (self.update_addr >> 0) as u8,

                    0x14 => (self.attr_start >> 8) as u8,
                    0x15 => (self.attr_start >> 0) as u8,

                    0x16 => self.char_total_disp_horiz,

                    0x17 => self.char_disp_vert | 0xE0,

                    0x18 => self.vert_scroll_ctrl,

                    0x19 => self.horiz_scroll_ctrl,

                    0x1A => self.fg_bg_color,

                    0x1B => self.addr_inc,

                    0x1C => (self.char_start >> 8) as u8,

                    0x1D => self.underline_ctrl | 0xEF,

                    0x1E => self.word_count,

                    0x1F => {
                        // reads automatically increment the address to update :-)
                        let data = self.vram[(self.update_addr & 0x3FFF) as usize];
                        self.update_addr = (self.update_addr + 1) & 0x3FFF;
                        data
                    }

                    0x20 => (self.block_start >> 8) as u8,
                    0x21 => (self.block_start >> 0) as u8,

                    0x22 => self.disp_enable_end,
                    0x23 => self.disp_enable_begin,

                    // unused bits seem to read high?
                    _ => 0xFF,
                }
            }

            _ => unreachable!(),
        }
    }

    fn write(&mut self, port: u16, data: u8) {
        // TODO: Don't need to do this on every write
        self.parameters_dirty = true;

        match port & 0x01 {
            // select register
            0 => self.register_select = data & 0x1F,

            1 => match self.register_select {
                0x00 => self.horiz_total = data,

                0x01 => self.horiz_displayed = data,

                0x02 => self.horiz_sync = data,

                0x03 => self.sync_widths = data,

                0x04 => self.vert_total = data,

                0x05 => self.vert_adjust = data & 0x1F,

                0x06 => self.vert_displayed = data,

                0x07 => self.vert_sync = data,

                0x08 => self.interlace_mode = data & 0x03,

                0x09 => self.char_total_vert = data & 0x1F,

                0x0A => self.cursor_mode_start_scan = data & 0x7F,

                0x0B => self.cursor_end_scan_line = data & 0x1F,

                0x0C => self.disp_start = (self.disp_start & 0x00FF) | ((data as u16) << 8),
                0x0D => self.disp_start = (self.disp_start & 0xFF00) | ((data as u16) << 0),

                0x0E => self.cursor_pos = (self.cursor_pos & 0x00FF) | ((data as u16) << 8),
                0x0F => self.cursor_pos = (self.cursor_pos & 0xFF00) | ((data as u16) << 0),

                0x12 => self.update_addr = (self.update_addr & 0x00FF) | ((data as u16) << 8),
                0x13 => self.update_addr = (self.update_addr & 0xFF00) | ((data as u16) << 0),

                0x14 => self.attr_start = (self.attr_start & 0x00FF) | ((data as u16) << 8),
                0x15 => self.attr_start = (self.attr_start & 0xFF00) | ((data as u16) << 0),

                0x16 => self.char_total_disp_horiz = data,

                0x17 => self.char_disp_vert = data & 0x1F,

                0x18 => self.vert_scroll_ctrl = data,

                0x19 => self.horiz_scroll_ctrl = data,

                0x1A => self.fg_bg_color = data,

                0x1B => self.addr_inc = data,

                0x1C => self.char_start = ((data & 0xE0) as u16) << 8,

                0x1D => self.underline_ctrl = data & 0x1F,

                0x1E => self.word_count = data,

                0x1F => {
                    // writes automatically increment the address to update :-)
                    self.vram[(self.update_addr & 0x3FFF) as usize] = data;
                    self.update_addr = (self.update_addr + 1) & 0x3FFF;
                }

                0x20 => self.block_start = (self.block_start & 0x00FF) | ((data as u16) << 8),
                0x21 => self.block_start = (self.block_start & 0xFF00) | ((data as u16) << 0),

                0x22 => self.disp_enable_end = data,
                0x23 => self.disp_enable_begin = data,

                _ => {}
            },

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
