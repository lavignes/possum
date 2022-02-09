//! MOS 8563 VDC Emulation

use std::mem;

use crate::{Device, DeviceBus};

const VRAM_SIZE: usize = 0x4000;
const VRAM_ADDR_MAX: usize = 0x3FFF;

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

bitflags::bitflags! {
    // control bits in the horizontal scroll register
    struct HScrollControl: u8 {
        // double-wide all pixels
        const DBL = 0x10;

        // semigraphic mode
        const SEMI = 0x20;

        // enable attributes
        const ATR = 0x40;

        // text mode (1 sets to graphics mode)
        const TEXT = 0x80;
    }
}

bitflags::bitflags! {
    // control bits in the vertical scroll register
    struct VScrollControl: u8 {
        // double blink rate
        const CBRATE = 0x20;

        // reverse graphics
        const RVS = 0x40;

        // sets whether the next block operation is a
        // copy or fill
        const COPY = 0x80;
    }
}

fn color_lookup(bits: u8) -> u32 {
    match bits & 0x0F {
        // black
        0b0000 => 0xFF000000,
        0b0001 => 0xFF555555,

        // blue
        0b0010 => 0xFF0000AA,
        0b0011 => 0xFF5555FF,

        // green
        0b0100 => 0xFF00AA00,
        0b0101 => 0xFF55FF55,

        // cyan
        0b0110 => 0xFF00AAAA,
        0b0111 => 0xFF55FFFF,

        // red
        0b1000 => 0xFFAA0000,
        0b1001 => 0xFFFF5555,

        // purple
        0b1010 => 0xFFAA00AA,
        0b1011 => 0xFFFF55FF,

        // yellow
        0b1100 => 0xFFAAAA00,
        0b1101 => 0xFFFFFF55,

        // white
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
    fn resize(&mut self, width: usize, height: usize) -> bool {
        let changed = (width != self.width) || (height != self.height);
        self.pixels.resize(width * height, 0);
        self.width = width;
        self.height = height;
        changed
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
    framebuffer_ready: bool,
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
    hsync_start: usize,
    vsync_start: usize,
    hsync_width: usize,
    vsync_height: usize,
    cell_width: usize,
    cell_height: usize,
    cell_visible_width: usize,
    cell_visible_height: usize,
    cursor_start_line: usize,
    cursor_end_line: usize,
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
    disp_start_addr: u16,
    cursor_pos: u16,
    update_addr: u16,
    attr_addr: u16,
    char_total_disp_horiz: u8,
    char_disp_vert: u8,
    vert_scroll_ctrl: u8,
    horiz_scroll_ctrl: u8,
    fg_bg_color: u8,
    addr_inc: u8,
    char_base_addr: u16,
    underline_ctrl: u8,
    word_count: u8,
    block_start_addr: u16,
    disp_enable_begin: u8,
    disp_enable_end: u8,
}

impl Vdc {
    pub fn new() -> Self {
        Self {
            framebuffer_ready: false,
            framebuffer: Framebuffer::default(),
            vram: vec![0; VRAM_SIZE],

            parameters_dirty: true,
            signal_width: 0,
            signal_height: 0,
            top_border_height: 0,
            left_border_width: 0,
            visible_width: 0,
            visible_height: 0,
            right_border_width: 0,
            bottom_border_height: 0,
            hsync_start: 0,
            vsync_start: 0,
            hsync_width: 0,
            vsync_height: 0,
            cell_width: 0,
            cell_height: 0,
            cell_visible_width: 0,
            cell_visible_height: 0,
            cursor_start_line: 0,
            cursor_end_line: 0,
            raster_x: 0,
            raster_y: 0,

            status: 0,
            register_select: 0,

            // TODO: remove these hard-codes (stolen from C= 128 docs)
            // horiz_total: 126,
            // horiz_displayed: 80,
            // horiz_sync: 102,
            // sync_widths: 0b0100_1001,
            // vert_total: 32,
            // vert_adjust: 0,
            // vert_displayed: 25,
            // vert_sync: 29,
            // interlace_mode: 0,
            // char_total_vert: 7,
            // cursor_mode_start_scan: 0,
            // cursor_end_scan_line: 7,
            // disp_start: 0x0000,
            // cursor_pos: 0,
            // update_addr: 0,
            // attr_start: 0x0800,
            // char_total_disp_horiz: 0b0111_1000,
            // char_disp_vert: 0b0000_1000,
            // vert_scroll_ctrl: 0,
            // horiz_scroll_ctrl: 0,
            // fg_bg_color: 0,
            // addr_inc: 0,
            // char_start: 0x2000,
            // underline_ctrl: 0,
            // word_count: 0,
            // block_start: 0,

            // Commented names are the short-hand from the C=128 docs:
            // HT[0:7]
            horiz_total: 0,
            // HD[0:7]
            horiz_displayed: 0,
            // HP[0:7]
            horiz_sync: 0,
            // HW[0:4] VW[5:7]
            sync_widths: 0,
            // VT[0:7]
            vert_total: 0,
            // VA[0:4]
            vert_adjust: 0,
            // VD[0:7]
            vert_displayed: 0,
            // VP[0:7]
            vert_sync: 0,
            // IM[0:1]
            interlace_mode: 0,
            // CTV[0:4]
            char_total_vert: 0,
            // CS[0:4] CM[5:6]
            cursor_mode_start_scan: 0,
            // CE[0:4]
            cursor_end_scan_line: 0,
            // DS[0:7]
            disp_start_addr: 0,
            // CP[0:15]
            cursor_pos: 0,
            // UA[0:15]
            update_addr: 0,
            // AA[0:15]
            attr_addr: 0,
            // CDH[0:3] CTH[4:7]
            char_total_disp_horiz: 0,
            // CDV[0:4]
            char_disp_vert: 0,
            // VSS[0:4] CBRATE[5] RVS[6] COPY[7]
            vert_scroll_ctrl: 0,
            // HSS[0:3] DBL[4] SEMI[5] ATR[6] TEXT[7]
            horiz_scroll_ctrl: 0,
            // BG[0:4] FG[5:7]
            fg_bg_color: 0,
            // AI[0:7]
            addr_inc: 0,
            // RAM[4] CB[5:7]
            char_base_addr: 0,
            // UL[0:4]
            underline_ctrl: 0,
            // WC[0:7]
            word_count: 0,
            // BS[0:15]
            block_start_addr: 0,

            // The screen must turn off for some portion of the scan-line in RGBi.
            // These control when that period starts and ends (measured in char columns)
            // TODO: implement?

            // DEB[0:15]
            disp_enable_begin: 0,
            // DEE[0:15]
            disp_enable_end: 0,
        }
    }

    pub fn framebuffer(&self) -> &Framebuffer {
        &self.framebuffer
    }

    pub fn framebuffer_ready(&self) -> bool {
        self.framebuffer_ready
    }

    fn recompute_parameters(&mut self) {
        self.parameters_dirty = false;

        let cells_x = self.horiz_total.wrapping_add(1) as usize;
        let cells_y = self.vert_total.wrapping_add(1) as usize;
        self.cell_width = ((self.char_total_disp_horiz >> 4) as usize).wrapping_add(1) & 0x0F;
        self.cell_height = (self.char_total_vert.wrapping_add(1) as usize) & 0x1F;
        self.cell_visible_width = (self.char_total_disp_horiz & 0x0F) as usize;
        self.cell_visible_height = self.char_disp_vert as usize;
        self.signal_width = cells_x * self.cell_width;
        self.signal_height = cells_y * self.cell_height;

        self.visible_width = (self.horiz_displayed as usize) * self.cell_width;
        self.visible_height = (self.vert_displayed as usize) * self.cell_height;

        self.hsync_width =
            (((self.sync_widths & 0x0F) as usize).wrapping_sub(1) & 0x0F) * self.cell_width;
        self.vsync_height = (self.sync_widths >> 4) as usize;

        self.hsync_start = self.signal_width.wrapping_sub(self.hsync_width) & 0x3FF;
        self.vsync_start = self.signal_height.wrapping_sub(self.vsync_height) & 0x3FF;

        let vert_sync_pos = ((self.vert_sync.wrapping_sub(1) as usize) & 0xFF) * self.cell_height;
        self.top_border_height = self
            .signal_height
            .wrapping_sub(vert_sync_pos)
            .wrapping_sub(self.vsync_height)
            & 0xFF;
        self.bottom_border_height = self
            .signal_height
            .wrapping_sub(self.top_border_height)
            .wrapping_sub(self.visible_height)
            .wrapping_sub(self.vsync_height)
            & 0xFF;

        let horiz_sync_pos = (self.horiz_sync as usize) * self.cell_width;
        self.left_border_width = self
            .signal_width
            .wrapping_sub(horiz_sync_pos)
            .wrapping_sub(self.hsync_width)
            & 0xFF;
        self.right_border_width = self
            .signal_width
            .wrapping_sub(self.left_border_width)
            .wrapping_sub(self.visible_width)
            .wrapping_sub(self.hsync_width)
            & 0xFF;

        self.cursor_start_line = (self.cursor_mode_start_scan & 0x0F) as usize;
        self.cursor_end_line = (self.cursor_end_scan_line.wrapping_sub(1) as usize) & 0x1F;

        self.framebuffer_ready = false;

        let changed_size = self.framebuffer.resize(
            self.signal_width.wrapping_sub(self.hsync_width) & 0x3FF,
            self.signal_height.wrapping_sub(self.vsync_height) & 0x3FF,
        );
        // lets be a little paranoid ;-)
        if changed_size {
            self.framebuffer_ready = false;
        }
    }
}

impl Device for Vdc {
    fn tick(&mut self, _: &mut dyn DeviceBus) {
        self.status &= !Status::VBLANK.bits();

        if self.parameters_dirty {
            self.recompute_parameters();
        }

        // in hblank
        if self.raster_x == self.hsync_start {
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

                // Adjust the line to the vertical scroll
                let scroll_y = (self.vert_scroll_ctrl & 0x0F) as usize;
                let scrolled_y = self.raster_y - self.top_border_height + scroll_y;

                // Lets find what cell row we want to render
                let cell_y = scrolled_y / self.cell_height;
                let cell_yoffset = scrolled_y % self.cell_height;
                let cell_stride = self.horiz_displayed as usize;

                // and make sure we aren't clipping the bottom of the cell
                if cell_yoffset <= self.cell_visible_height {
                    // find where it starts in the display memory
                    let row_start_addr = (self.disp_start_addr as usize) + (cell_y * cell_stride);

                    // now, start drawing...
                    let mut x = self.left_border_width;
                    for (cell_x, addr) in (row_start_addr..(row_start_addr + cell_stride))
                        .into_iter()
                        .enumerate()
                    {
                        let cell_index = self.vram[addr & VRAM_ADDR_MAX];

                        // If attributes are enabled, locate and apply them
                        let (mut fg_color, mut bg_color, char_set_offset) =
                            if (self.horiz_scroll_ctrl & HScrollControl::ATR.bits()) != 0 {
                                // get the attrs for this cell
                                let attr_index = (self.attr_addr as usize) + (cell_index as usize);
                                let attr = self.vram[attr_index & VRAM_ADDR_MAX];
                                let mut fg_color = color_lookup(attr);
                                let mut bg_color = color_lookup(self.fg_bg_color);

                                // there are expected to be 2 sets of 256 characters when attributes
                                // are enabled. The alternate set follows the first in memory.
                                let char_set_offset =
                                    if attr & Attribute::ALTERNATE_CHARACTER.bits() != 0 {
                                        0
                                    } else {
                                        256
                                    };

                                // reverse it
                                if (attr & Attribute::REVERSE.bits()) != 0 {
                                    mem::swap(&mut fg_color, &mut bg_color);
                                }

                                // underline it
                                if (attr & Attribute::UNDERLINE.bits() != 0)
                                    && cell_yoffset == (self.underline_ctrl & 0x0F) as usize
                                {
                                    mem::swap(&mut fg_color, &mut bg_color);
                                }

                                (fg_color, bg_color, char_set_offset)
                            } else {
                                (
                                    color_lookup(self.fg_bg_color >> 4),
                                    color_lookup(self.fg_bg_color),
                                    0,
                                )
                            };

                        // reverse everything if the control bit is set
                        if (self.vert_scroll_ctrl & VScrollControl::RVS.bits()) != 0 {
                            mem::swap(&mut fg_color, &mut bg_color);
                        }

                        // Reverse the video *AGAIN* if this is the cursor
                        let is_cursor =
                            (cell_x + (cell_y * cell_stride)) == (self.cursor_pos as usize);
                        if is_cursor {
                            if cell_yoffset >= self.cursor_start_line
                                && cell_yoffset <= self.cursor_end_line
                            {
                                mem::swap(&mut fg_color, &mut bg_color);
                            }
                        }

                        // note: there are *ONLY* up to 16 rows of 8 bytes for each char.
                        // Regardless of the character width, only 8 bytes can be used per cell.
                        // And for an 8x8 character, the bottom half is effectively wasted space :-(
                        // TODO: technically there is a double-height mode with 32 bytes
                        const PIX_STRIDE: usize = 16;

                        // get the 8 pixels for the char
                        let pix_index = (self.char_base_addr as usize)
                            + ((char_set_offset + (cell_index as usize)) * PIX_STRIDE)
                            + cell_yoffset;
                        let mut pix = self.vram[pix_index & VRAM_ADDR_MAX];

                        // for each visible bit, blit the pixel
                        for _ in 0..self.cell_visible_width {
                            self.framebuffer.pixels[x + (self.raster_y * self.framebuffer.width)] =
                                if (pix & 0x80) != 0 {
                                    fg_color
                                } else {
                                    bg_color
                                };
                            x += 1;
                            pix <<= 1;
                        }

                        // continue right to account for the total width
                        for _ in 0..(self.cell_width - self.cell_visible_width) {
                            // TODO: semigraphics mode
                            self.framebuffer.pixels[x + (self.raster_y * self.framebuffer.width)] =
                                0;
                            x += 1;
                        }
                    }
                }
            } else if self.raster_y < self.vsync_start {
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
            self.framebuffer_ready = self.raster_y == self.vsync_start;
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

                    0x0C => (self.disp_start_addr >> 8) as u8,
                    0x0D => (self.disp_start_addr >> 0) as u8,

                    0x0E => (self.cursor_pos >> 8) as u8,
                    0x0F => (self.cursor_pos >> 0) as u8,

                    0x12 => (self.update_addr >> 8) as u8,
                    0x13 => (self.update_addr >> 0) as u8,

                    0x14 => (self.attr_addr >> 8) as u8,
                    0x15 => (self.attr_addr >> 0) as u8,

                    0x16 => self.char_total_disp_horiz,

                    0x17 => self.char_disp_vert | 0xE0,

                    0x18 => self.vert_scroll_ctrl,

                    0x19 => self.horiz_scroll_ctrl,

                    0x1A => self.fg_bg_color,

                    0x1B => self.addr_inc,

                    0x1C => (self.char_base_addr >> 8) as u8,

                    0x1D => self.underline_ctrl | 0xEF,

                    0x1E => self.word_count,

                    0x1F => {
                        // reads automatically increment the address to update :-)
                        let data = self.vram[(self.update_addr as usize) & VRAM_ADDR_MAX];
                        self.update_addr = (self.update_addr + 1) & (VRAM_ADDR_MAX as u16);
                        data
                    }

                    0x20 => (self.block_start_addr >> 8) as u8,
                    0x21 => (self.block_start_addr >> 0) as u8,

                    0x22 => self.disp_enable_begin,
                    0x23 => self.disp_enable_end,

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

                0x0C => {
                    self.disp_start_addr = (self.disp_start_addr & 0x00FF) | ((data as u16) << 8)
                }
                0x0D => {
                    self.disp_start_addr = (self.disp_start_addr & 0xFF00) | ((data as u16) << 0)
                }

                0x0E => self.cursor_pos = (self.cursor_pos & 0x00FF) | ((data as u16) << 8),
                0x0F => self.cursor_pos = (self.cursor_pos & 0xFF00) | ((data as u16) << 0),

                0x12 => self.update_addr = (self.update_addr & 0x00FF) | ((data as u16) << 8),
                0x13 => self.update_addr = (self.update_addr & 0xFF00) | ((data as u16) << 0),

                0x14 => self.attr_addr = (self.attr_addr & 0x00FF) | ((data as u16) << 8),
                0x15 => self.attr_addr = (self.attr_addr & 0xFF00) | ((data as u16) << 0),

                0x16 => self.char_total_disp_horiz = data,

                0x17 => self.char_disp_vert = data & 0x1F,

                0x18 => self.vert_scroll_ctrl = data,

                0x19 => self.horiz_scroll_ctrl = data,

                0x1A => self.fg_bg_color = data,

                0x1B => self.addr_inc = data,

                0x1C => self.char_base_addr = ((data & 0xE0) as u16) << 8,

                0x1D => self.underline_ctrl = data & 0x1F,

                0x1E => self.word_count = data,

                0x1F => {
                    // writes automatically increment the address to update :-)
                    self.vram[(self.update_addr as usize) & VRAM_ADDR_MAX] = data;
                    self.update_addr = (self.update_addr + 1) & (VRAM_ADDR_MAX as u16);
                }

                0x20 => {
                    self.block_start_addr = (self.block_start_addr & 0x00FF) | ((data as u16) << 8)
                }
                0x21 => {
                    self.block_start_addr = (self.block_start_addr & 0xFF00) | ((data as u16) << 0)
                }

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
