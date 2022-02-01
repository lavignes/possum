//! z80 CPU emulation

#[cfg(test)]
mod tests;

use std::mem;

use crate::bus::Bus;

#[derive(Copy, Clone, Debug)]
enum WideRegister {
    PC,
    SP,
    AF,
    BC,
    DE,
    HL,
    IX,
    IY,
    AFPrime,
    BCPrime,
    DEPrime,
    HLPrime,
}

#[derive(Copy, Clone, Debug)]
enum Register {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
    IXH,
    IXL,
    IYH,
    IYL,
    I,
    R,
}

#[derive(Copy, Clone, Debug)]
enum Flag {
    C = 0x01,
    N = 0x02,
    PV = 0x04,
    X = 0x08,
    H = 0x10,
    Y = 0x20,
    Z = 0x40,
    S = 0x80,
}

#[derive(Copy, Clone, Debug)]
enum Condition {
    NZ,
    Z,
    NC,
    C,
    PO,
    PE,
    P,
    M,
}

#[derive(Copy, Clone, Debug)]
enum InterruptMode {
    Zero,
    One,
    Two,
}

impl Default for InterruptMode {
    fn default() -> Self {
        Self::Zero
    }
}

#[derive(Default)]
pub struct Cpu {
    pc: u16,
    sp: u16,
    af: u16,
    bc: u16,
    de: u16,
    hl: u16,
    ix: u16,
    iy: u16,
    wz: u16,
    ir: u16,
    af_prime: u16,
    bc_prime: u16,
    de_prime: u16,
    hl_prime: u16,
    wz_prime: u16,

    interrupt_mode: InterruptMode,
    halted: bool,
    interrupts_enabled: bool,
    iff1: bool,
    iff2: bool,
    irq: bool,
    reti: bool,
}

#[inline]
const fn u16_parts(value: u16) -> [u8; 2] {
    unsafe { mem::transmute(value) }
}

#[inline]
fn u16_parts_mut(value: &mut u16) -> &mut [u8; 2] {
    unsafe { mem::transmute(value) }
}

impl Cpu {
    #[inline]
    fn flag(&self, flag: Flag) -> bool {
        (self.af & (flag as u16)) != 0
    }

    #[inline]
    fn set_flag(&mut self, flag: Flag, value: bool) {
        if value {
            self.af |= flag as u16;
        } else {
            self.af &= !(flag as u16);
        }
    }

    #[inline]
    fn condition(&self, condition: Condition) -> bool {
        match condition {
            Condition::NZ => !self.flag(Flag::Z),
            Condition::Z => self.flag(Flag::Z),
            Condition::NC => !self.flag(Flag::C),
            Condition::C => self.flag(Flag::C),
            Condition::PO => self.flag(Flag::PV),
            Condition::PE => !self.flag(Flag::PV),
            Condition::P => self.flag(Flag::S),
            Condition::M => !self.flag(Flag::S),
        }
    }

    #[inline]
    fn register(&self, reg: Register) -> u8 {
        match reg {
            Register::A => u16_parts(self.af)[1],
            Register::B => u16_parts(self.bc)[1],
            Register::C => u16_parts(self.bc)[0],
            Register::D => u16_parts(self.de)[1],
            Register::E => u16_parts(self.de)[0],
            Register::H => u16_parts(self.hl)[1],
            Register::L => u16_parts(self.hl)[0],
            Register::IXH => u16_parts(self.ix)[1],
            Register::IXL => u16_parts(self.ix)[0],
            Register::IYH => u16_parts(self.iy)[1],
            Register::IYL => u16_parts(self.iy)[0],
            Register::I => u16_parts(self.ir)[1],
            Register::R => u16_parts(self.ir)[0],
        }
    }

    #[inline]
    fn set_register(&mut self, reg: Register, data: u8) {
        match reg {
            Register::A => u16_parts_mut(&mut self.af)[1] = data,
            Register::B => u16_parts_mut(&mut self.bc)[1] = data,
            Register::C => u16_parts_mut(&mut self.bc)[0] = data,
            Register::D => u16_parts_mut(&mut self.de)[1] = data,
            Register::E => u16_parts_mut(&mut self.de)[0] = data,
            Register::H => u16_parts_mut(&mut self.hl)[1] = data,
            Register::L => u16_parts_mut(&mut self.hl)[0] = data,
            Register::IXH => u16_parts_mut(&mut self.ix)[1] = data,
            Register::IXL => u16_parts_mut(&mut self.ix)[0] = data,
            Register::IYH => u16_parts_mut(&mut self.iy)[1] = data,
            Register::IYL => u16_parts_mut(&mut self.iy)[0] = data,
            Register::I => u16_parts_mut(&mut self.ir)[1] = data,
            Register::R => u16_parts_mut(&mut self.ir)[0] = data,
        }
    }

    #[inline]
    fn wide_register(&self, reg: WideRegister) -> u16 {
        match reg {
            WideRegister::PC => self.pc,
            WideRegister::SP => self.sp,
            WideRegister::AF => self.af,
            WideRegister::BC => self.bc,
            WideRegister::DE => self.de,
            WideRegister::HL => self.hl,
            WideRegister::IX => self.ix,
            WideRegister::IY => self.iy,
            WideRegister::AFPrime => self.af_prime,
            WideRegister::BCPrime => self.bc_prime,
            WideRegister::DEPrime => self.de_prime,
            WideRegister::HLPrime => self.hl_prime,
        }
    }

    #[inline]
    fn set_wide_register(&mut self, reg: WideRegister, data: u16) {
        match reg {
            WideRegister::PC => self.pc = data,
            WideRegister::SP => self.sp = data,
            WideRegister::AF => self.af = data,
            WideRegister::BC => self.bc = data,
            WideRegister::DE => self.de = data,
            WideRegister::HL => self.hl = data,
            WideRegister::IX => self.ix = data,
            WideRegister::IY => self.iy = data,
            WideRegister::AFPrime => self.af_prime = data,
            WideRegister::BCPrime => self.bc_prime = data,
            WideRegister::DEPrime => self.de_prime = data,
            WideRegister::HLPrime => self.hl_prime = data,
        };
    }

    #[inline]
    fn immediate(&mut self, bus: &mut impl Bus) -> u8 {
        let opcode = bus.read(self.pc);
        self.pc = self.pc.carrying_add(1, false).0;
        opcode
    }

    #[inline]
    fn wide_immediate(&mut self, bus: &mut impl Bus) -> u16 {
        (self.immediate(bus) as u16) | ((self.immediate(bus) as u16) << 8)
    }

    #[inline]
    fn fetch(&mut self, bus: &mut impl Bus) -> u8 {
        // increment 7-bits of memory refresh register
        // basically, carries from bit 6 -> 7 are ignored
        let r = self.register(Register::R);
        self.set_register(Register::R, (r & 0x80) | (r.overflowing_add(1).0 & 0x7F));
        self.immediate(bus)
    }

    #[inline]
    fn nop(&self) -> usize {
        4
    }

    #[inline]
    fn read_wide_immediate(&mut self, reg: WideRegister, bus: &mut impl Bus) -> usize {
        let data = self.wide_immediate(bus);
        self.set_wide_register(reg, data);
        10
    }

    #[inline]
    fn read_immediate(&mut self, reg: Register, bus: &mut impl Bus) -> usize {
        let data = self.immediate(bus);
        self.set_register(reg, data);
        7
    }

    #[inline]
    fn write_indirect_wz(
        &mut self,
        addr: WideRegister,
        reg: Register,
        bus: &mut impl Bus,
    ) -> usize {
        let addr = self.wide_register(addr);
        bus.write(addr, self.register(reg));
        self.wz = (self.register(Register::A) as u16) | (addr.carrying_add(1, false).0 & 0xFF);
        7
    }

    #[inline]
    fn inc_wide(&mut self, reg: WideRegister) -> usize {
        let data = self.wide_register(reg).carrying_add(1, false).0;
        self.set_wide_register(reg, data);
        6
    }

    #[inline]
    fn inc_wz(&mut self, reg: Register) -> usize {
        let data = self.register(reg);
        let result = data.carrying_add(1, false).0;
        self.wz = result as u16;
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, result == 0x80);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, ((result ^ data) & (Flag::H as u8)) != 0);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
        self.set_register(reg, result);
        4
    }

    #[inline]
    fn dec_wz(&mut self, reg: Register) -> usize {
        let data = self.register(reg);
        let result = data.borrowing_sub(1, false).0;
        self.wz = result as u16;
        self.set_flag(Flag::N, true);
        self.set_flag(Flag::PV, result == 0x7F);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, ((result ^ data) & (Flag::H as u8)) != 0);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
        self.set_register(reg, result);
        4
    }

    #[inline]
    fn rlca(&mut self) -> usize {
        let data = self.register(Register::A);
        let result = (data << 1) | (data >> 7);
        self.set_flag(Flag::C, (data & 0x80) != 0);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_register(Register::A, result);
        4
    }

    #[inline]
    fn exchange(&mut self, reg1: WideRegister, reg2: WideRegister) -> usize {
        let tmp = self.wide_register(reg1);
        self.set_wide_register(reg1, self.wide_register(reg2));
        self.set_wide_register(reg2, tmp);
        4
    }

    #[inline]
    fn add_wide_wz(&mut self, dst: WideRegister, src: WideRegister) -> usize {
        let lhs = self.wide_register(dst);
        let rhs = self.wide_register(src);
        self.wz = lhs.carrying_add(1, false).0;
        let (result, carry) = lhs.carrying_add(rhs, false);
        self.set_flag(Flag::C, carry);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::X, ((result >> 8) & (Flag::X as u16)) != 0);
        self.set_flag(
            Flag::H,
            (((lhs ^ result ^ rhs) >> 8) & (Flag::H as u16)) != 0,
        );
        self.set_flag(Flag::Y, ((result >> 8) & (Flag::Y as u16)) != 0);
        self.set_wide_register(dst, result);
        11
    }

    #[inline]
    fn read_indirect_wz(&mut self, reg: Register, addr: WideRegister, bus: &mut impl Bus) -> usize {
        let addr = self.wide_register(addr);
        self.set_register(reg, bus.read(addr));
        self.wz = addr.carrying_add(1, false).0;
        7
    }

    #[inline]
    fn dec_wide(&mut self, reg: WideRegister) -> usize {
        let data = self.wide_register(reg).borrowing_sub(1, false).0;
        self.set_wide_register(reg, data);
        6
    }

    #[inline]
    fn rrca(&mut self) -> usize {
        let data = self.register(Register::A);
        let result = (data >> 1) | (data << 7);
        self.set_flag(Flag::C, (data & 0x01) != 0);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_register(Register::A, result);
        4
    }

    #[inline]
    fn djnz_wz(&mut self, bus: &mut impl Bus) -> usize {
        let b = self.register(Register::B).borrowing_sub(1, false).0;
        self.set_register(Register::B, b);
        if b > 0 {
            let offset = bus.read(self.pc) as i8 as i16;
            let wz = self.pc.carrying_add(offset as u16, true).0;
            self.wz = wz;
            self.pc = wz;
            13
        } else {
            self.pc = self.pc.carrying_add(1, false).0;
            8
        }
    }

    #[inline]
    fn rla(&mut self) -> usize {
        let data = self.register(Register::A);
        let result = (data << 1) | (if self.flag(Flag::C) { 0x01 } else { 0x00 });
        self.set_flag(Flag::C, (data & 0x80) != 0);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_register(Register::A, result);
        4
    }

    #[inline]
    fn jump_relative_wz(&mut self, bus: &mut impl Bus) -> usize {
        let offset = bus.read(self.pc) as i8 as i16;
        let wz = self.pc.carrying_add(offset as u16, true).0;
        self.wz = wz;
        self.pc = wz;
        12
    }

    #[inline]
    fn rra(&mut self) -> usize {
        let data = self.register(Register::A);
        let result = (data >> 1) | (if self.flag(Flag::C) { 0x80 } else { 0x00 });
        self.set_flag(Flag::C, (data & 0x01) != 0);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_register(Register::A, result);
        4
    }

    #[inline]
    fn conditional_jump_relative_wz(&mut self, condition: Condition, bus: &mut impl Bus) -> usize {
        if self.condition(condition) {
            self.jump_relative_wz(bus)
        } else {
            self.pc = self.pc.carrying_add(1, false).0;
            7
        }
    }

    #[inline]
    fn write_wide_absolute_wz(&mut self, reg: WideRegister, bus: &mut impl Bus) -> usize {
        let addr = self.wide_immediate(bus);
        let parts = u16_parts(self.wide_register(reg));
        bus.write(addr, parts[0]);
        let addr = addr.carrying_add(1, false).0;
        bus.write(addr, parts[1]);
        self.wz = addr;
        16
    }

    #[inline]
    fn daa(&mut self) -> usize {
        let data = self.register(Register::A);
        let mut result = data;
        if self.flag(Flag::N) {
            if ((data & 0x0F) > 0x09) || self.flag(Flag::H) {
                result = result.borrowing_sub(0x06, false).0;
            }
            if (data > 0x99) || self.flag(Flag::C) {
                result = result.borrowing_sub(0x60, false).0;
            }
        } else {
            if ((data & 0x0F) > 0x09) || self.flag(Flag::H) {
                result = result.carrying_add(0x06, false).0;
            }
            if (data > 0x99) || self.flag(Flag::C) {
                result = result.carrying_add(0x60, false).0;
            }
        }
        self.set_flag(Flag::C, self.flag(Flag::C) || (data > 0x99));
        self.set_flag(Flag::PV, (result.count_ones() & 1) == 0);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, ((data ^ result) & (Flag::H as u8)) != 0);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
        self.set_register(Register::A, result);
        4
    }

    #[inline]
    fn read_wide_absolute_wz(&mut self, reg: WideRegister, bus: &mut impl Bus) -> usize {
        let addr = self.wide_immediate(bus);
        let low = bus.read(addr);
        let addr = addr.carrying_add(1, false).0;
        let high = bus.read(addr);
        self.set_wide_register(reg, (low as u16) | ((high as u16) << 8));
        self.wz = addr.carrying_add(1, false).0;
        16
    }

    #[inline]
    fn cpl(&mut self) -> usize {
        let result = self.register(Register::A) ^ 0xFF;
        self.set_flag(Flag::N, true);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, true);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_register(Register::A, result);
        4
    }

    #[inline]
    fn write_absolute_wz(&mut self, reg: Register, bus: &mut impl Bus) -> usize {
        let addr = self.wide_immediate(bus);
        bus.write(addr, self.register(reg));
        self.wz = (self.register(Register::A) as u16) | (addr.carrying_add(1, false).0 & 0xFF);
        13
    }

    #[inline]
    fn inc_hl_indirect(&mut self, bus: &mut impl Bus) -> usize {
        let addr = self.wide_register(WideRegister::HL);
        let data = bus.read(addr);
        let result = data.carrying_add(1, false).0;
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, result == 0x80);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, ((result ^ data) & (Flag::H as u8)) != 0);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
        bus.write(addr, result);
        11
    }

    #[inline]
    fn dec_hl_indirect(&mut self, bus: &mut impl Bus) -> usize {
        let addr = self.wide_register(WideRegister::HL);
        let data = bus.read(addr);
        let result = data.borrowing_sub(1, false).0;
        self.set_flag(Flag::N, true);
        self.set_flag(Flag::PV, result == 0x7F);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, ((result ^ data) & (Flag::H as u8)) != 0);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
        bus.write(addr, result);
        11
    }

    #[inline]
    fn write_immediate_hl_indirect(&mut self, bus: &mut impl Bus) -> usize {
        let addr = self.wide_register(WideRegister::HL);
        let data = self.immediate(bus);
        bus.write(addr, data);
        10
    }

    #[inline]
    fn scf(&mut self) -> usize {
        self.set_flag(Flag::C, true);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::H, false);
        4
    }

    #[inline]
    fn read_absolute_wz(&mut self, reg: Register, bus: &mut impl Bus) -> usize {
        let addr = self.wide_immediate(bus);
        self.set_register(reg, bus.read(addr));
        self.wz = addr.carrying_add(1, false).0;
        13
    }

    #[inline]
    fn ccf(&mut self) -> usize {
        let carry = self.flag(Flag::C);
        self.set_flag(Flag::C, !carry);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::H, carry);
        4
    }

    #[inline]
    fn copy_register(&mut self, dst: Register, src: Register) -> usize {
        self.set_register(dst, self.register(src));
        4
    }

    #[inline]
    fn copy_register_hl_indirect(&mut self, reg: Register, bus: &mut impl Bus) -> usize {
        let data = bus.read(self.hl);
        self.set_register(reg, data);
        7
    }

    #[inline]
    fn copy_hl_indirect_register(&mut self, reg: Register, bus: &mut impl Bus) -> usize {
        let data = self.register(reg);
        bus.write(self.hl, data);
        7
    }

    #[inline]
    fn halt(&mut self) -> usize {
        self.halted = true;
        self.pc = self.pc.borrowing_sub(1, false).0;
        4
    }

    #[inline]
    fn add_carry_base(&mut self, rhs: u8, carry: bool) {
        let lhs = self.register(Register::A);
        let (result, carry) = lhs.carrying_add(rhs, carry);
        self.set_flag(Flag::C, carry);
        self.set_flag(Flag::N, false);
        self.set_flag(
            Flag::PV,
            ((((lhs ^ rhs ^ 0x80) & (rhs ^ result)) >> 5) & (Flag::PV as u8)) != 0,
        );
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, ((lhs ^ rhs ^ result) & (Flag::H as u8)) != 0);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
        self.set_register(Register::A, result);
    }

    #[inline]
    fn add_carry(&mut self, reg: Register, carry: bool) -> usize {
        self.add_carry_base(self.register(reg), carry);
        4
    }

    #[inline]
    fn add_carry_hl_indirect(&mut self, carry: bool, bus: &mut impl Bus) -> usize {
        let rhs = bus.read(self.hl);
        self.add_carry_base(rhs, carry);
        7
    }

    #[inline]
    fn sub_carry_base(&mut self, rhs: u8, carry: bool) {
        let lhs = self.register(Register::A);
        let (result, carry) = lhs.borrowing_sub(rhs, carry);
        self.set_flag(Flag::C, carry);
        self.set_flag(Flag::N, true);
        self.set_flag(
            Flag::PV,
            ((((lhs ^ rhs) & (result ^ lhs)) >> 5) & (Flag::PV as u8)) != 0,
        );
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, ((lhs ^ rhs ^ result) & (Flag::H as u8)) != 0);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
        self.set_register(Register::A, result);
    }

    #[inline]
    fn sub_carry(&mut self, reg: Register, carry: bool) -> usize {
        self.sub_carry_base(self.register(reg), carry);
        4
    }

    #[inline]
    fn sub_carry_hl_indirect(&mut self, carry: bool, bus: &mut impl Bus) -> usize {
        let rhs = bus.read(self.hl);
        self.sub_carry_base(rhs, carry);
        7
    }

    #[inline]
    fn and_base(&mut self, rhs: u8) {
        let data = self.register(Register::A);
        let result = data & rhs;
        self.set_flag(Flag::C, false);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, (result.count_ones() & 1) == 0);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, true);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
        self.set_register(Register::A, result);
    }

    #[inline]
    fn and(&mut self, reg: Register) -> usize {
        self.and_base(self.register(reg));
        4
    }

    #[inline]
    fn and_hl_indirect(&mut self, bus: &mut impl Bus) -> usize {
        let rhs = bus.read(self.hl);
        self.and_base(rhs);
        7
    }

    #[inline]
    fn xor_base(&mut self, rhs: u8) {
        let data = self.register(Register::A);
        let result = data ^ rhs;
        self.set_flag(Flag::C, false);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, (result.count_ones() & 1) == 0);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
        self.set_register(Register::A, result);
    }

    #[inline]
    fn xor(&mut self, reg: Register) -> usize {
        self.xor_base(self.register(reg));
        4
    }

    #[inline]
    fn xor_hl_indirect(&mut self, bus: &mut impl Bus) -> usize {
        let rhs = bus.read(self.hl);
        self.xor_base(rhs);
        7
    }

    #[inline]
    fn or_base(&mut self, rhs: u8) {
        let data = self.register(Register::A);
        let result = data | rhs;
        self.set_flag(Flag::C, false);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, (result.count_ones() & 1) == 0);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
        self.set_register(Register::A, result);
    }

    #[inline]
    fn or(&mut self, reg: Register) -> usize {
        self.or_base(self.register(reg));
        4
    }

    #[inline]
    fn or_hl_indirect(&mut self, bus: &mut impl Bus) -> usize {
        let rhs = bus.read(self.hl);
        self.or_base(rhs);
        7
    }

    #[inline]
    fn compare_base(&mut self, rhs: u8) {
        let lhs = self.register(Register::A);
        let (result, carry) = lhs.borrowing_sub(rhs, false);
        self.set_flag(Flag::C, carry);
        self.set_flag(Flag::N, true);
        self.set_flag(
            Flag::PV,
            ((((lhs ^ rhs) & (result ^ lhs)) >> 5) & (Flag::PV as u8)) != 0,
        );
        // note that the flags for X and Y are taken from sub
        // this is what differentiates this from sbc_base's flags
        self.set_flag(Flag::X, (rhs & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, ((lhs ^ rhs ^ result) & (Flag::H as u8)) != 0);
        self.set_flag(Flag::Y, (rhs & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
    }

    #[inline]
    fn compare(&mut self, reg: Register) -> usize {
        self.compare_base(self.register(reg));
        4
    }

    #[inline]
    fn compare_hl_indirect(&mut self, bus: &mut impl Bus) -> usize {
        let rhs = bus.read(self.hl);
        self.compare_base(rhs);
        7
    }

    #[inline]
    fn pop_base(&mut self, bus: &mut impl Bus) -> u16 {
        let low = bus.read(self.sp);
        self.sp = self.sp.carrying_add(1, false).0;
        let high = bus.read(self.sp);
        self.sp = self.sp.carrying_add(1, false).0;
        ((high as u16) << 8) | (low as u16)
    }

    #[inline]
    fn return_wz(&mut self, bus: &mut impl Bus) -> usize {
        let wz = self.pop_base(bus);
        self.pc = wz;
        self.wz = wz;
        10
    }

    #[inline]
    fn conditional_return_wz(&mut self, condition: Condition, bus: &mut impl Bus) -> usize {
        if self.condition(condition) {
            self.return_wz(bus) + 1
        } else {
            5
        }
    }

    #[inline]
    fn pop(&mut self, reg: WideRegister, bus: &mut impl Bus) -> usize {
        let data = self.pop_base(bus);
        self.set_wide_register(reg, data);
        10
    }

    #[inline]
    fn conditional_jump_wz(&mut self, condition: Condition, bus: &mut impl Bus) -> usize {
        let addr = self.wide_immediate(bus);
        self.wz = addr;
        if self.condition(condition) {
            self.pc = addr;
        }
        10
    }

    #[inline]
    fn jump_wz(&mut self, bus: &mut impl Bus) -> usize {
        let addr = self.wide_immediate(bus);
        self.wz = addr;
        self.pc = addr;
        10
    }

    #[inline]
    fn push_base(&mut self, data: u16, bus: &mut impl Bus) {
        self.sp = self.sp.borrowing_sub(1, false).0;
        bus.write(self.sp, (data >> 8) as u8);
        self.sp = self.sp.borrowing_sub(1, false).0;
        bus.write(self.sp, data as u8);
    }

    #[inline]
    fn call_wz(&mut self, bus: &mut impl Bus) -> usize {
        let wz = self.wide_immediate(bus);
        self.push_base(self.pc, bus);
        self.pc = wz;
        self.wz = wz;
        17
    }

    #[inline]
    fn conditional_call_wz(&mut self, condition: Condition, bus: &mut impl Bus) -> usize {
        if self.condition(condition) {
            self.call_wz(bus)
        } else {
            self.wz = self.wide_immediate(bus);
            10
        }
    }

    #[inline]
    fn push(&mut self, reg: WideRegister, bus: &mut impl Bus) -> usize {
        let data = self.wide_register(reg);
        self.push_base(data, bus);
        bus.write(self.sp, data as u8);
        11
    }

    #[inline]
    fn add_carry_immediate(&mut self, carry: bool, bus: &mut impl Bus) -> usize {
        let rhs = self.immediate(bus);
        self.add_carry_base(rhs, carry);
        7
    }

    #[inline]
    fn reset_wz(&mut self, offset: u8, bus: &mut impl Bus) -> usize {
        self.push_base(self.pc, bus);
        self.pc = offset as u16;
        self.wz = offset as u16;
        11
    }

    #[inline]
    fn output_immediate_indirect(&mut self, bus: &mut impl Bus) -> usize {
        let a = self.register(Register::A);
        let port = ((a as u16) << 8) | (self.immediate(bus) as u16);
        bus.output(port, a);
        11
    }

    #[inline]
    fn sub_carry_immediate(&mut self, carry: bool, bus: &mut impl Bus) -> usize {
        let rhs = self.immediate(bus);
        self.sub_carry_base(rhs, carry);
        7
    }

    #[inline]
    fn exchange_extra(&mut self) -> usize {
        mem::swap(&mut self.bc, &mut self.bc_prime);
        mem::swap(&mut self.de, &mut self.de_prime);
        mem::swap(&mut self.hl, &mut self.hl_prime);
        mem::swap(&mut self.wz, &mut self.wz_prime);
        4
    }

    #[inline]
    fn input_immediate_indirect(&mut self, bus: &mut impl Bus) -> usize {
        let a = self.register(Register::A);
        let port = ((a as u16) << 8) | (self.immediate(bus) as u16);
        self.set_register(Register::A, bus.input(port));
        11
    }

    #[inline]
    fn exchange_stack_pointer_indirect_wz(
        &mut self,
        reg: WideRegister,
        bus: &mut impl Bus,
    ) -> usize {
        self.wz = self.pop_base(bus);

        let data = self.wide_register(reg);
        self.push_base(data, bus);

        self.set_wide_register(reg, self.wz);
        19
    }

    #[inline]
    fn and_immediate(&mut self, bus: &mut impl Bus) -> usize {
        let rhs = self.immediate(bus);
        self.and_base(rhs);
        7
    }

    #[inline]
    fn jump_indirect(&mut self, reg: WideRegister, bus: &mut impl Bus) -> usize {
        let addr = self.wide_register(reg);
        let low = bus.read(addr);
        let high = bus.read(addr.carrying_add(1, false).0);
        self.pc = ((high as u16) << 8) | (low as u16);
        4
    }

    #[inline]
    fn xor_immediate(&mut self, bus: &mut impl Bus) -> usize {
        let rhs = self.immediate(bus);
        self.xor_base(rhs);
        7
    }

    #[inline]
    fn disable_interrupts(&mut self) -> usize {
        self.iff1 = false;
        self.iff2 = false;
        4
    }

    #[inline]
    fn or_immediate(&mut self, bus: &mut impl Bus) -> usize {
        let rhs = self.immediate(bus);
        self.or_base(rhs);
        7
    }

    #[inline]
    fn enable_interrupts(&mut self) -> usize {
        self.interrupts_enabled = true;
        4
    }

    #[inline]
    fn copy_wide_register(&mut self, dst: WideRegister, src: WideRegister) -> usize {
        self.set_wide_register(dst, self.wide_register(src));
        6
    }

    #[inline]
    fn compare_immediate(&mut self, bus: &mut impl Bus) -> usize {
        let rhs = self.immediate(bus);
        self.compare_base(rhs);
        7
    }

    #[inline]
    fn rlc_base(&mut self, data: u8) -> u8 {
        let result = (data << 1) | (data >> 7);
        self.set_flag(Flag::C, (data & 0x80) != 0);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, (result.count_ones() & 1) == 0);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);

        result
    }

    #[inline]
    fn rlc_register(&mut self, reg: Register) -> usize {
        let result = self.rlc_base(self.register(reg));
        self.set_register(reg, result);
        4
    }

    #[inline]
    fn rlc_hl_indirect(&mut self, bus: &mut impl Bus) -> usize {
        let data = bus.read(self.hl);
        let result = self.rlc_base(data);
        bus.write(self.hl, result);
        11
    }

    #[inline]
    fn rrc_base(&mut self, data: u8) -> u8 {
        let result = (data >> 1) | (data << 7);
        self.set_flag(Flag::C, (data & 0x01) != 0);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, (result.count_ones() & 1) == 0);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);

        result
    }

    #[inline]
    fn rrc_register(&mut self, reg: Register) -> usize {
        let result = self.rrc_base(self.register(reg));
        self.set_register(reg, result);
        4
    }

    #[inline]
    fn rrc_hl_indirect(&mut self, bus: &mut impl Bus) -> usize {
        let data = bus.read(self.hl);
        let result = self.rrc_base(data);
        bus.write(self.hl, result);
        11
    }

    #[inline]
    fn rl_base(&mut self, data: u8) -> u8 {
        let result = (data << 1) | (if self.flag(Flag::C) { 0x01 } else { 0x00 });
        self.set_flag(Flag::C, (data & 0x80) != 0);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, (result.count_ones() & 1) == 0);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);

        result
    }

    #[inline]
    fn rl_register(&mut self, reg: Register) -> usize {
        let result = self.rl_base(self.register(reg));
        self.set_register(reg, result);
        4
    }

    #[inline]
    fn rl_hl_indirect(&mut self, bus: &mut impl Bus) -> usize {
        let data = bus.read(self.hl);
        let result = self.rl_base(data);
        bus.write(self.hl, result);
        11
    }

    #[inline]
    fn rr_base(&mut self, data: u8) -> u8 {
        let result = (data >> 1) | (if self.flag(Flag::C) { 0x80 } else { 0x00 });
        self.set_flag(Flag::C, (data & 0x01) != 0);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, (result.count_ones() & 1) == 0);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);

        result
    }

    #[inline]
    fn rr_register(&mut self, reg: Register) -> usize {
        let result = self.rr_base(self.register(reg));
        self.set_register(reg, result);
        4
    }

    #[inline]
    fn rr_hl_indirect(&mut self, bus: &mut impl Bus) -> usize {
        let data = bus.read(self.hl);
        let result = self.rr_base(data);
        bus.write(self.hl, result);
        11
    }

    #[inline]
    fn sla_base(&mut self, data: u8) -> u8 {
        let result = data << 1;
        self.set_flag(Flag::C, (data & 0x80) != 0);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, (result.count_ones() & 1) == 0);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);

        result
    }

    #[inline]
    fn sla_register(&mut self, reg: Register) -> usize {
        let result = self.sla_base(self.register(reg));
        self.set_register(reg, result);
        4
    }

    #[inline]
    fn sla_hl_indirect(&mut self, bus: &mut impl Bus) -> usize {
        let data = bus.read(self.hl);
        let result = self.sla_base(data);
        bus.write(self.hl, result);
        11
    }

    #[inline]
    fn sra_base(&mut self, data: u8) -> u8 {
        let result = (data >> 1) | (data & 0x80);
        self.set_flag(Flag::C, (data & 0x01) != 0);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, (result.count_ones() & 1) == 0);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);

        result
    }

    #[inline]
    fn sra_register(&mut self, reg: Register) -> usize {
        let result = self.sra_base(self.register(reg));
        self.set_register(reg, result);
        4
    }

    #[inline]
    fn sra_hl_indirect(&mut self, bus: &mut impl Bus) -> usize {
        let data = bus.read(self.hl);
        let result = self.sra_base(data);
        bus.write(self.hl, result);
        11
    }

    #[inline]
    fn sll_base(&mut self, data: u8) -> u8 {
        let result = (data << 1) | 0x01;
        self.set_flag(Flag::C, (data & 0x80) != 0);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, (result.count_ones() & 1) == 0);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);

        result
    }

    #[inline]
    fn sll_register(&mut self, reg: Register) -> usize {
        let result = self.sll_base(self.register(reg));
        self.set_register(reg, result);
        4
    }

    #[inline]
    fn sll_hl_indirect(&mut self, bus: &mut impl Bus) -> usize {
        let data = bus.read(self.hl);
        let result = self.sll_base(data);
        bus.write(self.hl, result);
        11
    }

    #[inline]
    fn srl_base(&mut self, data: u8) -> u8 {
        let result = data >> 1;
        self.set_flag(Flag::C, (data & 0x01) != 0);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, (result.count_ones() & 1) == 0);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);

        result
    }

    #[inline]
    fn srl_register(&mut self, reg: Register) -> usize {
        let result = self.srl_base(self.register(reg));
        self.set_register(reg, result);
        4
    }

    #[inline]
    fn srl_hl_indirect(&mut self, bus: &mut impl Bus) -> usize {
        let data = bus.read(self.hl);
        let result = self.srl_base(data);
        bus.write(self.hl, result);
        11
    }

    #[inline]
    fn bit_register(&mut self, mask: u8, reg: Register) -> usize {
        let data = self.register(reg);
        let result = data & mask;
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, result == 0);
        self.set_flag(Flag::X, (data & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, true);
        self.set_flag(Flag::Y, (data & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
        4
    }

    #[inline]
    fn bit_hl_indirect_wz(&mut self, mask: u8, bus: &mut impl Bus) -> usize {
        let result = bus.read(self.hl) & mask;
        let w = (self.wz >> 8) as u8;
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, result == 0);
        self.set_flag(Flag::X, (w & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, true);
        self.set_flag(Flag::Y, (w & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
        8
    }

    #[inline]
    fn reset_bit_register(&mut self, mask: u8, reg: Register) -> usize {
        let result = self.register(reg) & !mask;
        self.set_register(reg, result);
        4
    }

    #[inline]
    fn reset_bit_hl_indirect(&mut self, mask: u8, bus: &mut impl Bus) -> usize {
        let result = bus.read(self.hl) & !mask;
        bus.write(self.hl, result);
        11
    }

    #[inline]
    fn set_bit_register(&mut self, mask: u8, reg: Register) -> usize {
        let result = self.register(reg) | mask;
        self.set_register(reg, result);
        4
    }

    #[inline]
    fn set_bit_hl_indirect(&mut self, mask: u8, bus: &mut impl Bus) -> usize {
        let result = bus.read(self.hl) | mask;
        bus.write(self.hl, result);
        11
    }

    #[inline]
    fn inc_index_indirect_wz(&mut self, index: WideRegister, bus: &mut impl Bus) -> usize {
        let addr = self.wide_register(index);
        let offset = self.immediate(bus) as i8 as i16;
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let data = bus.read(addr);
        let result = data.carrying_add(1, false).0;
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, result == 0x80);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, ((result ^ data) & (Flag::H as u8)) != 0);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
        bus.write(addr, result);
        19
    }

    #[inline]
    fn dec_index_indirect_wz(&mut self, index: WideRegister, bus: &mut impl Bus) -> usize {
        let addr = self.wide_register(index);
        let offset = self.immediate(bus) as i8 as i16;
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let data = bus.read(addr);
        let result = data.borrowing_sub(1, false).0;
        self.set_flag(Flag::N, true);
        self.set_flag(Flag::PV, result == 0x7F);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, ((result ^ data) & (Flag::H as u8)) != 0);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
        bus.write(addr, result);
        19
    }

    #[inline]
    fn write_immediate_index_indirect_wz(
        &mut self,
        index: WideRegister,
        bus: &mut impl Bus,
    ) -> usize {
        let addr = self.wide_register(index);
        let offset = self.immediate(bus) as i8 as i16;
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let data = self.immediate(bus);
        bus.write(addr, data);
        15
    }

    #[inline]
    fn read_index_indirect_wz(
        &mut self,
        reg: Register,
        index: WideRegister,
        bus: &mut impl Bus,
    ) -> usize {
        let addr = self.wide_register(index);
        let offset = self.immediate(bus) as i8 as i16;
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let data = bus.read(addr);
        self.set_register(reg, data);
        15
    }

    #[inline]
    fn write_index_indirect_wz(
        &mut self,
        index: WideRegister,
        reg: Register,
        bus: &mut impl Bus,
    ) -> usize {
        let addr = self.wide_register(index);
        let offset = self.immediate(bus) as i8 as i16;
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        bus.write(addr, self.register(reg));
        15
    }

    #[inline]
    fn add_carry_index_indirect_wz(
        &mut self,
        index: WideRegister,
        carry: bool,
        bus: &mut impl Bus,
    ) -> usize {
        let addr = self.wide_register(index);
        let offset = self.immediate(bus) as i8 as i16;
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let rhs = bus.read(addr);
        self.add_carry_base(rhs, carry);
        15
    }

    #[inline]
    fn sub_carry_index_indirect_wz(
        &mut self,
        index: WideRegister,
        carry: bool,
        bus: &mut impl Bus,
    ) -> usize {
        let addr = self.wide_register(index);
        let offset = self.immediate(bus) as i8 as i16;
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let rhs = bus.read(addr);
        self.sub_carry_base(rhs, carry);
        15
    }

    #[inline]
    fn and_index_indirect_wz(&mut self, index: WideRegister, bus: &mut impl Bus) -> usize {
        let addr = self.wide_register(index);
        let offset = self.immediate(bus) as i8 as i16;
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let rhs = bus.read(addr);
        self.and_base(rhs);
        15
    }

    #[inline]
    fn xor_index_indirect_wz(&mut self, index: WideRegister, bus: &mut impl Bus) -> usize {
        let addr = self.wide_register(index);
        let offset = self.immediate(bus) as i8 as i16;
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let rhs = bus.read(addr);
        self.xor_base(rhs);
        15
    }

    #[inline]
    fn or_index_indirect_wz(&mut self, index: WideRegister, bus: &mut impl Bus) -> usize {
        let addr = self.wide_register(index);
        let offset = self.immediate(bus) as i8 as i16;
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let rhs = bus.read(addr);
        self.or_base(rhs);
        15
    }

    #[inline]
    fn compare_index_indirect_wz(&mut self, index: WideRegister, bus: &mut impl Bus) -> usize {
        let addr = self.wide_register(index);
        let offset = self.immediate(bus) as i8 as i16;
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let rhs = bus.read(addr);
        self.compare_base(rhs);
        15
    }

    #[inline]
    fn input(&mut self, reg: Register, bus: &mut impl Bus) -> usize {
        let data = bus.input(self.bc);
        self.set_register(reg, data);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, (data.count_ones() & 1) == 0);
        self.set_flag(Flag::X, (data & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (data & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, data == 0);
        self.set_flag(Flag::S, (data & (Flag::S as u8)) != 0);
        8
    }

    #[inline]
    fn output(&mut self, reg: Register, bus: &mut impl Bus) -> usize {
        let data = self.register(reg);
        bus.output(self.bc, data);
        8
    }

    #[inline]
    fn sub_carry_wide_wz(&mut self, dst: WideRegister, rhs: WideRegister) -> usize {
        let lhs = self.wide_register(dst);
        let rhs = self.wide_register(rhs);
        self.wz = lhs.carrying_add(1, false).0;
        let (result, carry) = lhs.borrowing_sub(rhs, self.flag(Flag::C));
        self.set_wide_register(dst, result);
        self.set_flag(Flag::C, carry);
        self.set_flag(Flag::N, true);
        self.set_flag(
            Flag::PV,
            ((((rhs ^ lhs) & (lhs ^ result) & 0x8000) >> 13) & (Flag::PV as u16)) != 0,
        );
        self.set_flag(Flag::X, ((result >> 8) & (Flag::X as u16)) != 0);
        self.set_flag(
            Flag::H,
            (((lhs ^ result ^ rhs) >> 8) & (Flag::H as u16)) != 0,
        );
        self.set_flag(Flag::Y, ((result >> 8) & (Flag::Y as u16)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, ((result >> 8) & (Flag::S as u16)) != 0);
        11
    }

    #[inline]
    fn neg(&mut self) -> usize {
        let a = self.register(Register::A);
        self.set_register(Register::A, 0);
        self.sub_carry_base(a, false);
        4
    }

    #[inline]
    fn retn(&mut self, _: &mut impl Bus) -> usize {
        todo!()
    }

    #[inline]
    fn set_interrupt_mode(&mut self, mode: InterruptMode) -> usize {
        self.interrupt_mode = mode;
        4
    }

    #[inline]
    fn copy_ir_register(&mut self, reg: Register) -> usize {
        let data = self.register(reg);
        self.set_register(Register::A, data);
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, self.iff2);
        self.set_flag(Flag::X, (data & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (data & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, data == 0);
        self.set_flag(Flag::S, (data & (Flag::S as u8)) != 0);
        5
    }

    #[inline]
    fn reti_wz(&mut self, bus: &mut impl Bus) -> usize {
        let cycles = 1 + self.return_wz(bus);
        // signal to the bus that we're open for business
        self.reti = true;
        cycles
    }

    #[inline]
    fn add_carry_wide_wz(&mut self, dst: WideRegister, rhs: WideRegister) -> usize {
        let lhs = self.wide_register(dst);
        let rhs = self.wide_register(rhs);
        self.wz = lhs.carrying_add(1, false).0;
        let (result, carry) = lhs.carrying_add(rhs, self.flag(Flag::C));
        self.set_wide_register(dst, result);
        self.set_flag(Flag::C, carry);
        self.set_flag(Flag::N, false);
        self.set_flag(
            Flag::PV,
            ((((rhs ^ lhs ^ 0x8000) & (rhs ^ result) & 0x8000) >> 13) & (Flag::PV as u16)) != 0,
        );
        self.set_flag(Flag::X, ((result >> 8) & (Flag::X as u16)) != 0);
        self.set_flag(
            Flag::H,
            (((lhs ^ result ^ rhs) >> 8) & (Flag::H as u16)) != 0,
        );
        self.set_flag(Flag::Y, ((result >> 8) & (Flag::Y as u16)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, ((result >> 8) & (Flag::S as u16)) != 0);
        11
    }

    #[inline]
    fn rrd_wz(&mut self, bus: &mut impl Bus) -> usize {
        let addr = self.hl;
        let data = bus.read(addr);
        let a = self.register(Register::A);
        let result = (a & 0xF0) | (data & 0x0F);
        self.set_register(Register::A, result);
        bus.write(addr, (data >> 4) | (a << 4));
        self.wz = addr.carrying_add(1, false).0;
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, (result.count_ones() & 1) == 0);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
        14
    }

    #[inline]
    fn rld_wz(&mut self, bus: &mut impl Bus) -> usize {
        let addr = self.hl;
        let data = bus.read(addr);
        let a = self.register(Register::A);
        let result = (a & 0xF0) | (data >> 4);
        self.set_register(Register::A, result);
        bus.write(addr, (data << 4) | (a & 0x0F));
        self.wz = addr.carrying_add(1, false).0;
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, (result.count_ones() & 1) == 0);
        self.set_flag(Flag::X, (result & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
        14
    }

    #[inline]
    fn input_and_drop(&mut self, bus: &mut impl Bus) -> usize {
        bus.input(self.bc);
        8
    }

    #[inline]
    fn output_zero(&mut self, bus: &mut impl Bus) -> usize {
        bus.output(self.bc, 0);
        8
    }

    #[inline]
    fn ldi(&mut self, bus: &mut impl Bus) -> usize {
        let data = bus.read(self.hl);
        bus.write(self.de, data);
        self.hl = self.hl.carrying_add(1, false).0;
        self.de = self.de.carrying_add(1, false).0;
        self.bc = self.bc.borrowing_sub(1, false).0;
        let result = data.carrying_add(self.register(Register::A), false).0;
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, self.bc != 0);
        self.set_flag(Flag::X, (result & 0x08) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & 0x02) != 0);
        12
    }

    #[inline]
    fn cpi_wz(&mut self, bus: &mut impl Bus) -> usize {
        self.wz = self.wz.carrying_add(1, false).0;
        let hl = self.hl;
        self.hl = hl.carrying_add(1, false).0;
        self.bc = self.bc.borrowing_sub(1, false).0;
        let a = self.register(Register::A);
        let mut result = a.borrowing_sub(bus.read(hl), false).0;
        self.set_flag(Flag::N, true);
        self.set_flag(Flag::PV, self.bc != 0);
        self.set_flag(Flag::H, (result & 0x0F) > (a & 0x0F));
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, result & (Flag::S as u8) != 0);
        if self.flag(Flag::H) {
            result = result.borrowing_sub(1, false).0;
        }
        self.set_flag(Flag::X, result & (Flag::X as u8) != 0);
        self.set_flag(Flag::Y, result & (Flag::Y as u8) != 0);
        12
    }

    #[inline]
    fn ini(&mut self, _: &mut impl Bus) -> usize {
        todo!()
    }

    #[inline]
    fn outi(&mut self, _: &mut impl Bus) -> usize {
        todo!()
    }

    #[inline]
    fn ldd(&mut self, bus: &mut impl Bus) -> usize {
        let data = bus.read(self.hl);
        bus.write(self.de, data);
        self.hl = self.hl.borrowing_sub(1, false).0;
        self.de = self.de.borrowing_sub(1, false).0;
        self.bc = self.bc.borrowing_sub(1, false).0;
        let result = data.carrying_add(self.register(Register::A), false).0;
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, self.bc != 0);
        self.set_flag(Flag::X, (result & 0x08) != 0);
        self.set_flag(Flag::H, false);
        self.set_flag(Flag::Y, (result & 0x02) != 0);
        12
    }

    #[inline]
    fn cpd_wz(&mut self, bus: &mut impl Bus) -> usize {
        self.wz = self.wz.borrowing_sub(1, false).0;
        let hl = self.hl;
        self.hl = hl.borrowing_sub(1, false).0;
        self.bc = self.bc.borrowing_sub(1, false).0;
        let a = self.register(Register::A);
        let mut result = a.borrowing_sub(bus.read(hl), false).0;
        self.set_flag(Flag::N, true);
        self.set_flag(Flag::PV, self.bc != 0);
        self.set_flag(Flag::H, (result & 0x0F) > (a & 0x0F));
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, result & (Flag::S as u8) != 0);
        if self.flag(Flag::H) {
            result = result.borrowing_sub(1, false).0;
        }
        self.set_flag(Flag::X, result & (Flag::X as u8) != 0);
        self.set_flag(Flag::Y, result & (Flag::Y as u8) != 0);
        12
    }

    #[inline]
    fn ind(&mut self, _: &mut impl Bus) -> usize {
        todo!()
    }

    #[inline]
    fn outd(&mut self, _: &mut impl Bus) -> usize {
        todo!()
    }

    #[inline]
    fn ldir(&mut self, bus: &mut impl Bus) -> usize {
        let cycles = self.ldi(bus);
        if self.flag(Flag::PV) {
            let pc = self.pc;
            self.pc = pc.borrowing_sub(2, false).0;
            self.wz = pc.carrying_add(1, false).0;
            5 + cycles
        } else {
            cycles
        }
    }

    #[inline]
    fn cpir(&mut self, bus: &mut impl Bus) -> usize {
        let cycles = self.cpi_wz(bus);
        if self.flag(Flag::PV) && !self.flag(Flag::Z) {
            let pc = self.pc;
            self.pc = pc.borrowing_sub(2, false).0;
            self.wz = pc.carrying_add(1, false).0;
            5 + cycles
        } else {
            cycles
        }
    }

    #[inline]
    fn inir(&mut self, _: &mut impl Bus) -> usize {
        todo!()
    }

    #[inline]
    fn otir(&mut self, _: &mut impl Bus) -> usize {
        todo!()
    }

    #[inline]
    fn lddr(&mut self, bus: &mut impl Bus) -> usize {
        let cycles = self.ldd(bus);
        if self.flag(Flag::PV) {
            let pc = self.pc;
            self.pc = pc.borrowing_sub(2, false).0;
            self.wz = pc.carrying_add(1, false).0;
            5 + cycles
        } else {
            cycles
        }
    }

    #[inline]
    fn cpdr_wz(&mut self, bus: &mut impl Bus) -> usize {
        let cycles = self.cpd_wz(bus);
        if self.flag(Flag::PV) && !self.flag(Flag::Z) {
            let pc = self.pc;
            self.pc = pc.borrowing_sub(2, false).0;
            self.wz = pc.carrying_add(1, false).0;
            5 + cycles
        } else {
            cycles
        }
    }

    #[inline]
    fn indr(&mut self, _: &mut impl Bus) -> usize {
        todo!()
    }

    #[inline]
    fn otdr(&mut self, _: &mut impl Bus) -> usize {
        todo!()
    }

    #[inline]
    fn rlc_index_indirect_wz(
        &mut self,
        offset: i16,
        index: WideRegister,
        reg: Option<Register>,
        bus: &mut impl Bus,
    ) -> usize {
        let addr = self.wide_register(index);
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let data = bus.read(addr);
        let result = self.rlc_base(data);
        bus.write(addr, result);
        if let Some(reg) = reg {
            self.set_register(reg, result);
        }
        15
    }

    #[inline]
    fn rrc_index_indirect_wz(
        &mut self,
        offset: i16,
        index: WideRegister,
        reg: Option<Register>,
        bus: &mut impl Bus,
    ) -> usize {
        let addr = self.wide_register(index);
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let data = bus.read(addr);
        let result = self.rrc_base(data);
        bus.write(addr, result);
        if let Some(reg) = reg {
            self.set_register(reg, result);
        }
        15
    }

    #[inline]
    fn rl_index_indirect_wz(
        &mut self,
        offset: i16,
        index: WideRegister,
        reg: Option<Register>,
        bus: &mut impl Bus,
    ) -> usize {
        let addr = self.wide_register(index);
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let data = bus.read(addr);
        let result = self.rl_base(data);
        bus.write(addr, result);
        if let Some(reg) = reg {
            self.set_register(reg, result);
        }
        15
    }

    #[inline]
    fn rr_index_indirect_wz(
        &mut self,
        offset: i16,
        index: WideRegister,
        reg: Option<Register>,
        bus: &mut impl Bus,
    ) -> usize {
        let addr = self.wide_register(index);
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let data = bus.read(addr);
        let result = self.rr_base(data);
        bus.write(addr, result);
        if let Some(reg) = reg {
            self.set_register(reg, result);
        }
        15
    }

    #[inline]
    fn sla_index_indirect_wz(
        &mut self,
        offset: i16,
        index: WideRegister,
        reg: Option<Register>,
        bus: &mut impl Bus,
    ) -> usize {
        let addr = self.wide_register(index);
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let data = bus.read(addr);
        let result = self.sla_base(data);
        bus.write(addr, result);
        if let Some(reg) = reg {
            self.set_register(reg, result);
        }
        15
    }

    #[inline]
    fn sra_index_indirect_wz(
        &mut self,
        offset: i16,
        index: WideRegister,
        reg: Option<Register>,
        bus: &mut impl Bus,
    ) -> usize {
        let addr = self.wide_register(index);
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let data = bus.read(addr);
        let result = self.sra_base(data);
        bus.write(addr, result);
        if let Some(reg) = reg {
            self.set_register(reg, result);
        }
        15
    }

    #[inline]
    fn sll_index_indirect_wz(
        &mut self,
        offset: i16,
        index: WideRegister,
        reg: Option<Register>,
        bus: &mut impl Bus,
    ) -> usize {
        let addr = self.wide_register(index);
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let data = bus.read(addr);
        let result = self.sll_base(data);
        bus.write(addr, result);
        if let Some(reg) = reg {
            self.set_register(reg, result);
        }
        15
    }

    #[inline]
    fn srl_index_indirect_wz(
        &mut self,
        offset: i16,
        index: WideRegister,
        reg: Option<Register>,
        bus: &mut impl Bus,
    ) -> usize {
        let addr = self.wide_register(index);
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let data = bus.read(addr);
        let result = self.srl_base(data);
        bus.write(addr, result);
        if let Some(reg) = reg {
            self.set_register(reg, result);
        }
        15
    }

    #[inline]
    fn bit_index_indirect_wz(
        &mut self,
        mask: u8,
        offset: i16,
        index: WideRegister,
        bus: &mut impl Bus,
    ) -> usize {
        let addr = self.wide_register(index);
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let result = bus.read(addr) & mask;
        let w = (self.wz >> 8) as u8;
        self.set_flag(Flag::N, false);
        self.set_flag(Flag::PV, result == 0);
        self.set_flag(Flag::X, (w & (Flag::X as u8)) != 0);
        self.set_flag(Flag::H, true);
        self.set_flag(Flag::Y, (w & (Flag::Y as u8)) != 0);
        self.set_flag(Flag::Z, result == 0);
        self.set_flag(Flag::S, (result & (Flag::S as u8)) != 0);
        11
    }

    #[inline]
    fn reset_bit_index_indirect_wz(
        &mut self,
        mask: u8,
        offset: i16,
        index: WideRegister,
        reg: Option<Register>,
        bus: &mut impl Bus,
    ) -> usize {
        let addr = self.wide_register(index);
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let result = bus.read(addr) & !mask;
        bus.write(addr, result);
        if let Some(reg) = reg {
            self.set_register(reg, result);
        }
        15
    }

    #[inline]
    fn set_bit_index_indirect_wz(
        &mut self,
        mask: u8,
        offset: i16,
        index: WideRegister,
        reg: Option<Register>,
        bus: &mut impl Bus,
    ) -> usize {
        let addr = self.wide_register(index);
        let addr = addr.carrying_add(offset as u16, false).0;
        self.wz = addr;
        let result = bus.read(addr) | mask;
        bus.write(addr, result);
        if let Some(reg) = reg {
            self.set_register(reg, result);
        }
        15
    }

    #[inline]
    pub fn returned_from_interrupt(&self) -> bool {
        self.reti
    }

    pub fn step(&mut self, bus: &mut impl Bus) -> usize {
        // We've finished returning from interrupts
        self.reti = false;

        // TODO: service interrupts here

        let opcode = self.fetch(bus);
        #[rustfmt::skip]
        match opcode {
            0x00 => /* nop              */ self.nop(),
            0x01 => /* ld bc, **        */ self.read_wide_immediate(WideRegister::BC, bus),
            0x02 => /* ld (bc), a       */ self.write_indirect_wz(WideRegister::BC, Register::A, bus),
            0x03 => /* inc bc           */ self.inc_wide(WideRegister::BC),
            0x04 => /* inc b            */ self.inc_wz(Register::B),
            0x05 => /* dec c            */ self.dec_wz(Register::B),
            0x06 => /* ld b, *          */ self.read_immediate(Register::B, bus),
            0x07 => /* rlca             */ self.rlca(),
            0x08 => /* ex af, af'       */ self.exchange(WideRegister::AF, WideRegister::AFPrime),
            0x09 => /* add hl, bc       */ self.add_wide_wz(WideRegister::HL, WideRegister::BC),
            0x0A => /* ld a, (bc)       */ self.read_indirect_wz(Register::A, WideRegister::BC, bus),
            0x0B => /* dec bc           */ self.dec_wide(WideRegister::BC),
            0x0C => /* inc c            */ self.inc_wz(Register::C),
            0x0D => /* dec c            */ self.dec_wz(Register::C),
            0x0E => /* ld c, *          */ self.read_immediate(Register::C, bus),
            0x0F => /* rrca             */ self.rrca(),

            0x10 => /* djnz             */ self.djnz_wz(bus),
            0x11 => /* ld de, **        */ self.read_wide_immediate(WideRegister::DE, bus),
            0x12 => /* ld (de), a       */ self.write_indirect_wz(WideRegister::DE, Register::A, bus),
            0x13 => /* inc de           */ self.inc_wide(WideRegister::DE),
            0x14 => /* inc d            */ self.inc_wz(Register::D),
            0x15 => /* dec d            */ self.dec_wz(Register::D),
            0x16 => /* ld d, *          */ self.read_immediate(Register::D, bus),
            0x17 => /* rla              */ self.rla(),
            0x18 => /* jr *             */ self.jump_relative_wz(bus),
            0x19 => /* add hl, de       */ self.add_wide_wz(WideRegister::HL, WideRegister::DE),
            0x1A => /* ld a, (de)       */ self.read_indirect_wz(Register::A, WideRegister::DE, bus),
            0x1B => /* dec de           */ self.dec_wide(WideRegister::DE),
            0x1C => /* inc e            */ self.inc_wz(Register::E),
            0x1D => /* dec e            */ self.dec_wz(Register::E),
            0x1E => /* ld e, *          */ self.read_immediate(Register::E, bus),
            0x1F => /* rra              */ self.rra(),

            0x20 => /* jr cc, *         */ self.conditional_jump_relative_wz(Condition::NZ, bus),
            0x21 => /* ld hl, **        */ self.read_wide_immediate(WideRegister::HL, bus),
            0x22 => /* ld (**), hl      */ self.write_wide_absolute_wz(WideRegister::HL, bus),
            0x23 => /* inc hl           */ self.inc_wide(WideRegister::HL),
            0x24 => /* inc h            */ self.inc_wz(Register::H),
            0x25 => /* dec h            */ self.dec_wz(Register::H),
            0x26 => /* ld h, **         */ self.read_immediate(Register::H, bus),
            0x27 => /* daa              */ self.daa(),
            0x28 => /* jr z, *          */ self.conditional_jump_relative_wz(Condition::Z, bus),
            0x29 => /* add hl, hl       */ self.add_wide_wz(WideRegister::HL, WideRegister::HL),
            0x2A => /* ld hl, (**)      */ self.read_wide_absolute_wz(WideRegister::HL, bus),
            0x2B => /* dec hl           */ self.dec_wide(WideRegister::HL),
            0x2C => /* inc l            */ self.inc_wz(Register::L),
            0x2D => /* dec l            */ self.dec_wz(Register::L),
            0x2E => /* ld l, *          */ self.read_immediate(Register::L, bus),
            0x2F => /* cpl              */ self.cpl(),

            0x30 => /* jr nc, *         */ self.conditional_jump_relative_wz(Condition::NC, bus),
            0x31 => /* ld sp, **        */ self.read_wide_immediate(WideRegister::SP, bus),
            0x32 => /* ld (**), a       */ self.write_absolute_wz(Register::A, bus),
            0x33 => /* inc sp           */ self.inc_wide(WideRegister::SP),
            0x34 => /* inc (hl)         */ self.inc_hl_indirect(bus),
            0x35 => /* dec (hl)         */ self.dec_hl_indirect(bus),
            0x36 => /* ld (hl), *       */ self.write_immediate_hl_indirect(bus),
            0x37 => /* scf              */ self.scf(),
            0x38 => /* jr c, *          */ self.conditional_jump_relative_wz(Condition::C, bus),
            0x39 => /* add hl, sp       */ self.add_wide_wz(WideRegister::HL, WideRegister::SP),
            0x3A => /* ld a, (**)       */ self.read_absolute_wz(Register::A, bus),
            0x3B => /* dec sp           */ self.dec_wide(WideRegister::SP),
            0x3C => /* inc a            */ self.inc_wz(Register::A),
            0x3D => /* dec a            */ self.dec_wz(Register::A),
            0x3E => /* ld a, *          */ self.read_immediate(Register::A, bus),
            0x3F => /* ccf              */ self.ccf(),

            0x40 => /* ld b, b          */ self.copy_register(Register::B, Register::B),
            0x41 => /* ld b, c          */ self.copy_register(Register::B, Register::C),
            0x42 => /* ld b, d          */ self.copy_register(Register::B, Register::D),
            0x43 => /* ld b, e          */ self.copy_register(Register::B, Register::E),
            0x44 => /* ld b, h          */ self.copy_register(Register::B, Register::H),
            0x45 => /* ld b, l          */ self.copy_register(Register::B, Register::L),
            0x46 => /* ld b, (hl)       */ self.copy_register_hl_indirect(Register::B, bus),
            0x47 => /* ld b, a          */ self.copy_register(Register::B, Register::A),
            0x48 => /* ld c, b          */ self.copy_register(Register::C, Register::B),
            0x49 => /* ld c, c          */ self.copy_register(Register::C, Register::C),
            0x4A => /* ld c, d          */ self.copy_register(Register::C, Register::D),
            0x4B => /* ld c, e          */ self.copy_register(Register::C, Register::E),
            0x4C => /* ld c, h          */ self.copy_register(Register::C, Register::H),
            0x4D => /* ld c, l          */ self.copy_register(Register::C, Register::L),
            0x4E => /* ld c, (hl)       */ self.copy_register_hl_indirect(Register::C, bus),
            0x4F => /* ld c, a          */ self.copy_register(Register::C, Register::A),

            0x50 => /* ld d, b          */ self.copy_register(Register::D, Register::B),
            0x51 => /* ld d, c          */ self.copy_register(Register::D, Register::C),
            0x52 => /* ld d, d          */ self.copy_register(Register::D, Register::D),
            0x53 => /* ld d, e          */ self.copy_register(Register::D, Register::E),
            0x54 => /* ld d, h          */ self.copy_register(Register::D, Register::H),
            0x55 => /* ld d, l          */ self.copy_register(Register::D, Register::L),
            0x56 => /* ld d, (hl)       */ self.copy_register_hl_indirect(Register::D, bus),
            0x57 => /* ld d, a          */ self.copy_register(Register::D, Register::A),
            0x58 => /* ld e, b          */ self.copy_register(Register::E, Register::B),
            0x59 => /* ld e, c          */ self.copy_register(Register::E, Register::C),
            0x5A => /* ld e, d          */ self.copy_register(Register::E, Register::D),
            0x5B => /* ld e, e          */ self.copy_register(Register::E, Register::E),
            0x5C => /* ld e, h          */ self.copy_register(Register::E, Register::H),
            0x5D => /* ld e, l          */ self.copy_register(Register::E, Register::L),
            0x5E => /* ld e, (hl)       */ self.copy_register_hl_indirect(Register::E, bus),
            0x5F => /* ld e, a          */ self.copy_register(Register::E, Register::A),

            0x60 => /* ld h, b          */ self.copy_register(Register::H, Register::B),
            0x61 => /* ld h, c          */ self.copy_register(Register::H, Register::C),
            0x62 => /* ld h, d          */ self.copy_register(Register::H, Register::D),
            0x63 => /* ld h, e          */ self.copy_register(Register::H, Register::E),
            0x64 => /* ld h, h          */ self.copy_register(Register::H, Register::H),
            0x65 => /* ld h, l          */ self.copy_register(Register::H, Register::L),
            0x66 => /* ld h, (hl)       */ self.copy_register_hl_indirect(Register::H, bus),
            0x67 => /* ld h, a          */ self.copy_register(Register::H, Register::A),
            0x68 => /* ld l, b          */ self.copy_register(Register::L, Register::B),
            0x69 => /* ld l, c          */ self.copy_register(Register::L, Register::C),
            0x6A => /* ld l, d          */ self.copy_register(Register::L, Register::D),
            0x6B => /* ld l, e          */ self.copy_register(Register::L, Register::E),
            0x6C => /* ld l, h          */ self.copy_register(Register::L, Register::H),
            0x6D => /* ld l, l          */ self.copy_register(Register::L, Register::L),
            0x6E => /* ld l, (hl)       */ self.copy_register_hl_indirect(Register::L, bus),
            0x6F => /* ld l, a          */ self.copy_register(Register::L, Register::A),

            0x70 => /* ld (hl), b       */ self.copy_hl_indirect_register(Register::B, bus),
            0x71 => /* ld (hl), c       */ self.copy_hl_indirect_register(Register::C, bus),
            0x72 => /* ld (hl), d       */ self.copy_hl_indirect_register(Register::D, bus),
            0x73 => /* ld (hl), e       */ self.copy_hl_indirect_register(Register::E, bus),
            0x74 => /* ld (hl), h       */ self.copy_hl_indirect_register(Register::H, bus),
            0x75 => /* ld (hl), l       */ self.copy_hl_indirect_register(Register::L, bus),
            0x76 => /* halt             */ self.halt(),
            0x77 => /* ld (hl), a       */ self.copy_hl_indirect_register(Register::A, bus),
            0x78 => /* ld a, b          */ self.copy_register(Register::A, Register::B),
            0x79 => /* ld a, c          */ self.copy_register(Register::A, Register::C),
            0x7A => /* ld a, d          */ self.copy_register(Register::A, Register::D),
            0x7B => /* ld a, e          */ self.copy_register(Register::A, Register::E),
            0x7C => /* ld a, h          */ self.copy_register(Register::A, Register::H),
            0x7D => /* ld a, l          */ self.copy_register(Register::A, Register::L),
            0x7E => /* ld a, (hl)       */ self.copy_register_hl_indirect(Register::A, bus),
            0x7F => /* ld a, a          */ self.copy_register(Register::A, Register::A),

            0x80 => /* add a, b         */ self.add_carry(Register::B, false),
            0x81 => /* add a, c         */ self.add_carry(Register::C, false),
            0x82 => /* add a, d         */ self.add_carry(Register::D, false),
            0x83 => /* add a, e         */ self.add_carry(Register::E, false),
            0x84 => /* add a, h         */ self.add_carry(Register::H, false),
            0x85 => /* add a, l         */ self.add_carry(Register::L, false),
            0x86 => /* add a, (hl)      */ self.add_carry_hl_indirect(false, bus),
            0x87 => /* add a, a         */ self.add_carry(Register::A, false),
            0x88 => /* adc a, b         */ self.add_carry(Register::B, self.flag(Flag::C)),
            0x89 => /* adc a, c         */ self.add_carry(Register::C, self.flag(Flag::C)),
            0x8A => /* adc a, d         */ self.add_carry(Register::D, self.flag(Flag::C)),
            0x8B => /* adc a, e         */ self.add_carry(Register::E, self.flag(Flag::C)),
            0x8C => /* adc a, h         */ self.add_carry(Register::H, self.flag(Flag::C)),
            0x8D => /* adc a, l         */ self.add_carry(Register::L, self.flag(Flag::C)),
            0x8E => /* adc a, (hl)      */ self.add_carry_hl_indirect(self.flag(Flag::C), bus),
            0x8F => /* adc a, a         */ self.add_carry(Register::A, self.flag(Flag::C)),

            0x90 => /* sub b            */ self.sub_carry(Register::B, false),
            0x91 => /* sub c            */ self.sub_carry(Register::C, false),
            0x92 => /* sub d            */ self.sub_carry(Register::D, false),
            0x93 => /* sub e            */ self.sub_carry(Register::E, false),
            0x94 => /* sub h            */ self.sub_carry(Register::H, false),
            0x95 => /* sub l            */ self.sub_carry(Register::L, false),
            0x96 => /* sub (hl)         */ self.sub_carry_hl_indirect(false, bus),
            0x97 => /* sub a            */ self.sub_carry(Register::A, false),
            0x98 => /* sbc a, b         */ self.sub_carry(Register::B, self.flag(Flag::C)),
            0x99 => /* sbc a, c         */ self.sub_carry(Register::C, self.flag(Flag::C)),
            0x9A => /* sbc a, d         */ self.sub_carry(Register::D, self.flag(Flag::C)),
            0x9B => /* sbc a, e         */ self.sub_carry(Register::E, self.flag(Flag::C)),
            0x9C => /* sbc a, h         */ self.sub_carry(Register::H, self.flag(Flag::C)),
            0x9D => /* sbc a, l         */ self.sub_carry(Register::L, self.flag(Flag::C)),
            0x9E => /* sbc a, (hl)      */ self.sub_carry_hl_indirect(self.flag(Flag::C), bus),
            0x9F => /* sbc a, a         */ self.sub_carry(Register::A, self.flag(Flag::C)),

            0xA0 => /* and b            */ self.and(Register::B),
            0xA1 => /* and c            */ self.and(Register::C),
            0xA2 => /* and d            */ self.and(Register::D),
            0xA3 => /* and e            */ self.and(Register::E),
            0xA4 => /* and h            */ self.and(Register::H),
            0xA5 => /* and l            */ self.and(Register::L),
            0xA6 => /* and (hl)         */ self.and_hl_indirect(bus),
            0xA7 => /* and a            */ self.and(Register::A),
            0xA8 => /* xor b            */ self.xor(Register::B),
            0xA9 => /* xor c            */ self.xor(Register::C),
            0xAA => /* xor d            */ self.xor(Register::D),
            0xAB => /* xor e            */ self.xor(Register::E),
            0xAC => /* xor h            */ self.xor(Register::H),
            0xAD => /* xor l            */ self.xor(Register::L),
            0xAE => /* xor (hl)         */ self.xor_hl_indirect(bus),
            0xAF => /* xor a            */ self.xor(Register::A),

            0xB0 => /* or b             */ self.or(Register::B),
            0xB1 => /* or c             */ self.or(Register::C),
            0xB2 => /* or d             */ self.or(Register::D),
            0xB3 => /* or e             */ self.or(Register::E),
            0xB4 => /* or h             */ self.or(Register::H),
            0xB5 => /* or l             */ self.or(Register::L),
            0xB6 => /* or (hl)          */ self.or_hl_indirect(bus),
            0xB7 => /* or a             */ self.or(Register::A),
            0xB8 => /* cp b             */ self.compare(Register::B),
            0xB9 => /* cp c             */ self.compare(Register::C),
            0xBA => /* cp d             */ self.compare(Register::D),
            0xBB => /* cp e             */ self.compare(Register::E),
            0xBC => /* cp h             */ self.compare(Register::H),
            0xBD => /* cp l             */ self.compare(Register::L),
            0xBE => /* cp (hl)          */ self.compare_hl_indirect(bus),
            0xBF => /* cp a             */ self.compare(Register::A),

            0xC0 => /* ret nz           */ self.conditional_return_wz(Condition::NZ, bus),
            0xC1 => /* pop bc           */ self.pop(WideRegister::BC, bus),
            0xC2 => /* jp nz, **        */ self.conditional_jump_wz(Condition::NZ, bus),
            0xC3 => /* jp **            */ self.jump_wz(bus),
            0xC4 => /* call nz, **      */ self.conditional_call_wz(Condition::NZ, bus),
            0xC5 => /* push bc          */ self.push(WideRegister::BC, bus),
            0xC6 => /* add a, *         */ self.add_carry_immediate(false, bus),
            0xC7 => /* rst $00          */ self.reset_wz(0x00, bus),
            0xC8 => /* ret z            */ self.conditional_return_wz(Condition::Z, bus),
            0xC9 => /* ret              */ self.return_wz(bus),
            0xCA => /* jp z, **         */ self.conditional_jump_wz(Condition::Z, bus),
            0xCB => /*                  */ self.cb_prefix(bus),
            0xCC => /* call z, **       */ self.conditional_call_wz(Condition::Z, bus),
            0xCD => /* call **          */ self.call_wz(bus),
            0xCE => /* adc a, *         */ self.add_carry_immediate(self.flag(Flag::C), bus),
            0xCF => /* rst $08          */ self.reset_wz(0x08, bus),

            0xD0 => /* ret nc           */ self.conditional_return_wz(Condition::NC, bus),
            0xD1 => /* pop de           */ self.pop(WideRegister::DE, bus),
            0xD2 => /* jp nc, **        */ self.conditional_jump_wz(Condition::NC, bus),
            0xD3 => /* out (*), a       */ self.output_immediate_indirect(bus),
            0xD4 => /* call nc          */ self.conditional_call_wz(Condition::NC, bus),
            0xD5 => /* push de          */ self.push(WideRegister::DE, bus),
            0xD6 => /* sub *            */ self.sub_carry_immediate(false, bus),
            0xD7 => /* rst $01          */ self.reset_wz(0x10, bus),
            0xD8 => /* ret c            */ self.conditional_return_wz(Condition::C, bus),
            0xD9 => /* exx              */ self.exchange_extra(),
            0xDA => /* jp c             */ self.conditional_jump_wz(Condition::C, bus),
            0xDB => /* in a, (*)        */ self.input_immediate_indirect(bus),
            0xDC => /* call c, **       */ self.conditional_call_wz(Condition::C, bus),
            0xDD => /*                  */ self.dd_prefix(bus),
            0xDE => /* sbc a, *         */ self.sub_carry_immediate(self.flag(Flag::C), bus),
            0xDF => /* rst $18          */ self.reset_wz(0x18, bus),

            0xE0 => /* ret po           */ self.conditional_return_wz(Condition::PO, bus),
            0xE1 => /* pop hl           */ self.pop(WideRegister::HL, bus),
            0xE2 => /* jp po, **        */ self.conditional_jump_wz(Condition::PO, bus),
            0xE3 => /* ex (sp), hl      */ self.exchange_stack_pointer_indirect_wz(WideRegister::HL, bus),
            0xE4 => /* call po, **      */ self.conditional_call_wz(Condition::PO, bus),
            0xE5 => /* push hl          */ self.push(WideRegister::HL, bus),
            0xE6 => /* and *            */ self.and_immediate(bus),
            0xE7 => /* rst $20          */ self.reset_wz(0x20, bus),
            0xE8 => /* ret pe           */ self.conditional_return_wz(Condition::PE, bus),
            0xE9 => /* jp (hl)          */ self.jump_indirect(WideRegister::HL, bus),
            0xEA => /* jp pe, **        */ self.conditional_jump_wz(Condition::PE, bus),
            0xEB => /* ex de, hl        */ self.exchange(WideRegister::DE, WideRegister::HL),
            0xEC => /* call pe, **      */ self.conditional_call_wz(Condition::PE, bus),
            0xED => /*                  */ self.ed_prefix(bus),
            0xEE => /* xor *            */ self.xor_immediate(bus),
            0xEF => /* rst $28          */ self.reset_wz(0x28, bus),

            0xF0 => /* ret p            */ self.conditional_return_wz(Condition::P, bus),
            0xF1 => /* pop af           */ self.pop(WideRegister::AF, bus),
            0xF2 => /* jp p, **         */ self.conditional_jump_wz(Condition::P, bus),
            0xF3 => /* di               */ self.disable_interrupts(),
            0xF4 => /* call p, **       */ self.conditional_call_wz(Condition::P, bus),
            0xF5 => /* push af          */ self.push(WideRegister::AF, bus),
            0xF6 => /* or *             */ self.or_immediate(bus),
            0xF7 => /* rst $30          */ self.reset_wz(0x30, bus),
            0xF8 => /* ret m            */ self.conditional_return_wz(Condition::M, bus),
            0xF9 => /* ld sp, hl        */ self.copy_wide_register(WideRegister::SP, WideRegister::HL),
            0xFA => /* jp m, **         */ self.conditional_jump_wz(Condition::M, bus),
            0xFB => /* ei               */ self.enable_interrupts(),
            0xFC => /* call pe, **      */ self.conditional_call_wz(Condition::PE, bus),
            0xFD => /*                  */ self.fd_prefix(bus),
            0xFE => /* cp *             */ self.compare_immediate(bus),
            0xFF => /* rst $38          */ self.reset_wz(0x38, bus),
        }
    }

    fn cb_prefix(&mut self, bus: &mut impl Bus) -> usize {
        let opcode = self.fetch(bus);
        #[rustfmt::skip]
        (4 + match opcode {
            0x00 => /* rlc b            */ self.rlc_register(Register::B),
            0x01 => /* rlc c            */ self.rlc_register(Register::C),
            0x02 => /* rlc d            */ self.rlc_register(Register::D),
            0x03 => /* rlc e            */ self.rlc_register(Register::E),
            0x04 => /* rlc h            */ self.rlc_register(Register::H),
            0x05 => /* rlc l            */ self.rlc_register(Register::L),
            0x06 => /* rlc (hl)         */ self.rlc_hl_indirect(bus),
            0x07 => /* rlc a            */ self.rlc_register(Register::A),
            0x08 => /* rrc b            */ self.rrc_register(Register::B),
            0x09 => /* rrc c            */ self.rrc_register(Register::C),
            0x0A => /* rrc d            */ self.rrc_register(Register::D),
            0x0B => /* rrc e            */ self.rrc_register(Register::E),
            0x0C => /* rrc h            */ self.rrc_register(Register::H),
            0x0D => /* rrc l            */ self.rrc_register(Register::L),
            0x0E => /* rrc (hl)         */ self.rrc_hl_indirect(bus),
            0x0F => /* rrc a            */ self.rrc_register(Register::A),

            0x10 => /* rl b             */ self.rl_register(Register::B),
            0x11 => /* rl c             */ self.rl_register(Register::C),
            0x12 => /* rl d             */ self.rl_register(Register::D),
            0x13 => /* rl e             */ self.rl_register(Register::E),
            0x14 => /* rl h             */ self.rl_register(Register::H),
            0x15 => /* rl l             */ self.rl_register(Register::L),
            0x16 => /* rl (hl)          */ self.rl_hl_indirect(bus),
            0x17 => /* rl a             */ self.rl_register(Register::A),
            0x18 => /* rr b             */ self.rr_register(Register::B),
            0x19 => /* rr c             */ self.rr_register(Register::C),
            0x1A => /* rr d             */ self.rr_register(Register::D),
            0x1B => /* rr e             */ self.rr_register(Register::E),
            0x1C => /* rr h             */ self.rr_register(Register::H),
            0x1D => /* rr l             */ self.rr_register(Register::L),
            0x1E => /* rr (hl)          */ self.rr_hl_indirect(bus),
            0x1F => /* rr a             */ self.rr_register(Register::A),

            0x20 => /* sla b            */ self.sla_register(Register::B),
            0x21 => /* sla c            */ self.sla_register(Register::C),
            0x22 => /* sla d            */ self.sla_register(Register::D),
            0x23 => /* sla e            */ self.sla_register(Register::E),
            0x24 => /* sla h            */ self.sla_register(Register::H),
            0x25 => /* sla l            */ self.sla_register(Register::L),
            0x26 => /* sla (hl)         */ self.sla_hl_indirect(bus),
            0x27 => /* sla a            */ self.sla_register(Register::A),
            0x28 => /* sra b            */ self.sra_register(Register::B),
            0x29 => /* sra c            */ self.sra_register(Register::C),
            0x2A => /* sra d            */ self.sra_register(Register::D),
            0x2B => /* sra e            */ self.sra_register(Register::E),
            0x2C => /* sra h            */ self.sra_register(Register::H),
            0x2D => /* sra l            */ self.sra_register(Register::L),
            0x2E => /* sra (hl)         */ self.sra_hl_indirect(bus),
            0x2F => /* sra a            */ self.sra_register(Register::A),

            0x30 => /* sll b            */ self.sll_register(Register::B),
            0x31 => /* sll c            */ self.sll_register(Register::C),
            0x32 => /* sll d            */ self.sll_register(Register::D),
            0x33 => /* sll e            */ self.sll_register(Register::E),
            0x34 => /* sll h            */ self.sll_register(Register::H),
            0x35 => /* sll l            */ self.sll_register(Register::L),
            0x36 => /* sll (hl)         */ self.sll_hl_indirect(bus),
            0x37 => /* sll a            */ self.sll_register(Register::A),
            0x38 => /* srl b            */ self.srl_register(Register::B),
            0x39 => /* srl c            */ self.srl_register(Register::C),
            0x3A => /* srl d            */ self.srl_register(Register::D),
            0x3B => /* srl e            */ self.srl_register(Register::E),
            0x3C => /* srl h            */ self.srl_register(Register::H),
            0x3D => /* srl l            */ self.srl_register(Register::L),
            0x3E => /* srl (hl)         */ self.srl_hl_indirect(bus),
            0x3F => /* srl a            */ self.srl_register(Register::A),

            0x40 => /* bit 0, b         */ self.bit_register(0x01, Register::B),
            0x41 => /* bit 0, c         */ self.bit_register(0x01, Register::C),
            0x42 => /* bit 0, d         */ self.bit_register(0x01, Register::D),
            0x43 => /* bit 0, e         */ self.bit_register(0x01, Register::E),
            0x44 => /* bit 0, h         */ self.bit_register(0x01, Register::H),
            0x45 => /* bit 0, l         */ self.bit_register(0x01, Register::L),
            0x46 => /* bit 0, (hl)      */ self.bit_hl_indirect_wz(0x01, bus),
            0x47 => /* bit 0, a         */ self.bit_register(0x01, Register::A),
            0x48 => /* bit 1, b         */ self.bit_register(0x02, Register::B),
            0x49 => /* bit 1, c         */ self.bit_register(0x02, Register::C),
            0x4A => /* bit 1, d         */ self.bit_register(0x02, Register::D),
            0x4B => /* bit 1, e         */ self.bit_register(0x02, Register::E),
            0x4C => /* bit 1, h         */ self.bit_register(0x02, Register::H),
            0x4D => /* bit 1, l         */ self.bit_register(0x02, Register::L),
            0x4E => /* bit 1, (hl)      */ self.bit_hl_indirect_wz(0x02, bus),
            0x4F => /* bit 1, a         */ self.bit_register(0x02, Register::A),

            0x50 => /* bit 2, b         */ self.bit_register(0x04, Register::B),
            0x51 => /* bit 2, c         */ self.bit_register(0x04, Register::C),
            0x52 => /* bit 2, d         */ self.bit_register(0x04, Register::D),
            0x53 => /* bit 2, e         */ self.bit_register(0x04, Register::E),
            0x54 => /* bit 2, h         */ self.bit_register(0x04, Register::H),
            0x55 => /* bit 2, l         */ self.bit_register(0x04, Register::L),
            0x56 => /* bit 2, (hl)      */ self.bit_hl_indirect_wz(0x04, bus),
            0x57 => /* bit 2, a         */ self.bit_register(0x04, Register::A),
            0x58 => /* bit 3, b         */ self.bit_register(0x08, Register::B),
            0x59 => /* bit 3, c         */ self.bit_register(0x08, Register::C),
            0x5A => /* bit 3, d         */ self.bit_register(0x08, Register::D),
            0x5B => /* bit 3, e         */ self.bit_register(0x08, Register::E),
            0x5C => /* bit 3, h         */ self.bit_register(0x08, Register::H),
            0x5D => /* bit 3, l         */ self.bit_register(0x08, Register::L),
            0x5E => /* bit 3, (hl)      */ self.bit_hl_indirect_wz(0x08, bus),
            0x5F => /* bit 3, a         */ self.bit_register(0x08, Register::A),

            0x60 => /* bit 4, b         */ self.bit_register(0x10, Register::B),
            0x61 => /* bit 4, c         */ self.bit_register(0x10, Register::C),
            0x62 => /* bit 4, d         */ self.bit_register(0x10, Register::D),
            0x63 => /* bit 4, e         */ self.bit_register(0x10, Register::E),
            0x64 => /* bit 4, h         */ self.bit_register(0x10, Register::H),
            0x65 => /* bit 4, l         */ self.bit_register(0x10, Register::L),
            0x66 => /* bit 4, (hl)      */ self.bit_hl_indirect_wz(0x10, bus),
            0x67 => /* bit 4, a         */ self.bit_register(0x10, Register::A),
            0x68 => /* bit 5, b         */ self.bit_register(0x20, Register::B),
            0x69 => /* bit 5, c         */ self.bit_register(0x20, Register::C),
            0x6A => /* bit 5, d         */ self.bit_register(0x20, Register::D),
            0x6B => /* bit 5, e         */ self.bit_register(0x20, Register::E),
            0x6C => /* bit 5, h         */ self.bit_register(0x20, Register::H),
            0x6D => /* bit 5, l         */ self.bit_register(0x20, Register::L),
            0x6E => /* bit 5, (hl)      */ self.bit_hl_indirect_wz(0x20, bus),
            0x6F => /* bit 5, a         */ self.bit_register(0x20, Register::A),

            0x70 => /* bit 6, b         */ self.bit_register(0x40, Register::B),
            0x71 => /* bit 6, c         */ self.bit_register(0x40, Register::C),
            0x72 => /* bit 6, d         */ self.bit_register(0x40, Register::D),
            0x73 => /* bit 6, e         */ self.bit_register(0x40, Register::E),
            0x74 => /* bit 6, h         */ self.bit_register(0x40, Register::H),
            0x75 => /* bit 6, l         */ self.bit_register(0x40, Register::L),
            0x76 => /* bit 6, (hl)      */ self.bit_hl_indirect_wz(0x40, bus),
            0x77 => /* bit 6, a         */ self.bit_register(0x40, Register::A),
            0x78 => /* bit 7, b         */ self.bit_register(0x80, Register::B),
            0x79 => /* bit 7, c         */ self.bit_register(0x80, Register::C),
            0x7A => /* bit 7, d         */ self.bit_register(0x80, Register::D),
            0x7B => /* bit 7, e         */ self.bit_register(0x80, Register::E),
            0x7C => /* bit 7, h         */ self.bit_register(0x80, Register::H),
            0x7D => /* bit 7, l         */ self.bit_register(0x80, Register::L),
            0x7E => /* bit 7, (hl)      */ self.bit_hl_indirect_wz(0x80, bus),
            0x7F => /* bit 7, a         */ self.bit_register(0x80, Register::A),

            0x80 => /* res 0, b         */ self.reset_bit_register(0x01, Register::B),
            0x81 => /* res 0, c         */ self.reset_bit_register(0x01, Register::C),
            0x82 => /* res 0, d         */ self.reset_bit_register(0x01, Register::D),
            0x83 => /* res 0, e         */ self.reset_bit_register(0x01, Register::E),
            0x84 => /* res 0, h         */ self.reset_bit_register(0x01, Register::H),
            0x85 => /* res 0, l         */ self.reset_bit_register(0x01, Register::L),
            0x86 => /* res 0, (hl)      */ self.reset_bit_hl_indirect(0x01, bus),
            0x87 => /* res 0, a         */ self.reset_bit_register(0x01, Register::A),
            0x88 => /* res 1, b         */ self.reset_bit_register(0x02, Register::B),
            0x89 => /* res 1, c         */ self.reset_bit_register(0x02, Register::C),
            0x8A => /* res 1, d         */ self.reset_bit_register(0x02, Register::D),
            0x8B => /* res 1, e         */ self.reset_bit_register(0x02, Register::E),
            0x8C => /* res 1, h         */ self.reset_bit_register(0x02, Register::H),
            0x8D => /* res 1, l         */ self.reset_bit_register(0x02, Register::L),
            0x8E => /* res 1, (hl)      */ self.reset_bit_hl_indirect(0x02, bus),
            0x8F => /* res 1, a         */ self.reset_bit_register(0x02, Register::A),

            0x90 => /* res 1, b         */ self.reset_bit_register(0x04, Register::B),
            0x91 => /* res 1, c         */ self.reset_bit_register(0x04, Register::C),
            0x92 => /* res 1, d         */ self.reset_bit_register(0x04, Register::D),
            0x93 => /* res 1, e         */ self.reset_bit_register(0x04, Register::E),
            0x94 => /* res 1, h         */ self.reset_bit_register(0x04, Register::H),
            0x95 => /* res 1, l         */ self.reset_bit_register(0x04, Register::L),
            0x96 => /* res 1, (hl)      */ self.reset_bit_hl_indirect(0x04, bus),
            0x97 => /* res 1, a         */ self.reset_bit_register(0x04, Register::A),
            0x98 => /* res 3, b         */ self.reset_bit_register(0x08, Register::B),
            0x99 => /* res 3, c         */ self.reset_bit_register(0x08, Register::C),
            0x9A => /* res 3, d         */ self.reset_bit_register(0x08, Register::D),
            0x9B => /* res 3, e         */ self.reset_bit_register(0x08, Register::E),
            0x9C => /* res 3, h         */ self.reset_bit_register(0x08, Register::H),
            0x9D => /* res 3, l         */ self.reset_bit_register(0x08, Register::L),
            0x9E => /* res 3, (hl)      */ self.reset_bit_hl_indirect(0x08, bus),
            0x9F => /* res 3, a         */ self.reset_bit_register(0x08, Register::A),

            0xA0 => /* res 4, b         */ self.reset_bit_register(0x10, Register::B),
            0xA1 => /* res 4, c         */ self.reset_bit_register(0x10, Register::C),
            0xA2 => /* res 4, d         */ self.reset_bit_register(0x10, Register::D),
            0xA3 => /* res 4, e         */ self.reset_bit_register(0x10, Register::E),
            0xA4 => /* res 4, h         */ self.reset_bit_register(0x10, Register::H),
            0xA5 => /* res 4, l         */ self.reset_bit_register(0x10, Register::L),
            0xA6 => /* res 4, (hl)      */ self.reset_bit_hl_indirect(0x10, bus),
            0xA7 => /* res 4, a         */ self.reset_bit_register(0x10, Register::A),
            0xA8 => /* res 5, b         */ self.reset_bit_register(0x20, Register::B),
            0xA9 => /* res 5, c         */ self.reset_bit_register(0x20, Register::C),
            0xAA => /* res 5, d         */ self.reset_bit_register(0x20, Register::D),
            0xAB => /* res 5, e         */ self.reset_bit_register(0x20, Register::E),
            0xAC => /* res 5, h         */ self.reset_bit_register(0x20, Register::H),
            0xAD => /* res 5, l         */ self.reset_bit_register(0x20, Register::L),
            0xAE => /* res 5, (hl)      */ self.reset_bit_hl_indirect(0x20, bus),
            0xAF => /* res 5, a         */ self.reset_bit_register(0x20, Register::A),

            0xB0 => /* res 6, b         */ self.reset_bit_register(0x40, Register::B),
            0xB1 => /* res 6, c         */ self.reset_bit_register(0x40, Register::C),
            0xB2 => /* res 6, d         */ self.reset_bit_register(0x40, Register::D),
            0xB3 => /* res 6, e         */ self.reset_bit_register(0x40, Register::E),
            0xB4 => /* res 6, h         */ self.reset_bit_register(0x40, Register::H),
            0xB5 => /* res 6, l         */ self.reset_bit_register(0x40, Register::L),
            0xB6 => /* res 6, (hl)      */ self.reset_bit_hl_indirect(0x40, bus),
            0xB7 => /* res 6, a         */ self.reset_bit_register(0x40, Register::A),
            0xB8 => /* res 7, b         */ self.reset_bit_register(0x80, Register::B),
            0xB9 => /* res 7, c         */ self.reset_bit_register(0x80, Register::C),
            0xBA => /* res 7, d         */ self.reset_bit_register(0x80, Register::D),
            0xBB => /* res 7, e         */ self.reset_bit_register(0x80, Register::E),
            0xBC => /* res 7, h         */ self.reset_bit_register(0x80, Register::H),
            0xBD => /* res 7, l         */ self.reset_bit_register(0x80, Register::L),
            0xBE => /* res 7, (hl)      */ self.reset_bit_hl_indirect(0x80, bus),
            0xBF => /* res 7, a         */ self.reset_bit_register(0x80, Register::A),

            0xC0 => /* set 0, b         */ self.set_bit_register(0x01, Register::B),
            0xC1 => /* set 0, c         */ self.set_bit_register(0x01, Register::C),
            0xC2 => /* set 0, d         */ self.set_bit_register(0x01, Register::D),
            0xC3 => /* set 0, e         */ self.set_bit_register(0x01, Register::E),
            0xC4 => /* set 0, h         */ self.set_bit_register(0x01, Register::H),
            0xC5 => /* set 0, l         */ self.set_bit_register(0x01, Register::L),
            0xC6 => /* set 0, (hl)      */ self.set_bit_hl_indirect(0x01, bus),
            0xC7 => /* set 0, a         */ self.set_bit_register(0x01, Register::A),
            0xC8 => /* set 1, b         */ self.set_bit_register(0x02, Register::B),
            0xC9 => /* set 1, c         */ self.set_bit_register(0x02, Register::C),
            0xCA => /* set 1, d         */ self.set_bit_register(0x02, Register::D),
            0xCB => /* set 1, e         */ self.set_bit_register(0x02, Register::E),
            0xCC => /* set 1, h         */ self.set_bit_register(0x02, Register::H),
            0xCD => /* set 1, l         */ self.set_bit_register(0x02, Register::L),
            0xCE => /* set 1, (hl)      */ self.set_bit_hl_indirect(0x02, bus),
            0xCF => /* set 1, a         */ self.set_bit_register(0x02, Register::A),

            0xD0 => /* set 2, b         */ self.set_bit_register(0x04, Register::B),
            0xD1 => /* set 2, c         */ self.set_bit_register(0x04, Register::C),
            0xD2 => /* set 2, d         */ self.set_bit_register(0x04, Register::D),
            0xD3 => /* set 2, e         */ self.set_bit_register(0x04, Register::E),
            0xD4 => /* set 2, h         */ self.set_bit_register(0x04, Register::H),
            0xD5 => /* set 2, l         */ self.set_bit_register(0x04, Register::L),
            0xD6 => /* set 2, (hl)      */ self.set_bit_hl_indirect(0x04, bus),
            0xD7 => /* set 2, a         */ self.set_bit_register(0x04, Register::A),
            0xD8 => /* set 3, b         */ self.set_bit_register(0x08, Register::B),
            0xD9 => /* set 3, c         */ self.set_bit_register(0x08, Register::C),
            0xDA => /* set 3, d         */ self.set_bit_register(0x08, Register::D),
            0xDB => /* set 3, e         */ self.set_bit_register(0x08, Register::E),
            0xDC => /* set 3, h         */ self.set_bit_register(0x08, Register::H),
            0xDD => /* set 3, l         */ self.set_bit_register(0x08, Register::L),
            0xDE => /* set 3, (hl)      */ self.set_bit_hl_indirect(0x08, bus),
            0xDF => /* set 3, a         */ self.set_bit_register(0x08, Register::A),

            0xE0 => /* set 4, b         */ self.set_bit_register(0x10, Register::B),
            0xE1 => /* set 4, c         */ self.set_bit_register(0x10, Register::C),
            0xE2 => /* set 4, d         */ self.set_bit_register(0x10, Register::D),
            0xE3 => /* set 4, e         */ self.set_bit_register(0x10, Register::E),
            0xE4 => /* set 4, h         */ self.set_bit_register(0x10, Register::H),
            0xE5 => /* set 4, l         */ self.set_bit_register(0x10, Register::L),
            0xE6 => /* set 4, (hl)      */ self.set_bit_hl_indirect(0x10, bus),
            0xE7 => /* set 4, a         */ self.set_bit_register(0x10, Register::A),
            0xE8 => /* set 5, b         */ self.set_bit_register(0x20, Register::B),
            0xE9 => /* set 5, c         */ self.set_bit_register(0x20, Register::C),
            0xEA => /* set 5, d         */ self.set_bit_register(0x20, Register::D),
            0xEB => /* set 5, e         */ self.set_bit_register(0x20, Register::E),
            0xEC => /* set 5, h         */ self.set_bit_register(0x20, Register::H),
            0xED => /* set 5, l         */ self.set_bit_register(0x20, Register::L),
            0xEE => /* set 5, (hl)      */ self.set_bit_hl_indirect(0x20, bus),
            0xEF => /* set 5, a         */ self.set_bit_register(0x20, Register::A),

            0xF0 => /* set 6, b         */ self.set_bit_register(0x40, Register::B),
            0xF1 => /* set 6, c         */ self.set_bit_register(0x40, Register::C),
            0xF2 => /* set 6, d         */ self.set_bit_register(0x40, Register::D),
            0xF3 => /* set 6, e         */ self.set_bit_register(0x40, Register::E),
            0xF4 => /* set 6, h         */ self.set_bit_register(0x40, Register::H),
            0xF5 => /* set 6, l         */ self.set_bit_register(0x40, Register::L),
            0xF6 => /* set 6, (hl)      */ self.set_bit_hl_indirect(0x40, bus),
            0xF7 => /* set 6, a         */ self.set_bit_register(0x40, Register::A),
            0xF8 => /* set 7, b         */ self.set_bit_register(0x80, Register::B),
            0xF9 => /* set 7, c         */ self.set_bit_register(0x80, Register::C),
            0xFA => /* set 7, d         */ self.set_bit_register(0x80, Register::D),
            0xFB => /* set 7, e         */ self.set_bit_register(0x80, Register::E),
            0xFC => /* set 7, h         */ self.set_bit_register(0x80, Register::H),
            0xFD => /* set 7, l         */ self.set_bit_register(0x80, Register::L),
            0xFE => /* set 7, (hl)      */ self.set_bit_hl_indirect(0x80, bus),
            0xFF => /* set 7, a         */ self.set_bit_register(0x80, Register::A),
        })
    }

    fn dd_prefix(&mut self, bus: &mut impl Bus) -> usize {
        let opcode = self.fetch(bus);
        #[rustfmt::skip]
        (4 + match opcode {
            0x09 => /* add ix, bc       */ self.add_wide_wz(WideRegister::IX, WideRegister::BC),

            0x19 => /* add ix, de       */ self.add_wide_wz(WideRegister::IX, WideRegister::DE),

            0x21 => /* ld ix, **        */ self.read_wide_immediate(WideRegister::IX, bus),
            0x22 => /* ld (**), ix      */ self.write_wide_absolute_wz(WideRegister::IX, bus),
            0x23 => /* inc ix           */ self.inc_wide(WideRegister::IX),
            0x24 => /* inc ixh          */ self.inc_wz(Register::IXH),
            0x25 => /* dec ixh          */ self.dec_wz(Register::IXH),
            0x26 => /* ld ixh, *        */ self.read_immediate(Register::IXH, bus),
            0x29 => /* add ix, ix       */ self.add_wide_wz(WideRegister::IX, WideRegister::IX),
            0x2A => /* ld ix, (**)      */ self.read_wide_absolute_wz(WideRegister::IX, bus),
            0x2B => /* dec ix           */ self.dec_wide(WideRegister::IX),
            0x2C => /* inc ixl          */ self.inc_wz(Register::IXL),
            0x2D => /* dec ixl          */ self.dec_wz(Register::IXL),
            0x2E => /* ld ixl, *        */ self.read_immediate(Register::IXL, bus),

            0x34 => /* inc (ix+*)       */ self.inc_index_indirect_wz(WideRegister::IX, bus),
            0x35 => /* dec (ix+*)       */ self.dec_index_indirect_wz(WideRegister::IX, bus),
            0x36 => /* ld (ix+*), *     */ self.write_immediate_index_indirect_wz(WideRegister::IX, bus),
            0x39 => /* add ix, sp       */ self.add_wide_wz(WideRegister::IX, WideRegister::SP),

            0x44 => /* ld b, ixh        */ self.copy_register(Register::B, Register::IXH),
            0x45 => /* ld b, ixl        */ self.copy_register(Register::B, Register::IXL),
            0x46 => /* ld b, (ix+*)     */ self.read_index_indirect_wz(Register::B, WideRegister::IX, bus),
            0x4C => /* ld c, ixh        */ self.copy_register(Register::C, Register::IXH),
            0x4D => /* ld c, ixl        */ self.copy_register(Register::C, Register::IXL),
            0x4E => /* ld c, (ix+*)     */ self.read_index_indirect_wz(Register::C, WideRegister::IX, bus),

            0x54 => /* ld d, ixh        */ self.copy_register(Register::D, Register::IXH),
            0x55 => /* ld d, ixl        */ self.copy_register(Register::D, Register::IXL),
            0x56 => /* ld d, (ix+*)     */ self.read_index_indirect_wz(Register::D, WideRegister::IX, bus),
            0x5C => /* ld e, ixh        */ self.copy_register(Register::E, Register::IXH),
            0x5D => /* ld e, ixl        */ self.copy_register(Register::E, Register::IXL),
            0x5E => /* ld e, (ix+*)     */ self.read_index_indirect_wz(Register::E, WideRegister::IX, bus),

            0x60 => /* ld ixh, b        */ self.copy_register(Register::IXH, Register::B),
            0x61 => /* ld ixh, c        */ self.copy_register(Register::IXH, Register::C),
            0x62 => /* ld ixh, d        */ self.copy_register(Register::IXH, Register::D),
            0x63 => /* ld ixh, e        */ self.copy_register(Register::IXH, Register::E),
            0x64 => /* ld ixh, ixh      */ self.copy_register(Register::IXH, Register::IXH),
            0x65 => /* ld ixh, ixl      */ self.copy_register(Register::IXH, Register::IXL),
            0x66 => /* ld h, (ix+*)     */ self.read_index_indirect_wz(Register::H, WideRegister::IX, bus),
            0x67 => /* ld ixh, a        */ self.copy_register(Register::IXH, Register::A),
            0x68 => /* ld ixl, b        */ self.copy_register(Register::IXL, Register::B),
            0x69 => /* ld ixl, c        */ self.copy_register(Register::IXL, Register::C),
            0x6A => /* ld ixl, d        */ self.copy_register(Register::IXL, Register::D),
            0x6B => /* ld ixl, e        */ self.copy_register(Register::IXL, Register::E),
            0x6C => /* ld ixl, ixh      */ self.copy_register(Register::IXL, Register::IXH),
            0x6D => /* ld ixl, ixl      */ self.copy_register(Register::IXL, Register::IXL),
            0x6E => /* ld l, (ix+*)     */ self.read_index_indirect_wz(Register::L, WideRegister::IX, bus),
            0x6F => /* ld ixl, a        */ self.copy_register(Register::IXL, Register::A),

            0x70 => /* ld (ix+*), b     */ self.write_index_indirect_wz(WideRegister::IX, Register::B, bus),
            0x71 => /* ld (ix+*), c     */ self.write_index_indirect_wz(WideRegister::IX, Register::C, bus),
            0x72 => /* ld (ix+*), d     */ self.write_index_indirect_wz(WideRegister::IX, Register::D, bus),
            0x73 => /* ld (ix+*), e     */ self.write_index_indirect_wz(WideRegister::IX, Register::E, bus),
            0x74 => /* ld (ix+*), h     */ self.write_index_indirect_wz(WideRegister::IX, Register::H, bus),
            0x75 => /* ld (ix+*), l     */ self.write_index_indirect_wz(WideRegister::IX, Register::L, bus),
            0x77 => /* ld (ix+*), a     */ self.write_index_indirect_wz(WideRegister::IX, Register::A, bus),
            0x7C => /* ld a, ixh        */ self.copy_register(Register::A, Register::IXH),
            0x7D => /* ld a, ixl        */ self.copy_register(Register::A, Register::IXL),
            0x7E => /* ld a, (ix+*)     */ self.read_index_indirect_wz(Register::A, WideRegister::IX, bus),

            0x84 => /* add a, ixh       */ self.add_carry(Register::IXH, false),
            0x85 => /* add a, ixl       */ self.add_carry(Register::IXL, false),
            0x86 => /* add a, (ix+*)    */ self.add_carry_index_indirect_wz(WideRegister::IX, false, bus),
            0x8C => /* adc a, ixh       */ self.add_carry(Register::IXH, self.flag(Flag::C)),
            0x8D => /* adc a, ixl       */ self.add_carry(Register::IXL, self.flag(Flag::C)),
            0x8E => /* adc a, (ix+*)    */ self.add_carry_index_indirect_wz(WideRegister::IX, self.flag(Flag::C), bus),

            0x94 => /* sub ixh          */ self.sub_carry(Register::IXH, false),
            0x95 => /* sub ixl          */ self.sub_carry(Register::IXL, false),
            0x96 => /* sub (ix+*)       */ self.sub_carry_index_indirect_wz(WideRegister::IX, false, bus),
            0x9C => /* sbc a, ixh       */ self.sub_carry(Register::IXH, self.flag(Flag::C)),
            0x9D => /* sbc a, ixl       */ self.sub_carry(Register::IXL, self.flag(Flag::C)),
            0x9E => /* sbc a, (ix+*)    */ self.sub_carry_index_indirect_wz(WideRegister::IX, self.flag(Flag::C), bus),

            0xA4 => /* and ixh          */ self.and(Register::IXH),
            0xA5 => /* and ixl          */ self.and(Register::IXL),
            0xA6 => /* and (ix+*)       */ self.and_index_indirect_wz(WideRegister::IX, bus),
            0xAC => /* xor ixh          */ self.xor(Register::IXH),
            0xAD => /* xor ixl          */ self.xor(Register::IXL),
            0xAE => /* xor (ix+*)       */ self.xor_index_indirect_wz(WideRegister::IX, bus),

            0xB4 => /* or ixh           */ self.or(Register::IXH),
            0xB5 => /* or ixl           */ self.or(Register::IXL),
            0xB6 => /* or (ix+*)        */ self.or_index_indirect_wz(WideRegister::IX, bus),
            0xBC => /* cp ixh           */ self.compare(Register::IXH),
            0xBD => /* cp ixl           */ self.compare(Register::IXL),
            0xBE => /* cp (ix+*)        */ self.compare_index_indirect_wz(WideRegister::IX, bus),

            0xCB => /*                  */ self.ddcb_prefix(bus),

            0xE1 => /* pop ix           */ self.pop(WideRegister::IX, bus),
            0xE3 => /* ex (sp), ix      */ self.exchange_stack_pointer_indirect_wz(WideRegister::IX, bus),
            0xE5 => /* push ix          */ self.push(WideRegister::IX, bus),
            0xE9 => /* jp (ix)          */ self.jump_indirect(WideRegister::IX, bus),

            0xF9 => /* ld sp, ix        */ self.copy_wide_register(WideRegister::SP, WideRegister::IX),

            // Any other opcode seems to act as if the prefix was a nop
            _ => self.step(bus),
        })
    }

    fn ed_prefix(&mut self, bus: &mut impl Bus) -> usize {
        let opcode = self.fetch(bus);
        #[rustfmt::skip]
        (4 + match opcode {
            0x40 => /* in b, (c)        */ self.input(Register::B, bus),
            0x41 => /* out (c), b       */ self.output(Register::B, bus),
            0x42 => /* sbc hl, bc       */ self.sub_carry_wide_wz(WideRegister::HL, WideRegister::BC),
            0x43 => /* ld (**), bc      */ self.write_wide_absolute_wz(WideRegister::BC, bus),
            0x44 => /* neg              */ self.neg(),
            0x45 => /* retn             */ self.retn(bus),
            0x46 => /* im 0             */ self.set_interrupt_mode(InterruptMode::Zero),
            0x47 => /* ld i, a          */ self.copy_register(Register::I, Register::A),
            0x48 => /* in c, (c)        */ self.input(Register::C, bus),
            0x49 => /* out (c), c       */ self.output(Register::C, bus),
            0x4A => /* adc hl, bc       */ self.add_carry_wide_wz(WideRegister::HL, WideRegister::BC),
            0x4B => /* ld bc, (**)      */ self.read_wide_absolute_wz(WideRegister::BC, bus),
            0x4C => /* neg              */ self.neg(),
            0x4D => /* reti             */ self.reti_wz(bus),
            0x4E => /* im 0/1           */ self.set_interrupt_mode(InterruptMode::Zero),
            0x4F => /* ld r, a          */ self.copy_register(Register::R, Register::A),

            0x50 => /* in d, (c)        */ self.input(Register::D, bus),
            0x51 => /* out (c), d       */ self.output(Register::D, bus),
            0x52 => /* sbc hl, de       */ self.sub_carry_wide_wz(WideRegister::HL, WideRegister::DE),
            0x53 => /* ld (**), de      */ self.write_wide_absolute_wz(WideRegister::DE, bus),
            0x54 => /* neg              */ self.neg(),
            0x55 => /* retn             */ self.retn(bus),
            0x56 => /* im 1             */ self.set_interrupt_mode(InterruptMode::One),
            0x57 => /* ld a, i          */ self.copy_ir_register(Register::I),
            0x58 => /* in e, (c)        */ self.input(Register::E, bus),
            0x59 => /* out (c), e       */ self.output(Register::E, bus),
            0x5A => /* adc hl, de       */ self.add_carry_wide_wz(WideRegister::HL, WideRegister::DE),
            0x5B => /* ld de, (**)      */ self.read_wide_absolute_wz(WideRegister::DE, bus),
            0x5C => /* neg              */ self.neg(),
            0x5D => /* retn             */ self.retn(bus),
            0x5E => /* im 2             */ self.set_interrupt_mode(InterruptMode::Two),
            0x5F => /* ld a, r          */ self.copy_ir_register(Register::R),

            0x60 => /* in h, (c)        */ self.input(Register::H, bus),
            0x61 => /* out (c), h       */ self.output(Register::H, bus),
            0x62 => /* sbc hl, hl       */ self.sub_carry_wide_wz(WideRegister::HL, WideRegister::HL),
            0x63 => /* ld (**), hl      */ self.write_wide_absolute_wz(WideRegister::HL, bus),
            0x64 => /* neg              */ self.neg(),
            0x65 => /* retn             */ self.retn(bus),
            0x66 => /* im 0             */ self.set_interrupt_mode(InterruptMode::Zero),
            0x67 => /* rrd              */ self.rrd_wz(bus),
            0x68 => /* in l, (c)        */ self.input(Register::L, bus),
            0x69 => /* out (c), l       */ self.output(Register::L, bus),
            0x6A => /* adc hl, de       */ self.add_carry_wide_wz(WideRegister::HL, WideRegister::HL),
            0x6B => /* ld hl, (**)      */ self.read_wide_absolute_wz(WideRegister::HL, bus),
            0x6C => /* neg              */ self.neg(),
            0x6D => /* retn             */ self.retn(bus),
            0x6E => /* im 0/1           */ self.set_interrupt_mode(InterruptMode::Zero),
            0x6F => /* rld              */ self.rld_wz(bus),

            0x70 => /* in (c)           */ self.input_and_drop(bus),
            0x71 => /* out (c), 0       */ self.output_zero(bus),
            0x72 => /* sbc hl, sp       */ self.sub_carry_wide_wz(WideRegister::HL, WideRegister::SP),
            0x73 => /* ld (**), sp      */ self.write_wide_absolute_wz(WideRegister::SP, bus),
            0x74 => /* neg              */ self.neg(),
            0x75 => /* retn             */ self.retn(bus),
            0x76 => /* im 1             */ self.set_interrupt_mode(InterruptMode::One),
            0x78 => /* in a, (c)        */ self.input(Register::A, bus),
            0x79 => /* out (c), a       */ self.output(Register::A, bus),
            0x7A => /* adc hl, sp       */ self.add_carry_wide_wz(WideRegister::HL, WideRegister::SP),
            0x7B => /* ld sp, (**)      */ self.read_wide_absolute_wz(WideRegister::SP, bus),
            0x7C => /* neg              */ self.neg(),
            0x7D => /* retn             */ self.retn(bus),
            0x7E => /* im 2             */ self.set_interrupt_mode(InterruptMode::Two),

            0xA0 => /* ldi              */ self.ldi(bus),
            0xA1 => /* cpi              */ self.cpi_wz(bus),
            0xA2 => /* ini              */ self.ini(bus),
            0xA3 => /* outi             */ self.outi(bus),
            0xA8 => /* ldd              */ self.ldd(bus),
            0xA9 => /* cpd              */ self.cpd_wz(bus),
            0xAA => /* ind              */ self.ind(bus),
            0xAB => /* outd             */ self.outd(bus),

            0xB0 => /* ldir             */ self.ldir(bus),
            0xB1 => /* cpir             */ self.cpir(bus),
            0xB2 => /* inir             */ self.inir(bus),
            0xB3 => /* otir             */ self.otir(bus),
            0xB8 => /* lddr             */ self.lddr(bus),
            0xB9 => /* cpdr             */ self.cpdr_wz(bus),
            0xBA => /* indr             */ self.indr(bus),
            0xBB => /* otdr             */ self.otdr(bus),

            // Any other opcode seems to act as if the prefix was a nop
            _ => self.step(bus),
        })
    }

    fn fd_prefix(&mut self, bus: &mut impl Bus) -> usize {
        let opcode = self.fetch(bus);
        #[rustfmt::skip]
        (4 + match opcode {
            0x09 => /* add iy, bc       */ self.add_wide_wz(WideRegister::IY, WideRegister::BC),

            0x19 => /* add iy, de       */ self.add_wide_wz(WideRegister::IY, WideRegister::DE),

            0x21 => /* ld iy, **        */ self.read_wide_immediate(WideRegister::IY, bus),
            0x22 => /* ld (**), iy      */ self.write_wide_absolute_wz(WideRegister::IY, bus),
            0x23 => /* inc iy           */ self.inc_wide(WideRegister::IY),
            0x24 => /* inc iyh          */ self.inc_wz(Register::IYH),
            0x25 => /* dec iyh          */ self.dec_wz(Register::IYH),
            0x26 => /* ld iyh, *        */ self.read_immediate(Register::IYH, bus),
            0x29 => /* add iy, iy       */ self.add_wide_wz(WideRegister::IY, WideRegister::IY),
            0x2A => /* ld iy, (**)      */ self.read_wide_absolute_wz(WideRegister::IY, bus),
            0x2B => /* dec iy           */ self.dec_wide(WideRegister::IY),
            0x2C => /* inc iyl          */ self.inc_wz(Register::IYL),
            0x2D => /* dec iyl          */ self.dec_wz(Register::IYL),
            0x2E => /* ld iyl, *        */ self.read_immediate(Register::IYL, bus),

            0x34 => /* inc (iy+*)       */ self.inc_index_indirect_wz(WideRegister::IY, bus),
            0x35 => /* dec (iy+*)       */ self.dec_index_indirect_wz(WideRegister::IY, bus),
            0x36 => /* ld (iy+*), *     */ self.write_immediate_index_indirect_wz(WideRegister::IY, bus),
            0x39 => /* add iy, sp       */ self.add_wide_wz(WideRegister::IY, WideRegister::SP),

            0x44 => /* ld b, iyh        */ self.copy_register(Register::B, Register::IYH),
            0x45 => /* ld b, iyl        */ self.copy_register(Register::B, Register::IYL),
            0x46 => /* ld b, (iy+*)     */ self.read_index_indirect_wz(Register::B, WideRegister::IY, bus),
            0x4C => /* ld c, iyh        */ self.copy_register(Register::C, Register::IYH),
            0x4D => /* ld c, iyl        */ self.copy_register(Register::C, Register::IYL),
            0x4E => /* ld c, (iy+*)     */ self.read_index_indirect_wz(Register::C, WideRegister::IY, bus),

            0x54 => /* ld d, iyh        */ self.copy_register(Register::D, Register::IYH),
            0x55 => /* ld d, iyl        */ self.copy_register(Register::D, Register::IYL),
            0x56 => /* ld d, (iy+*)     */ self.read_index_indirect_wz(Register::D, WideRegister::IY, bus),
            0x5C => /* ld e, iyh        */ self.copy_register(Register::E, Register::IYH),
            0x5D => /* ld e, iyl        */ self.copy_register(Register::E, Register::IYL),
            0x5E => /* ld e, (iy+*)     */ self.read_index_indirect_wz(Register::E, WideRegister::IY, bus),

            0x60 => /* ld iyh, b        */ self.copy_register(Register::IYH, Register::B),
            0x61 => /* ld iyh, c        */ self.copy_register(Register::IYH, Register::C),
            0x62 => /* ld iyh, d        */ self.copy_register(Register::IYH, Register::D),
            0x63 => /* ld iyh, e        */ self.copy_register(Register::IYH, Register::E),
            0x64 => /* ld iyh, iyh      */ self.copy_register(Register::IYH, Register::IYH),
            0x65 => /* ld iyh, iyl      */ self.copy_register(Register::IYH, Register::IYL),
            0x66 => /* ld h, (iy+*)     */ self.read_index_indirect_wz(Register::H, WideRegister::IY, bus),
            0x67 => /* ld iyh, a        */ self.copy_register(Register::IYH, Register::A),
            0x68 => /* ld iyl, b        */ self.copy_register(Register::IYL, Register::B),
            0x69 => /* ld iyl, c        */ self.copy_register(Register::IYL, Register::C),
            0x6A => /* ld iyl, d        */ self.copy_register(Register::IYL, Register::D),
            0x6B => /* ld iyl, e        */ self.copy_register(Register::IYL, Register::E),
            0x6C => /* ld iyl, iyh      */ self.copy_register(Register::IYL, Register::IYH),
            0x6D => /* ld iyl, iyl      */ self.copy_register(Register::IYL, Register::IYL),
            0x6E => /* ld l, (iy+*)     */ self.read_index_indirect_wz(Register::L, WideRegister::IY, bus),
            0x6F => /* ld iyl, a        */ self.copy_register(Register::IYL, Register::A),

            0x70 => /* ld (iy+*), b     */ self.write_index_indirect_wz(WideRegister::IY, Register::B, bus),
            0x71 => /* ld (iy+*), c     */ self.write_index_indirect_wz(WideRegister::IY, Register::C, bus),
            0x72 => /* ld (iy+*), d     */ self.write_index_indirect_wz(WideRegister::IY, Register::D, bus),
            0x73 => /* ld (iy+*), e     */ self.write_index_indirect_wz(WideRegister::IY, Register::E, bus),
            0x74 => /* ld (iy+*), h     */ self.write_index_indirect_wz(WideRegister::IY, Register::H, bus),
            0x75 => /* ld (iy+*), l     */ self.write_index_indirect_wz(WideRegister::IY, Register::L, bus),
            0x77 => /* ld (iy+*), a     */ self.write_index_indirect_wz(WideRegister::IY, Register::A, bus),
            0x7C => /* ld a, iyh        */ self.copy_register(Register::A, Register::IYH),
            0x7D => /* ld a, iyl        */ self.copy_register(Register::A, Register::IYL),
            0x7E => /* ld a, (iy+*)     */ self.read_index_indirect_wz(Register::A, WideRegister::IY, bus),

            0x84 => /* add a, iyh       */ self.add_carry(Register::IYH, false),
            0x85 => /* add a, iyl       */ self.add_carry(Register::IYL, false),
            0x86 => /* add a, (iy+*)    */ self.add_carry_index_indirect_wz(WideRegister::IY, false, bus),
            0x8C => /* adc a, iyh       */ self.add_carry(Register::IYH, self.flag(Flag::C)),
            0x8D => /* adc a, iyl       */ self.add_carry(Register::IYL, self.flag(Flag::C)),
            0x8E => /* adc a, (iy+*)    */ self.add_carry_index_indirect_wz(WideRegister::IY, self.flag(Flag::C), bus),

            0x94 => /* sub iyh          */ self.sub_carry(Register::IYH, false),
            0x95 => /* sub iyl          */ self.sub_carry(Register::IYL, false),
            0x96 => /* sub (iy+*)       */ self.sub_carry_index_indirect_wz(WideRegister::IY, false, bus),
            0x9C => /* sbc a, iyh       */ self.sub_carry(Register::IYH, self.flag(Flag::C)),
            0x9D => /* sbc a, iyl       */ self.sub_carry(Register::IYL, self.flag(Flag::C)),
            0x9E => /* sbc a, (iy+*)    */ self.sub_carry_index_indirect_wz(WideRegister::IY, self.flag(Flag::C), bus),

            0xA4 => /* and iyh          */ self.and(Register::IYH),
            0xA5 => /* and iyl          */ self.and(Register::IYL),
            0xA6 => /* and (iy+*)       */ self.and_index_indirect_wz(WideRegister::IY, bus),
            0xAC => /* xor iyh          */ self.xor(Register::IYH),
            0xAD => /* xor iyl          */ self.xor(Register::IYL),
            0xAE => /* xor (iy+*)       */ self.xor_index_indirect_wz(WideRegister::IY, bus),

            0xB4 => /* or iyh           */ self.or(Register::IYH),
            0xB5 => /* or iyl           */ self.or(Register::IYL),
            0xB6 => /* or (iy+*)        */ self.or_index_indirect_wz(WideRegister::IY, bus),
            0xBC => /* cp iyh           */ self.compare(Register::IYH),
            0xBD => /* cp iyl           */ self.compare(Register::IYL),
            0xBE => /* cp (iy+*)        */ self.compare_index_indirect_wz(WideRegister::IY, bus),

            0xCB => /*                  */ self.fdcb_prefix(bus),

            0xE1 => /* pop iy           */ self.pop(WideRegister::IY, bus),
            0xE3 => /* ex (sp), iy      */ self.exchange_stack_pointer_indirect_wz(WideRegister::IY, bus),
            0xE5 => /* push iy          */ self.push(WideRegister::IY, bus),
            0xE9 => /* jp (iy)          */ self.jump_indirect(WideRegister::IY, bus),

            0xF9 => /* ld sp, iy        */ self.copy_wide_register(WideRegister::SP, WideRegister::IY),

            // Any other opcode seems to act as if the prefix was a nop
            _ => self.step(bus),
        })
    }

    fn ddcb_prefix(&mut self, bus: &mut impl Bus) -> usize {
        let offset = self.immediate(bus) as i8 as i16;
        let opcode = self.fetch(bus);
        #[rustfmt::skip]
        (4 + match opcode {
            0x00 => /* rlc (ix+*), b    */ self.rlc_index_indirect_wz(offset, WideRegister::IX, Some(Register::B), bus),
            0x01 => /* rlc (ix+*), c    */ self.rlc_index_indirect_wz(offset, WideRegister::IX, Some(Register::C), bus),
            0x02 => /* rlc (ix+*), d    */ self.rlc_index_indirect_wz(offset, WideRegister::IX, Some(Register::D), bus),
            0x03 => /* rlc (ix+*), e    */ self.rlc_index_indirect_wz(offset, WideRegister::IX, Some(Register::E), bus),
            0x04 => /* rlc (ix+*), h    */ self.rlc_index_indirect_wz(offset, WideRegister::IX, Some(Register::H), bus),
            0x05 => /* rlc (ix+*), l    */ self.rlc_index_indirect_wz(offset, WideRegister::IX, Some(Register::L), bus),
            0x06 => /* rlc (ix+*)       */ self.rlc_index_indirect_wz(offset, WideRegister::IX, None, bus),
            0x07 => /* rlc (ix+*), a    */ self.rlc_index_indirect_wz(offset, WideRegister::IX, Some(Register::A), bus),
            0x08 => /* rrc (ix+*), b    */ self.rrc_index_indirect_wz(offset, WideRegister::IX, Some(Register::B), bus),
            0x09 => /* rrc (ix+*), c    */ self.rrc_index_indirect_wz(offset, WideRegister::IX, Some(Register::C), bus),
            0x0A => /* rrc (ix+*), d    */ self.rrc_index_indirect_wz(offset, WideRegister::IX, Some(Register::D), bus),
            0x0B => /* rrc (ix+*), e    */ self.rrc_index_indirect_wz(offset, WideRegister::IX, Some(Register::E), bus),
            0x0C => /* rrc (ix+*), h    */ self.rrc_index_indirect_wz(offset, WideRegister::IX, Some(Register::H), bus),
            0x0D => /* rrc (ix+*), l    */ self.rrc_index_indirect_wz(offset, WideRegister::IX, Some(Register::L), bus),
            0x0E => /* rrc (ix+*)       */ self.rrc_index_indirect_wz(offset, WideRegister::IX, None, bus),
            0x0F => /* rrc (ix+*), a    */ self.rrc_index_indirect_wz(offset, WideRegister::IX, Some(Register::A), bus),

            0x10 => /* rl (ix+*), b     */ self.rl_index_indirect_wz(offset, WideRegister::IX, Some(Register::B), bus),
            0x11 => /* rl (ix+*), c     */ self.rl_index_indirect_wz(offset, WideRegister::IX, Some(Register::C), bus),
            0x12 => /* rl (ix+*), d     */ self.rl_index_indirect_wz(offset, WideRegister::IX, Some(Register::D), bus),
            0x13 => /* rl (ix+*), e     */ self.rl_index_indirect_wz(offset, WideRegister::IX, Some(Register::E), bus),
            0x14 => /* rl (ix+*), h     */ self.rl_index_indirect_wz(offset, WideRegister::IX, Some(Register::H), bus),
            0x15 => /* rl (ix+*), l     */ self.rl_index_indirect_wz(offset, WideRegister::IX, Some(Register::L), bus),
            0x16 => /* rl (ix+*)        */ self.rl_index_indirect_wz(offset, WideRegister::IX, None, bus),
            0x17 => /* rl (ix+*), a     */ self.rl_index_indirect_wz(offset, WideRegister::IX, Some(Register::A), bus),
            0x18 => /* rr (ix+*), b     */ self.rr_index_indirect_wz(offset, WideRegister::IX, Some(Register::B), bus),
            0x19 => /* rr (ix+*), c     */ self.rr_index_indirect_wz(offset, WideRegister::IX, Some(Register::C), bus),
            0x1A => /* rr (ix+*), d     */ self.rr_index_indirect_wz(offset, WideRegister::IX, Some(Register::D), bus),
            0x1B => /* rr (ix+*), e     */ self.rr_index_indirect_wz(offset, WideRegister::IX, Some(Register::E), bus),
            0x1C => /* rr (ix+*), h     */ self.rr_index_indirect_wz(offset, WideRegister::IX, Some(Register::H), bus),
            0x1D => /* rr (ix+*), l     */ self.rr_index_indirect_wz(offset, WideRegister::IX, Some(Register::L), bus),
            0x1E => /* rr (ix+*)        */ self.rr_index_indirect_wz(offset, WideRegister::IX, None, bus),
            0x1F => /* rr (ix+*), a     */ self.rr_index_indirect_wz(offset, WideRegister::IX, Some(Register::A), bus),

            0x20 => /* sla (ix+*), b    */ self.sla_index_indirect_wz(offset, WideRegister::IX, Some(Register::B), bus),
            0x21 => /* sla (ix+*), c    */ self.sla_index_indirect_wz(offset, WideRegister::IX, Some(Register::C), bus),
            0x22 => /* sla (ix+*), d    */ self.sla_index_indirect_wz(offset, WideRegister::IX, Some(Register::D), bus),
            0x23 => /* sla (ix+*), e    */ self.sla_index_indirect_wz(offset, WideRegister::IX, Some(Register::E), bus),
            0x24 => /* sla (ix+*), h    */ self.sla_index_indirect_wz(offset, WideRegister::IX, Some(Register::H), bus),
            0x25 => /* sla (ix+*), l    */ self.sla_index_indirect_wz(offset, WideRegister::IX, Some(Register::L), bus),
            0x26 => /* sla (ix+*)       */ self.sla_index_indirect_wz(offset, WideRegister::IX, None, bus),
            0x27 => /* sla (ix+*), a    */ self.sla_index_indirect_wz(offset, WideRegister::IX, Some(Register::A), bus),
            0x28 => /* sra (ix+*), b    */ self.sra_index_indirect_wz(offset, WideRegister::IX, Some(Register::B), bus),
            0x29 => /* sra (ix+*), c    */ self.sra_index_indirect_wz(offset, WideRegister::IX, Some(Register::C), bus),
            0x2A => /* sra (ix+*), d    */ self.sra_index_indirect_wz(offset, WideRegister::IX, Some(Register::D), bus),
            0x2B => /* sra (ix+*), e    */ self.sra_index_indirect_wz(offset, WideRegister::IX, Some(Register::E), bus),
            0x2C => /* sra (ix+*), h    */ self.sra_index_indirect_wz(offset, WideRegister::IX, Some(Register::H), bus),
            0x2D => /* sra (ix+*), l    */ self.sra_index_indirect_wz(offset, WideRegister::IX, Some(Register::L), bus),
            0x2E => /* sra (ix+*)       */ self.sra_index_indirect_wz(offset, WideRegister::IX, None, bus),
            0x2F => /* sra (ix+*), a    */ self.sra_index_indirect_wz(offset, WideRegister::IX, Some(Register::A), bus),

            0x30 => /* sll (ix+*), b    */ self.sll_index_indirect_wz(offset, WideRegister::IX, Some(Register::B), bus),
            0x31 => /* sll (ix+*), c    */ self.sll_index_indirect_wz(offset, WideRegister::IX, Some(Register::C), bus),
            0x32 => /* sll (ix+*), d    */ self.sll_index_indirect_wz(offset, WideRegister::IX, Some(Register::D), bus),
            0x33 => /* sll (ix+*), e    */ self.sll_index_indirect_wz(offset, WideRegister::IX, Some(Register::E), bus),
            0x34 => /* sll (ix+*), h    */ self.sll_index_indirect_wz(offset, WideRegister::IX, Some(Register::H), bus),
            0x35 => /* sll (ix+*), l    */ self.sll_index_indirect_wz(offset, WideRegister::IX, Some(Register::L), bus),
            0x36 => /* sll (ix+*)       */ self.sll_index_indirect_wz(offset, WideRegister::IX, None, bus),
            0x37 => /* sll (ix+*), a    */ self.sll_index_indirect_wz(offset, WideRegister::IX, Some(Register::A), bus),
            0x38 => /* srl (ix+*), b    */ self.srl_index_indirect_wz(offset, WideRegister::IX, Some(Register::B), bus),
            0x39 => /* srl (ix+*), c    */ self.srl_index_indirect_wz(offset, WideRegister::IX, Some(Register::C), bus),
            0x3A => /* srl (ix+*), d    */ self.srl_index_indirect_wz(offset, WideRegister::IX, Some(Register::D), bus),
            0x3B => /* srl (ix+*), e    */ self.srl_index_indirect_wz(offset, WideRegister::IX, Some(Register::E), bus),
            0x3C => /* srl (ix+*), h    */ self.srl_index_indirect_wz(offset, WideRegister::IX, Some(Register::H), bus),
            0x3D => /* srl (ix+*), l    */ self.srl_index_indirect_wz(offset, WideRegister::IX, Some(Register::L), bus),
            0x3E => /* srl (ix+*)       */ self.srl_index_indirect_wz(offset, WideRegister::IX, None, bus),
            0x3F => /* srl (ix+*), a    */ self.srl_index_indirect_wz(offset, WideRegister::IX, Some(Register::A), bus),

            0x40 => /* bit 0, (ix+*)    */ self.bit_index_indirect_wz(0x01, offset, WideRegister::IX, bus),
            0x41 => /* bit 0, (ix+*)    */ self.bit_index_indirect_wz(0x01, offset, WideRegister::IX, bus),
            0x42 => /* bit 0, (ix+*)    */ self.bit_index_indirect_wz(0x01, offset, WideRegister::IX, bus),
            0x43 => /* bit 0, (ix+*)    */ self.bit_index_indirect_wz(0x01, offset, WideRegister::IX, bus),
            0x44 => /* bit 0, (ix+*)    */ self.bit_index_indirect_wz(0x01, offset, WideRegister::IX, bus),
            0x45 => /* bit 0, (ix+*)    */ self.bit_index_indirect_wz(0x01, offset, WideRegister::IX, bus),
            0x46 => /* bit 0, (ix+*)    */ self.bit_index_indirect_wz(0x01, offset, WideRegister::IX, bus),
            0x47 => /* bit 0, (ix+*)    */ self.bit_index_indirect_wz(0x01, offset, WideRegister::IX, bus),
            0x48 => /* bit 1, (ix+*)    */ self.bit_index_indirect_wz(0x02, offset, WideRegister::IX, bus),
            0x49 => /* bit 1, (ix+*)    */ self.bit_index_indirect_wz(0x02, offset, WideRegister::IX, bus),
            0x4A => /* bit 1, (ix+*)    */ self.bit_index_indirect_wz(0x02, offset, WideRegister::IX, bus),
            0x4B => /* bit 1, (ix+*)    */ self.bit_index_indirect_wz(0x02, offset, WideRegister::IX, bus),
            0x4C => /* bit 1, (ix+*)    */ self.bit_index_indirect_wz(0x02, offset, WideRegister::IX, bus),
            0x4D => /* bit 1, (ix+*)    */ self.bit_index_indirect_wz(0x02, offset, WideRegister::IX, bus),
            0x4E => /* bit 1, (ix+*)    */ self.bit_index_indirect_wz(0x02, offset, WideRegister::IX, bus),
            0x4F => /* bit 1, (ix+*)    */ self.bit_index_indirect_wz(0x02, offset, WideRegister::IX, bus),

            0x50 => /* bit 2, (ix+*)    */ self.bit_index_indirect_wz(0x04, offset, WideRegister::IX, bus),
            0x51 => /* bit 2, (ix+*)    */ self.bit_index_indirect_wz(0x04, offset, WideRegister::IX, bus),
            0x52 => /* bit 2, (ix+*)    */ self.bit_index_indirect_wz(0x04, offset, WideRegister::IX, bus),
            0x53 => /* bit 2, (ix+*)    */ self.bit_index_indirect_wz(0x04, offset, WideRegister::IX, bus),
            0x54 => /* bit 2, (ix+*)    */ self.bit_index_indirect_wz(0x04, offset, WideRegister::IX, bus),
            0x55 => /* bit 2, (ix+*)    */ self.bit_index_indirect_wz(0x04, offset, WideRegister::IX, bus),
            0x56 => /* bit 2, (ix+*)    */ self.bit_index_indirect_wz(0x04, offset, WideRegister::IX, bus),
            0x57 => /* bit 2, (ix+*)    */ self.bit_index_indirect_wz(0x04, offset, WideRegister::IX, bus),
            0x58 => /* bit 3, (ix+*)    */ self.bit_index_indirect_wz(0x08, offset, WideRegister::IX, bus),
            0x59 => /* bit 3, (ix+*)    */ self.bit_index_indirect_wz(0x08, offset, WideRegister::IX, bus),
            0x5A => /* bit 3, (ix+*)    */ self.bit_index_indirect_wz(0x08, offset, WideRegister::IX, bus),
            0x5B => /* bit 3, (ix+*)    */ self.bit_index_indirect_wz(0x08, offset, WideRegister::IX, bus),
            0x5C => /* bit 3, (ix+*)    */ self.bit_index_indirect_wz(0x08, offset, WideRegister::IX, bus),
            0x5D => /* bit 3, (ix+*)    */ self.bit_index_indirect_wz(0x08, offset, WideRegister::IX, bus),
            0x5E => /* bit 3, (ix+*)    */ self.bit_index_indirect_wz(0x08, offset, WideRegister::IX, bus),
            0x5F => /* bit 3, (ix+*)    */ self.bit_index_indirect_wz(0x08, offset, WideRegister::IX, bus),

            0x60 => /* bit 4, (ix+*)    */ self.bit_index_indirect_wz(0x10, offset, WideRegister::IX, bus),
            0x61 => /* bit 4, (ix+*)    */ self.bit_index_indirect_wz(0x10, offset, WideRegister::IX, bus),
            0x62 => /* bit 4, (ix+*)    */ self.bit_index_indirect_wz(0x10, offset, WideRegister::IX, bus),
            0x63 => /* bit 4, (ix+*)    */ self.bit_index_indirect_wz(0x10, offset, WideRegister::IX, bus),
            0x64 => /* bit 4, (ix+*)    */ self.bit_index_indirect_wz(0x10, offset, WideRegister::IX, bus),
            0x65 => /* bit 4, (ix+*)    */ self.bit_index_indirect_wz(0x10, offset, WideRegister::IX, bus),
            0x66 => /* bit 4, (ix+*)    */ self.bit_index_indirect_wz(0x10, offset, WideRegister::IX, bus),
            0x67 => /* bit 4, (ix+*)    */ self.bit_index_indirect_wz(0x10, offset, WideRegister::IX, bus),
            0x68 => /* bit 5, (ix+*)    */ self.bit_index_indirect_wz(0x20, offset, WideRegister::IX, bus),
            0x69 => /* bit 5, (ix+*)    */ self.bit_index_indirect_wz(0x20, offset, WideRegister::IX, bus),
            0x6A => /* bit 5, (ix+*)    */ self.bit_index_indirect_wz(0x20, offset, WideRegister::IX, bus),
            0x6B => /* bit 5, (ix+*)    */ self.bit_index_indirect_wz(0x20, offset, WideRegister::IX, bus),
            0x6C => /* bit 5, (ix+*)    */ self.bit_index_indirect_wz(0x20, offset, WideRegister::IX, bus),
            0x6D => /* bit 5, (ix+*)    */ self.bit_index_indirect_wz(0x20, offset, WideRegister::IX, bus),
            0x6E => /* bit 5, (ix+*)    */ self.bit_index_indirect_wz(0x20, offset, WideRegister::IX, bus),
            0x6F => /* bit 5, (ix+*)    */ self.bit_index_indirect_wz(0x20, offset, WideRegister::IX, bus),

            0x70 => /* bit 6, (ix+*)    */ self.bit_index_indirect_wz(0x40, offset, WideRegister::IX, bus),
            0x71 => /* bit 6, (ix+*)    */ self.bit_index_indirect_wz(0x40, offset, WideRegister::IX, bus),
            0x72 => /* bit 6, (ix+*)    */ self.bit_index_indirect_wz(0x40, offset, WideRegister::IX, bus),
            0x73 => /* bit 6, (ix+*)    */ self.bit_index_indirect_wz(0x40, offset, WideRegister::IX, bus),
            0x74 => /* bit 6, (ix+*)    */ self.bit_index_indirect_wz(0x40, offset, WideRegister::IX, bus),
            0x75 => /* bit 6, (ix+*)    */ self.bit_index_indirect_wz(0x40, offset, WideRegister::IX, bus),
            0x76 => /* bit 6, (ix+*)    */ self.bit_index_indirect_wz(0x40, offset, WideRegister::IX, bus),
            0x77 => /* bit 6, (ix+*)    */ self.bit_index_indirect_wz(0x40, offset, WideRegister::IX, bus),
            0x78 => /* bit 7, (ix+*)    */ self.bit_index_indirect_wz(0x80, offset, WideRegister::IX, bus),
            0x79 => /* bit 7, (ix+*)    */ self.bit_index_indirect_wz(0x80, offset, WideRegister::IX, bus),
            0x7A => /* bit 7, (ix+*)    */ self.bit_index_indirect_wz(0x80, offset, WideRegister::IX, bus),
            0x7B => /* bit 7, (ix+*)    */ self.bit_index_indirect_wz(0x80, offset, WideRegister::IX, bus),
            0x7C => /* bit 7, (ix+*)    */ self.bit_index_indirect_wz(0x80, offset, WideRegister::IX, bus),
            0x7D => /* bit 7, (ix+*)    */ self.bit_index_indirect_wz(0x80, offset, WideRegister::IX, bus),
            0x7E => /* bit 7, (ix+*)    */ self.bit_index_indirect_wz(0x80, offset, WideRegister::IX, bus),
            0x7F => /* bit 7, (ix+*)    */ self.bit_index_indirect_wz(0x80, offset, WideRegister::IX, bus),

            0x80 => /* res 0, (ix+*), b */ self.reset_bit_index_indirect_wz(0x01, offset, WideRegister::IX, Some(Register::B), bus),
            0x81 => /* res 0, (ix+*), c */ self.reset_bit_index_indirect_wz(0x01, offset, WideRegister::IX, Some(Register::C), bus),
            0x82 => /* res 0, (ix+*), d */ self.reset_bit_index_indirect_wz(0x01, offset, WideRegister::IX, Some(Register::D), bus),
            0x83 => /* res 0, (ix+*), e */ self.reset_bit_index_indirect_wz(0x01, offset, WideRegister::IX, Some(Register::E), bus),
            0x84 => /* res 0, (ix+*), h */ self.reset_bit_index_indirect_wz(0x01, offset, WideRegister::IX, Some(Register::H), bus),
            0x85 => /* res 0, (ix+*), l */ self.reset_bit_index_indirect_wz(0x01, offset, WideRegister::IX, Some(Register::L), bus),
            0x86 => /* res 0, (ix+*)    */ self.reset_bit_index_indirect_wz(0x01, offset, WideRegister::IX, None, bus),
            0x87 => /* res 0, (ix+*), a */ self.reset_bit_index_indirect_wz(0x01, offset, WideRegister::IX, Some(Register::A), bus),
            0x88 => /* res 1, (ix+*), b */ self.reset_bit_index_indirect_wz(0x02, offset, WideRegister::IX, Some(Register::B), bus),
            0x89 => /* res 1, (ix+*), c */ self.reset_bit_index_indirect_wz(0x02, offset, WideRegister::IX, Some(Register::C), bus),
            0x8A => /* res 1, (ix+*), d */ self.reset_bit_index_indirect_wz(0x02, offset, WideRegister::IX, Some(Register::D), bus),
            0x8B => /* res 1, (ix+*), e */ self.reset_bit_index_indirect_wz(0x02, offset, WideRegister::IX, Some(Register::E), bus),
            0x8C => /* res 1, (ix+*), h */ self.reset_bit_index_indirect_wz(0x02, offset, WideRegister::IX, Some(Register::H), bus),
            0x8D => /* res 1, (ix+*), l */ self.reset_bit_index_indirect_wz(0x02, offset, WideRegister::IX, Some(Register::L), bus),
            0x8E => /* res 1, (ix+*)    */ self.reset_bit_index_indirect_wz(0x02, offset, WideRegister::IX, None, bus),
            0x8F => /* res 1, (ix+*), a */ self.reset_bit_index_indirect_wz(0x02, offset, WideRegister::IX, Some(Register::A), bus),

            0x90 => /* res 2, (ix+*), b */ self.reset_bit_index_indirect_wz(0x04, offset, WideRegister::IX, Some(Register::B), bus),
            0x91 => /* res 2, (ix+*), c */ self.reset_bit_index_indirect_wz(0x04, offset, WideRegister::IX, Some(Register::C), bus),
            0x92 => /* res 2, (ix+*), d */ self.reset_bit_index_indirect_wz(0x04, offset, WideRegister::IX, Some(Register::D), bus),
            0x93 => /* res 2, (ix+*), e */ self.reset_bit_index_indirect_wz(0x04, offset, WideRegister::IX, Some(Register::E), bus),
            0x94 => /* res 2, (ix+*), h */ self.reset_bit_index_indirect_wz(0x04, offset, WideRegister::IX, Some(Register::H), bus),
            0x95 => /* res 2, (ix+*), l */ self.reset_bit_index_indirect_wz(0x04, offset, WideRegister::IX, Some(Register::L), bus),
            0x96 => /* res 2, (ix+*)    */ self.reset_bit_index_indirect_wz(0x04, offset, WideRegister::IX, None, bus),
            0x97 => /* res 2, (ix+*), a */ self.reset_bit_index_indirect_wz(0x04, offset, WideRegister::IX, Some(Register::A), bus),
            0x98 => /* res 3, (ix+*), b */ self.reset_bit_index_indirect_wz(0x08, offset, WideRegister::IX, Some(Register::B), bus),
            0x99 => /* res 3, (ix+*), c */ self.reset_bit_index_indirect_wz(0x08, offset, WideRegister::IX, Some(Register::C), bus),
            0x9A => /* res 3, (ix+*), d */ self.reset_bit_index_indirect_wz(0x08, offset, WideRegister::IX, Some(Register::D), bus),
            0x9B => /* res 3, (ix+*), e */ self.reset_bit_index_indirect_wz(0x08, offset, WideRegister::IX, Some(Register::E), bus),
            0x9C => /* res 3, (ix+*), h */ self.reset_bit_index_indirect_wz(0x08, offset, WideRegister::IX, Some(Register::H), bus),
            0x9D => /* res 3, (ix+*), l */ self.reset_bit_index_indirect_wz(0x08, offset, WideRegister::IX, Some(Register::L), bus),
            0x9E => /* res 3, (ix+*)    */ self.reset_bit_index_indirect_wz(0x08, offset, WideRegister::IX, None, bus),
            0x9F => /* res 3, (ix+*), a */ self.reset_bit_index_indirect_wz(0x08, offset, WideRegister::IX, Some(Register::A), bus),

            0xA0 => /* res 4, (ix+*), b */ self.reset_bit_index_indirect_wz(0x10, offset, WideRegister::IX, Some(Register::B), bus),
            0xA1 => /* res 4, (ix+*), c */ self.reset_bit_index_indirect_wz(0x10, offset, WideRegister::IX, Some(Register::C), bus),
            0xA2 => /* res 4, (ix+*), d */ self.reset_bit_index_indirect_wz(0x10, offset, WideRegister::IX, Some(Register::D), bus),
            0xA3 => /* res 4, (ix+*), e */ self.reset_bit_index_indirect_wz(0x10, offset, WideRegister::IX, Some(Register::E), bus),
            0xA4 => /* res 4, (ix+*), h */ self.reset_bit_index_indirect_wz(0x10, offset, WideRegister::IX, Some(Register::H), bus),
            0xA5 => /* res 4, (ix+*), l */ self.reset_bit_index_indirect_wz(0x10, offset, WideRegister::IX, Some(Register::L), bus),
            0xA6 => /* res 4, (ix+*)    */ self.reset_bit_index_indirect_wz(0x10, offset, WideRegister::IX, None, bus),
            0xA7 => /* res 4, (ix+*), a */ self.reset_bit_index_indirect_wz(0x10, offset, WideRegister::IX, Some(Register::A), bus),
            0xA8 => /* res 5, (ix+*), b */ self.reset_bit_index_indirect_wz(0x20, offset, WideRegister::IX, Some(Register::B), bus),
            0xA9 => /* res 5, (ix+*), c */ self.reset_bit_index_indirect_wz(0x20, offset, WideRegister::IX, Some(Register::C), bus),
            0xAA => /* res 5, (ix+*), d */ self.reset_bit_index_indirect_wz(0x20, offset, WideRegister::IX, Some(Register::D), bus),
            0xAB => /* res 5, (ix+*), e */ self.reset_bit_index_indirect_wz(0x20, offset, WideRegister::IX, Some(Register::E), bus),
            0xAC => /* res 5, (ix+*), h */ self.reset_bit_index_indirect_wz(0x20, offset, WideRegister::IX, Some(Register::H), bus),
            0xAD => /* res 5, (ix+*), l */ self.reset_bit_index_indirect_wz(0x20, offset, WideRegister::IX, Some(Register::L), bus),
            0xAE => /* res 5, (ix+*)    */ self.reset_bit_index_indirect_wz(0x20, offset, WideRegister::IX, None, bus),
            0xAF => /* res 5, (ix+*), a */ self.reset_bit_index_indirect_wz(0x20, offset, WideRegister::IX, Some(Register::A), bus),

            0xB0 => /* res 6, (ix+*), b */ self.reset_bit_index_indirect_wz(0x40, offset, WideRegister::IX, Some(Register::B), bus),
            0xB1 => /* res 6, (ix+*), c */ self.reset_bit_index_indirect_wz(0x40, offset, WideRegister::IX, Some(Register::C), bus),
            0xB2 => /* res 6, (ix+*), d */ self.reset_bit_index_indirect_wz(0x40, offset, WideRegister::IX, Some(Register::D), bus),
            0xB3 => /* res 6, (ix+*), e */ self.reset_bit_index_indirect_wz(0x40, offset, WideRegister::IX, Some(Register::E), bus),
            0xB4 => /* res 6, (ix+*), h */ self.reset_bit_index_indirect_wz(0x40, offset, WideRegister::IX, Some(Register::H), bus),
            0xB5 => /* res 6, (ix+*), l */ self.reset_bit_index_indirect_wz(0x40, offset, WideRegister::IX, Some(Register::L), bus),
            0xB6 => /* res 6, (ix+*)    */ self.reset_bit_index_indirect_wz(0x40, offset, WideRegister::IX, None, bus),
            0xB7 => /* res 6, (ix+*), a */ self.reset_bit_index_indirect_wz(0x40, offset, WideRegister::IX, Some(Register::A), bus),
            0xB8 => /* res 7, (ix+*), b */ self.reset_bit_index_indirect_wz(0x80, offset, WideRegister::IX, Some(Register::B), bus),
            0xB9 => /* res 7, (ix+*), c */ self.reset_bit_index_indirect_wz(0x80, offset, WideRegister::IX, Some(Register::C), bus),
            0xBA => /* res 7, (ix+*), d */ self.reset_bit_index_indirect_wz(0x80, offset, WideRegister::IX, Some(Register::D), bus),
            0xBB => /* res 7, (ix+*), e */ self.reset_bit_index_indirect_wz(0x80, offset, WideRegister::IX, Some(Register::E), bus),
            0xBC => /* res 7, (ix+*), h */ self.reset_bit_index_indirect_wz(0x80, offset, WideRegister::IX, Some(Register::H), bus),
            0xBD => /* res 7, (ix+*), l */ self.reset_bit_index_indirect_wz(0x80, offset, WideRegister::IX, Some(Register::L), bus),
            0xBE => /* res 7, (ix+*)    */ self.reset_bit_index_indirect_wz(0x80, offset, WideRegister::IX, None, bus),
            0xBF => /* res 7, (ix+*), a */ self.reset_bit_index_indirect_wz(0x80, offset, WideRegister::IX, Some(Register::A), bus),

            0xC0 => /* set 0, (ix+*), b */ self.set_bit_index_indirect_wz(0x01, offset, WideRegister::IX, Some(Register::B), bus),
            0xC1 => /* set 0, (ix+*), c */ self.set_bit_index_indirect_wz(0x01, offset, WideRegister::IX, Some(Register::C), bus),
            0xC2 => /* set 0, (ix+*), d */ self.set_bit_index_indirect_wz(0x01, offset, WideRegister::IX, Some(Register::D), bus),
            0xC3 => /* set 0, (ix+*), e */ self.set_bit_index_indirect_wz(0x01, offset, WideRegister::IX, Some(Register::E), bus),
            0xC4 => /* set 0, (ix+*), h */ self.set_bit_index_indirect_wz(0x01, offset, WideRegister::IX, Some(Register::H), bus),
            0xC5 => /* set 0, (ix+*), l */ self.set_bit_index_indirect_wz(0x01, offset, WideRegister::IX, Some(Register::L), bus),
            0xC6 => /* set 0, (ix+*)    */ self.set_bit_index_indirect_wz(0x01, offset, WideRegister::IX, None, bus),
            0xC7 => /* set 0, (ix+*), a */ self.set_bit_index_indirect_wz(0x01, offset, WideRegister::IX, Some(Register::A), bus),
            0xC8 => /* set 1, (ix+*), b */ self.set_bit_index_indirect_wz(0x02, offset, WideRegister::IX, Some(Register::B), bus),
            0xC9 => /* set 1, (ix+*), c */ self.set_bit_index_indirect_wz(0x02, offset, WideRegister::IX, Some(Register::C), bus),
            0xCA => /* set 1, (ix+*), d */ self.set_bit_index_indirect_wz(0x02, offset, WideRegister::IX, Some(Register::D), bus),
            0xCB => /* set 1, (ix+*), e */ self.set_bit_index_indirect_wz(0x02, offset, WideRegister::IX, Some(Register::E), bus),
            0xCC => /* set 1, (ix+*), h */ self.set_bit_index_indirect_wz(0x02, offset, WideRegister::IX, Some(Register::H), bus),
            0xCD => /* set 1, (ix+*), l */ self.set_bit_index_indirect_wz(0x02, offset, WideRegister::IX, Some(Register::L), bus),
            0xCE => /* set 1, (ix+*)    */ self.set_bit_index_indirect_wz(0x02, offset, WideRegister::IX, None, bus),
            0xCF => /* set 1, (ix+*), a */ self.set_bit_index_indirect_wz(0x02, offset, WideRegister::IX, Some(Register::A), bus),

            0xD0 => /* set 2, (ix+*), b */ self.set_bit_index_indirect_wz(0x04, offset, WideRegister::IX, Some(Register::B), bus),
            0xD1 => /* set 2, (ix+*), c */ self.set_bit_index_indirect_wz(0x04, offset, WideRegister::IX, Some(Register::C), bus),
            0xD2 => /* set 2, (ix+*), d */ self.set_bit_index_indirect_wz(0x04, offset, WideRegister::IX, Some(Register::D), bus),
            0xD3 => /* set 2, (ix+*), e */ self.set_bit_index_indirect_wz(0x04, offset, WideRegister::IX, Some(Register::E), bus),
            0xD4 => /* set 2, (ix+*), h */ self.set_bit_index_indirect_wz(0x04, offset, WideRegister::IX, Some(Register::H), bus),
            0xD5 => /* set 2, (ix+*), l */ self.set_bit_index_indirect_wz(0x04, offset, WideRegister::IX, Some(Register::L), bus),
            0xD6 => /* set 2, (ix+*)    */ self.set_bit_index_indirect_wz(0x04, offset, WideRegister::IX, None, bus),
            0xD7 => /* set 2, (ix+*), a */ self.set_bit_index_indirect_wz(0x04, offset, WideRegister::IX, Some(Register::A), bus),
            0xD8 => /* set 3, (ix+*), b */ self.set_bit_index_indirect_wz(0x08, offset, WideRegister::IX, Some(Register::B), bus),
            0xD9 => /* set 3, (ix+*), c */ self.set_bit_index_indirect_wz(0x08, offset, WideRegister::IX, Some(Register::C), bus),
            0xDA => /* set 3, (ix+*), d */ self.set_bit_index_indirect_wz(0x08, offset, WideRegister::IX, Some(Register::D), bus),
            0xDB => /* set 3, (ix+*), e */ self.set_bit_index_indirect_wz(0x08, offset, WideRegister::IX, Some(Register::E), bus),
            0xDC => /* set 3, (ix+*), h */ self.set_bit_index_indirect_wz(0x08, offset, WideRegister::IX, Some(Register::H), bus),
            0xDD => /* set 3, (ix+*), l */ self.set_bit_index_indirect_wz(0x08, offset, WideRegister::IX, Some(Register::L), bus),
            0xDE => /* set 3, (ix+*)    */ self.set_bit_index_indirect_wz(0x08, offset, WideRegister::IX, None, bus),
            0xDF => /* set 3, (ix+*), a */ self.set_bit_index_indirect_wz(0x08, offset, WideRegister::IX, Some(Register::A), bus),

            0xE0 => /* set 4, (ix+*), b */ self.set_bit_index_indirect_wz(0x10, offset, WideRegister::IX, Some(Register::B), bus),
            0xE1 => /* set 4, (ix+*), c */ self.set_bit_index_indirect_wz(0x10, offset, WideRegister::IX, Some(Register::C), bus),
            0xE2 => /* set 4, (ix+*), d */ self.set_bit_index_indirect_wz(0x10, offset, WideRegister::IX, Some(Register::D), bus),
            0xE3 => /* set 4, (ix+*), e */ self.set_bit_index_indirect_wz(0x10, offset, WideRegister::IX, Some(Register::E), bus),
            0xE4 => /* set 4, (ix+*), h */ self.set_bit_index_indirect_wz(0x10, offset, WideRegister::IX, Some(Register::H), bus),
            0xE5 => /* set 4, (ix+*), l */ self.set_bit_index_indirect_wz(0x10, offset, WideRegister::IX, Some(Register::L), bus),
            0xE6 => /* set 4, (ix+*)    */ self.set_bit_index_indirect_wz(0x10, offset, WideRegister::IX, None, bus),
            0xE7 => /* set 4, (ix+*), a */ self.set_bit_index_indirect_wz(0x10, offset, WideRegister::IX, Some(Register::A), bus),
            0xE8 => /* set 5, (ix+*), b */ self.set_bit_index_indirect_wz(0x20, offset, WideRegister::IX, Some(Register::B), bus),
            0xE9 => /* set 5, (ix+*), c */ self.set_bit_index_indirect_wz(0x20, offset, WideRegister::IX, Some(Register::C), bus),
            0xEA => /* set 5, (ix+*), d */ self.set_bit_index_indirect_wz(0x20, offset, WideRegister::IX, Some(Register::D), bus),
            0xEB => /* set 5, (ix+*), e */ self.set_bit_index_indirect_wz(0x20, offset, WideRegister::IX, Some(Register::E), bus),
            0xEC => /* set 5, (ix+*), h */ self.set_bit_index_indirect_wz(0x20, offset, WideRegister::IX, Some(Register::H), bus),
            0xED => /* set 5, (ix+*), l */ self.set_bit_index_indirect_wz(0x20, offset, WideRegister::IX, Some(Register::L), bus),
            0xEE => /* set 5, (ix+*)    */ self.set_bit_index_indirect_wz(0x20, offset, WideRegister::IX, None, bus),
            0xEF => /* set 5, (ix+*), a */ self.set_bit_index_indirect_wz(0x20, offset, WideRegister::IX, Some(Register::A), bus),

            0xF0 => /* set 6, (ix+*), b */ self.set_bit_index_indirect_wz(0x40, offset, WideRegister::IX, Some(Register::B), bus),
            0xF1 => /* set 6, (ix+*), c */ self.set_bit_index_indirect_wz(0x40, offset, WideRegister::IX, Some(Register::C), bus),
            0xF2 => /* set 6, (ix+*), d */ self.set_bit_index_indirect_wz(0x40, offset, WideRegister::IX, Some(Register::D), bus),
            0xF3 => /* set 6, (ix+*), e */ self.set_bit_index_indirect_wz(0x40, offset, WideRegister::IX, Some(Register::E), bus),
            0xF4 => /* set 6, (ix+*), h */ self.set_bit_index_indirect_wz(0x40, offset, WideRegister::IX, Some(Register::H), bus),
            0xF5 => /* set 6, (ix+*), l */ self.set_bit_index_indirect_wz(0x40, offset, WideRegister::IX, Some(Register::L), bus),
            0xF6 => /* set 6, (ix+*)    */ self.set_bit_index_indirect_wz(0x40, offset, WideRegister::IX, None, bus),
            0xF7 => /* set 6, (ix+*), a */ self.set_bit_index_indirect_wz(0x40, offset, WideRegister::IX, Some(Register::A), bus),
            0xF8 => /* set 7, (ix+*), b */ self.set_bit_index_indirect_wz(0x80, offset, WideRegister::IX, Some(Register::B), bus),
            0xF9 => /* set 7, (ix+*), c */ self.set_bit_index_indirect_wz(0x80, offset, WideRegister::IX, Some(Register::C), bus),
            0xFA => /* set 7, (ix+*), d */ self.set_bit_index_indirect_wz(0x80, offset, WideRegister::IX, Some(Register::D), bus),
            0xFB => /* set 7, (ix+*), e */ self.set_bit_index_indirect_wz(0x80, offset, WideRegister::IX, Some(Register::E), bus),
            0xFC => /* set 7, (ix+*), h */ self.set_bit_index_indirect_wz(0x80, offset, WideRegister::IX, Some(Register::H), bus),
            0xFD => /* set 7, (ix+*), l */ self.set_bit_index_indirect_wz(0x80, offset, WideRegister::IX, Some(Register::L), bus),
            0xFE => /* set 7, (ix+*)    */ self.set_bit_index_indirect_wz(0x80, offset, WideRegister::IX, None, bus),
            0xFF => /* set 7, (ix+*), a */ self.set_bit_index_indirect_wz(0x80, offset, WideRegister::IX, Some(Register::A), bus),
        })
    }

    fn fdcb_prefix(&mut self, bus: &mut impl Bus) -> usize {
        let offset = self.immediate(bus) as i8 as i16;
        let opcode = self.fetch(bus);
        #[rustfmt::skip]
        (4 + match opcode {
            0x00 => /* rlc (iy+*), b    */ self.rlc_index_indirect_wz(offset, WideRegister::IY, Some(Register::B), bus),
            0x01 => /* rlc (iy+*), c    */ self.rlc_index_indirect_wz(offset, WideRegister::IY, Some(Register::C), bus),
            0x02 => /* rlc (iy+*), d    */ self.rlc_index_indirect_wz(offset, WideRegister::IY, Some(Register::D), bus),
            0x03 => /* rlc (iy+*), e    */ self.rlc_index_indirect_wz(offset, WideRegister::IY, Some(Register::E), bus),
            0x04 => /* rlc (iy+*), h    */ self.rlc_index_indirect_wz(offset, WideRegister::IY, Some(Register::H), bus),
            0x05 => /* rlc (iy+*), l    */ self.rlc_index_indirect_wz(offset, WideRegister::IY, Some(Register::L), bus),
            0x06 => /* rlc (iy+*)       */ self.rlc_index_indirect_wz(offset, WideRegister::IY, None, bus),
            0x07 => /* rlc (iy+*), a    */ self.rlc_index_indirect_wz(offset, WideRegister::IY, Some(Register::A), bus),
            0x08 => /* rrc (iy+*), b    */ self.rrc_index_indirect_wz(offset, WideRegister::IY, Some(Register::B), bus),
            0x09 => /* rrc (iy+*), c    */ self.rrc_index_indirect_wz(offset, WideRegister::IY, Some(Register::C), bus),
            0x0A => /* rrc (iy+*), d    */ self.rrc_index_indirect_wz(offset, WideRegister::IY, Some(Register::D), bus),
            0x0B => /* rrc (iy+*), e    */ self.rrc_index_indirect_wz(offset, WideRegister::IY, Some(Register::E), bus),
            0x0C => /* rrc (iy+*), h    */ self.rrc_index_indirect_wz(offset, WideRegister::IY, Some(Register::H), bus),
            0x0D => /* rrc (iy+*), l    */ self.rrc_index_indirect_wz(offset, WideRegister::IY, Some(Register::L), bus),
            0x0E => /* rrc (iy+*)       */ self.rrc_index_indirect_wz(offset, WideRegister::IY, None, bus),
            0x0F => /* rrc (iy+*), a    */ self.rrc_index_indirect_wz(offset, WideRegister::IY, Some(Register::A), bus),

            0x10 => /* rl (iy+*), b     */ self.rl_index_indirect_wz(offset, WideRegister::IY, Some(Register::B), bus),
            0x11 => /* rl (iy+*), c     */ self.rl_index_indirect_wz(offset, WideRegister::IY, Some(Register::C), bus),
            0x12 => /* rl (iy+*), d     */ self.rl_index_indirect_wz(offset, WideRegister::IY, Some(Register::D), bus),
            0x13 => /* rl (iy+*), e     */ self.rl_index_indirect_wz(offset, WideRegister::IY, Some(Register::E), bus),
            0x14 => /* rl (iy+*), h     */ self.rl_index_indirect_wz(offset, WideRegister::IY, Some(Register::H), bus),
            0x15 => /* rl (iy+*), l     */ self.rl_index_indirect_wz(offset, WideRegister::IY, Some(Register::L), bus),
            0x16 => /* rl (iy+*)        */ self.rl_index_indirect_wz(offset, WideRegister::IY, None, bus),
            0x17 => /* rl (iy+*), a     */ self.rl_index_indirect_wz(offset, WideRegister::IY, Some(Register::A), bus),
            0x18 => /* rr (iy+*), b     */ self.rr_index_indirect_wz(offset, WideRegister::IY, Some(Register::B), bus),
            0x19 => /* rr (iy+*), c     */ self.rr_index_indirect_wz(offset, WideRegister::IY, Some(Register::C), bus),
            0x1A => /* rr (iy+*), d     */ self.rr_index_indirect_wz(offset, WideRegister::IY, Some(Register::D), bus),
            0x1B => /* rr (iy+*), e     */ self.rr_index_indirect_wz(offset, WideRegister::IY, Some(Register::E), bus),
            0x1C => /* rr (iy+*), h     */ self.rr_index_indirect_wz(offset, WideRegister::IY, Some(Register::H), bus),
            0x1D => /* rr (iy+*), l     */ self.rr_index_indirect_wz(offset, WideRegister::IY, Some(Register::L), bus),
            0x1E => /* rr (iy+*)        */ self.rr_index_indirect_wz(offset, WideRegister::IY, None, bus),
            0x1F => /* rr (iy+*), a     */ self.rr_index_indirect_wz(offset, WideRegister::IY, Some(Register::A), bus),

            0x20 => /* sla (iy+*), b    */ self.sla_index_indirect_wz(offset, WideRegister::IY, Some(Register::B), bus),
            0x21 => /* sla (iy+*), c    */ self.sla_index_indirect_wz(offset, WideRegister::IY, Some(Register::C), bus),
            0x22 => /* sla (iy+*), d    */ self.sla_index_indirect_wz(offset, WideRegister::IY, Some(Register::D), bus),
            0x23 => /* sla (iy+*), e    */ self.sla_index_indirect_wz(offset, WideRegister::IY, Some(Register::E), bus),
            0x24 => /* sla (iy+*), h    */ self.sla_index_indirect_wz(offset, WideRegister::IY, Some(Register::H), bus),
            0x25 => /* sla (iy+*), l    */ self.sla_index_indirect_wz(offset, WideRegister::IY, Some(Register::L), bus),
            0x26 => /* sla (iy+*)       */ self.sla_index_indirect_wz(offset, WideRegister::IY, None, bus),
            0x27 => /* sla (iy+*), a    */ self.sla_index_indirect_wz(offset, WideRegister::IY, Some(Register::A), bus),
            0x28 => /* sra (iy+*), b    */ self.sra_index_indirect_wz(offset, WideRegister::IY, Some(Register::B), bus),
            0x29 => /* sra (iy+*), c    */ self.sra_index_indirect_wz(offset, WideRegister::IY, Some(Register::C), bus),
            0x2A => /* sra (iy+*), d    */ self.sra_index_indirect_wz(offset, WideRegister::IY, Some(Register::D), bus),
            0x2B => /* sra (iy+*), e    */ self.sra_index_indirect_wz(offset, WideRegister::IY, Some(Register::E), bus),
            0x2C => /* sra (iy+*), h    */ self.sra_index_indirect_wz(offset, WideRegister::IY, Some(Register::H), bus),
            0x2D => /* sra (iy+*), l    */ self.sra_index_indirect_wz(offset, WideRegister::IY, Some(Register::L), bus),
            0x2E => /* sra (iy+*)       */ self.sra_index_indirect_wz(offset, WideRegister::IY, None, bus),
            0x2F => /* sra (iy+*), a    */ self.sra_index_indirect_wz(offset, WideRegister::IY, Some(Register::A), bus),

            0x30 => /* sll (iy+*), b    */ self.sll_index_indirect_wz(offset, WideRegister::IY, Some(Register::B), bus),
            0x31 => /* sll (iy+*), c    */ self.sll_index_indirect_wz(offset, WideRegister::IY, Some(Register::C), bus),
            0x32 => /* sll (iy+*), d    */ self.sll_index_indirect_wz(offset, WideRegister::IY, Some(Register::D), bus),
            0x33 => /* sll (iy+*), e    */ self.sll_index_indirect_wz(offset, WideRegister::IY, Some(Register::E), bus),
            0x34 => /* sll (iy+*), h    */ self.sll_index_indirect_wz(offset, WideRegister::IY, Some(Register::H), bus),
            0x35 => /* sll (iy+*), l    */ self.sll_index_indirect_wz(offset, WideRegister::IY, Some(Register::L), bus),
            0x36 => /* sll (iy+*)       */ self.sll_index_indirect_wz(offset, WideRegister::IY, None, bus),
            0x37 => /* sll (iy+*), a    */ self.sll_index_indirect_wz(offset, WideRegister::IY, Some(Register::A), bus),
            0x38 => /* srl (iy+*), b    */ self.srl_index_indirect_wz(offset, WideRegister::IY, Some(Register::B), bus),
            0x39 => /* srl (iy+*), c    */ self.srl_index_indirect_wz(offset, WideRegister::IY, Some(Register::C), bus),
            0x3A => /* srl (iy+*), d    */ self.srl_index_indirect_wz(offset, WideRegister::IY, Some(Register::D), bus),
            0x3B => /* srl (iy+*), e    */ self.srl_index_indirect_wz(offset, WideRegister::IY, Some(Register::E), bus),
            0x3C => /* srl (iy+*), h    */ self.srl_index_indirect_wz(offset, WideRegister::IY, Some(Register::H), bus),
            0x3D => /* srl (iy+*), l    */ self.srl_index_indirect_wz(offset, WideRegister::IY, Some(Register::L), bus),
            0x3E => /* srl (iy+*)       */ self.srl_index_indirect_wz(offset, WideRegister::IY, None, bus),
            0x3F => /* srl (iy+*), a    */ self.srl_index_indirect_wz(offset, WideRegister::IY, Some(Register::A), bus),

            0x40 => /* bit 0, (iy+*)    */ self.bit_index_indirect_wz(0x01, offset, WideRegister::IY, bus),
            0x41 => /* bit 0, (iy+*)    */ self.bit_index_indirect_wz(0x01, offset, WideRegister::IY, bus),
            0x42 => /* bit 0, (iy+*)    */ self.bit_index_indirect_wz(0x01, offset, WideRegister::IY, bus),
            0x43 => /* bit 0, (iy+*)    */ self.bit_index_indirect_wz(0x01, offset, WideRegister::IY, bus),
            0x44 => /* bit 0, (iy+*)    */ self.bit_index_indirect_wz(0x01, offset, WideRegister::IY, bus),
            0x45 => /* bit 0, (iy+*)    */ self.bit_index_indirect_wz(0x01, offset, WideRegister::IY, bus),
            0x46 => /* bit 0, (iy+*)    */ self.bit_index_indirect_wz(0x01, offset, WideRegister::IY, bus),
            0x47 => /* bit 0, (iy+*)    */ self.bit_index_indirect_wz(0x01, offset, WideRegister::IY, bus),
            0x48 => /* bit 1, (iy+*)    */ self.bit_index_indirect_wz(0x02, offset, WideRegister::IY, bus),
            0x49 => /* bit 1, (iy+*)    */ self.bit_index_indirect_wz(0x02, offset, WideRegister::IY, bus),
            0x4A => /* bit 1, (iy+*)    */ self.bit_index_indirect_wz(0x02, offset, WideRegister::IY, bus),
            0x4B => /* bit 1, (iy+*)    */ self.bit_index_indirect_wz(0x02, offset, WideRegister::IY, bus),
            0x4C => /* bit 1, (iy+*)    */ self.bit_index_indirect_wz(0x02, offset, WideRegister::IY, bus),
            0x4D => /* bit 1, (iy+*)    */ self.bit_index_indirect_wz(0x02, offset, WideRegister::IY, bus),
            0x4E => /* bit 1, (iy+*)    */ self.bit_index_indirect_wz(0x02, offset, WideRegister::IY, bus),
            0x4F => /* bit 1, (iy+*)    */ self.bit_index_indirect_wz(0x02, offset, WideRegister::IY, bus),

            0x50 => /* bit 2, (iy+*)    */ self.bit_index_indirect_wz(0x04, offset, WideRegister::IY, bus),
            0x51 => /* bit 2, (iy+*)    */ self.bit_index_indirect_wz(0x04, offset, WideRegister::IY, bus),
            0x52 => /* bit 2, (iy+*)    */ self.bit_index_indirect_wz(0x04, offset, WideRegister::IY, bus),
            0x53 => /* bit 2, (iy+*)    */ self.bit_index_indirect_wz(0x04, offset, WideRegister::IY, bus),
            0x54 => /* bit 2, (iy+*)    */ self.bit_index_indirect_wz(0x04, offset, WideRegister::IY, bus),
            0x55 => /* bit 2, (iy+*)    */ self.bit_index_indirect_wz(0x04, offset, WideRegister::IY, bus),
            0x56 => /* bit 2, (iy+*)    */ self.bit_index_indirect_wz(0x04, offset, WideRegister::IY, bus),
            0x57 => /* bit 2, (iy+*)    */ self.bit_index_indirect_wz(0x04, offset, WideRegister::IY, bus),
            0x58 => /* bit 3, (iy+*)    */ self.bit_index_indirect_wz(0x08, offset, WideRegister::IY, bus),
            0x59 => /* bit 3, (iy+*)    */ self.bit_index_indirect_wz(0x08, offset, WideRegister::IY, bus),
            0x5A => /* bit 3, (iy+*)    */ self.bit_index_indirect_wz(0x08, offset, WideRegister::IY, bus),
            0x5B => /* bit 3, (iy+*)    */ self.bit_index_indirect_wz(0x08, offset, WideRegister::IY, bus),
            0x5C => /* bit 3, (iy+*)    */ self.bit_index_indirect_wz(0x08, offset, WideRegister::IY, bus),
            0x5D => /* bit 3, (iy+*)    */ self.bit_index_indirect_wz(0x08, offset, WideRegister::IY, bus),
            0x5E => /* bit 3, (iy+*)    */ self.bit_index_indirect_wz(0x08, offset, WideRegister::IY, bus),
            0x5F => /* bit 3, (iy+*)    */ self.bit_index_indirect_wz(0x08, offset, WideRegister::IY, bus),

            0x60 => /* bit 4, (iy+*)    */ self.bit_index_indirect_wz(0x10, offset, WideRegister::IY, bus),
            0x61 => /* bit 4, (iy+*)    */ self.bit_index_indirect_wz(0x10, offset, WideRegister::IY, bus),
            0x62 => /* bit 4, (iy+*)    */ self.bit_index_indirect_wz(0x10, offset, WideRegister::IY, bus),
            0x63 => /* bit 4, (iy+*)    */ self.bit_index_indirect_wz(0x10, offset, WideRegister::IY, bus),
            0x64 => /* bit 4, (iy+*)    */ self.bit_index_indirect_wz(0x10, offset, WideRegister::IY, bus),
            0x65 => /* bit 4, (iy+*)    */ self.bit_index_indirect_wz(0x10, offset, WideRegister::IY, bus),
            0x66 => /* bit 4, (iy+*)    */ self.bit_index_indirect_wz(0x10, offset, WideRegister::IY, bus),
            0x67 => /* bit 4, (iy+*)    */ self.bit_index_indirect_wz(0x10, offset, WideRegister::IY, bus),
            0x68 => /* bit 5, (iy+*)    */ self.bit_index_indirect_wz(0x20, offset, WideRegister::IY, bus),
            0x69 => /* bit 5, (iy+*)    */ self.bit_index_indirect_wz(0x20, offset, WideRegister::IY, bus),
            0x6A => /* bit 5, (iy+*)    */ self.bit_index_indirect_wz(0x20, offset, WideRegister::IY, bus),
            0x6B => /* bit 5, (iy+*)    */ self.bit_index_indirect_wz(0x20, offset, WideRegister::IY, bus),
            0x6C => /* bit 5, (iy+*)    */ self.bit_index_indirect_wz(0x20, offset, WideRegister::IY, bus),
            0x6D => /* bit 5, (iy+*)    */ self.bit_index_indirect_wz(0x20, offset, WideRegister::IY, bus),
            0x6E => /* bit 5, (iy+*)    */ self.bit_index_indirect_wz(0x20, offset, WideRegister::IY, bus),
            0x6F => /* bit 5, (iy+*)    */ self.bit_index_indirect_wz(0x20, offset, WideRegister::IY, bus),

            0x70 => /* bit 6, (iy+*)    */ self.bit_index_indirect_wz(0x40, offset, WideRegister::IY, bus),
            0x71 => /* bit 6, (iy+*)    */ self.bit_index_indirect_wz(0x40, offset, WideRegister::IY, bus),
            0x72 => /* bit 6, (iy+*)    */ self.bit_index_indirect_wz(0x40, offset, WideRegister::IY, bus),
            0x73 => /* bit 6, (iy+*)    */ self.bit_index_indirect_wz(0x40, offset, WideRegister::IY, bus),
            0x74 => /* bit 6, (iy+*)    */ self.bit_index_indirect_wz(0x40, offset, WideRegister::IY, bus),
            0x75 => /* bit 6, (iy+*)    */ self.bit_index_indirect_wz(0x40, offset, WideRegister::IY, bus),
            0x76 => /* bit 6, (iy+*)    */ self.bit_index_indirect_wz(0x40, offset, WideRegister::IY, bus),
            0x77 => /* bit 6, (iy+*)    */ self.bit_index_indirect_wz(0x40, offset, WideRegister::IY, bus),
            0x78 => /* bit 7, (iy+*)    */ self.bit_index_indirect_wz(0x80, offset, WideRegister::IY, bus),
            0x79 => /* bit 7, (iy+*)    */ self.bit_index_indirect_wz(0x80, offset, WideRegister::IY, bus),
            0x7A => /* bit 7, (iy+*)    */ self.bit_index_indirect_wz(0x80, offset, WideRegister::IY, bus),
            0x7B => /* bit 7, (iy+*)    */ self.bit_index_indirect_wz(0x80, offset, WideRegister::IY, bus),
            0x7C => /* bit 7, (iy+*)    */ self.bit_index_indirect_wz(0x80, offset, WideRegister::IY, bus),
            0x7D => /* bit 7, (iy+*)    */ self.bit_index_indirect_wz(0x80, offset, WideRegister::IY, bus),
            0x7E => /* bit 7, (iy+*)    */ self.bit_index_indirect_wz(0x80, offset, WideRegister::IY, bus),
            0x7F => /* bit 7, (iy+*)    */ self.bit_index_indirect_wz(0x80, offset, WideRegister::IY, bus),

            0x80 => /* res 0, (iy+*), b */ self.reset_bit_index_indirect_wz(0x01, offset, WideRegister::IY, Some(Register::B), bus),
            0x81 => /* res 0, (iy+*), c */ self.reset_bit_index_indirect_wz(0x01, offset, WideRegister::IY, Some(Register::C), bus),
            0x82 => /* res 0, (iy+*), d */ self.reset_bit_index_indirect_wz(0x01, offset, WideRegister::IY, Some(Register::D), bus),
            0x83 => /* res 0, (iy+*), e */ self.reset_bit_index_indirect_wz(0x01, offset, WideRegister::IY, Some(Register::E), bus),
            0x84 => /* res 0, (iy+*), h */ self.reset_bit_index_indirect_wz(0x01, offset, WideRegister::IY, Some(Register::H), bus),
            0x85 => /* res 0, (iy+*), l */ self.reset_bit_index_indirect_wz(0x01, offset, WideRegister::IY, Some(Register::L), bus),
            0x86 => /* res 0, (iy+*)    */ self.reset_bit_index_indirect_wz(0x01, offset, WideRegister::IY, None, bus),
            0x87 => /* res 0, (iy+*), a */ self.reset_bit_index_indirect_wz(0x01, offset, WideRegister::IY, Some(Register::A), bus),
            0x88 => /* res 1, (iy+*), b */ self.reset_bit_index_indirect_wz(0x02, offset, WideRegister::IY, Some(Register::B), bus),
            0x89 => /* res 1, (iy+*), c */ self.reset_bit_index_indirect_wz(0x02, offset, WideRegister::IY, Some(Register::C), bus),
            0x8A => /* res 1, (iy+*), d */ self.reset_bit_index_indirect_wz(0x02, offset, WideRegister::IY, Some(Register::D), bus),
            0x8B => /* res 1, (iy+*), e */ self.reset_bit_index_indirect_wz(0x02, offset, WideRegister::IY, Some(Register::E), bus),
            0x8C => /* res 1, (iy+*), h */ self.reset_bit_index_indirect_wz(0x02, offset, WideRegister::IY, Some(Register::H), bus),
            0x8D => /* res 1, (iy+*), l */ self.reset_bit_index_indirect_wz(0x02, offset, WideRegister::IY, Some(Register::L), bus),
            0x8E => /* res 1, (iy+*)    */ self.reset_bit_index_indirect_wz(0x02, offset, WideRegister::IY, None, bus),
            0x8F => /* res 1, (iy+*), a */ self.reset_bit_index_indirect_wz(0x02, offset, WideRegister::IY, Some(Register::A), bus),

            0x90 => /* res 2, (iy+*), b */ self.reset_bit_index_indirect_wz(0x04, offset, WideRegister::IY, Some(Register::B), bus),
            0x91 => /* res 2, (iy+*), c */ self.reset_bit_index_indirect_wz(0x04, offset, WideRegister::IY, Some(Register::C), bus),
            0x92 => /* res 2, (iy+*), d */ self.reset_bit_index_indirect_wz(0x04, offset, WideRegister::IY, Some(Register::D), bus),
            0x93 => /* res 2, (iy+*), e */ self.reset_bit_index_indirect_wz(0x04, offset, WideRegister::IY, Some(Register::E), bus),
            0x94 => /* res 2, (iy+*), h */ self.reset_bit_index_indirect_wz(0x04, offset, WideRegister::IY, Some(Register::H), bus),
            0x95 => /* res 2, (iy+*), l */ self.reset_bit_index_indirect_wz(0x04, offset, WideRegister::IY, Some(Register::L), bus),
            0x96 => /* res 2, (iy+*)    */ self.reset_bit_index_indirect_wz(0x04, offset, WideRegister::IY, None, bus),
            0x97 => /* res 2, (iy+*), a */ self.reset_bit_index_indirect_wz(0x04, offset, WideRegister::IY, Some(Register::A), bus),
            0x98 => /* res 3, (iy+*), b */ self.reset_bit_index_indirect_wz(0x08, offset, WideRegister::IY, Some(Register::B), bus),
            0x99 => /* res 3, (iy+*), c */ self.reset_bit_index_indirect_wz(0x08, offset, WideRegister::IY, Some(Register::C), bus),
            0x9A => /* res 3, (iy+*), d */ self.reset_bit_index_indirect_wz(0x08, offset, WideRegister::IY, Some(Register::D), bus),
            0x9B => /* res 3, (iy+*), e */ self.reset_bit_index_indirect_wz(0x08, offset, WideRegister::IY, Some(Register::E), bus),
            0x9C => /* res 3, (iy+*), h */ self.reset_bit_index_indirect_wz(0x08, offset, WideRegister::IY, Some(Register::H), bus),
            0x9D => /* res 3, (iy+*), l */ self.reset_bit_index_indirect_wz(0x08, offset, WideRegister::IY, Some(Register::L), bus),
            0x9E => /* res 3, (iy+*)    */ self.reset_bit_index_indirect_wz(0x08, offset, WideRegister::IY, None, bus),
            0x9F => /* res 3, (iy+*), a */ self.reset_bit_index_indirect_wz(0x08, offset, WideRegister::IY, Some(Register::A), bus),

            0xA0 => /* res 4, (iy+*), b */ self.reset_bit_index_indirect_wz(0x10, offset, WideRegister::IY, Some(Register::B), bus),
            0xA1 => /* res 4, (iy+*), c */ self.reset_bit_index_indirect_wz(0x10, offset, WideRegister::IY, Some(Register::C), bus),
            0xA2 => /* res 4, (iy+*), d */ self.reset_bit_index_indirect_wz(0x10, offset, WideRegister::IY, Some(Register::D), bus),
            0xA3 => /* res 4, (iy+*), e */ self.reset_bit_index_indirect_wz(0x10, offset, WideRegister::IY, Some(Register::E), bus),
            0xA4 => /* res 4, (iy+*), h */ self.reset_bit_index_indirect_wz(0x10, offset, WideRegister::IY, Some(Register::H), bus),
            0xA5 => /* res 4, (iy+*), l */ self.reset_bit_index_indirect_wz(0x10, offset, WideRegister::IY, Some(Register::L), bus),
            0xA6 => /* res 4, (iy+*)    */ self.reset_bit_index_indirect_wz(0x10, offset, WideRegister::IY, None, bus),
            0xA7 => /* res 4, (iy+*), a */ self.reset_bit_index_indirect_wz(0x10, offset, WideRegister::IY, Some(Register::A), bus),
            0xA8 => /* res 5, (iy+*), b */ self.reset_bit_index_indirect_wz(0x20, offset, WideRegister::IY, Some(Register::B), bus),
            0xA9 => /* res 5, (iy+*), c */ self.reset_bit_index_indirect_wz(0x20, offset, WideRegister::IY, Some(Register::C), bus),
            0xAA => /* res 5, (iy+*), d */ self.reset_bit_index_indirect_wz(0x20, offset, WideRegister::IY, Some(Register::D), bus),
            0xAB => /* res 5, (iy+*), e */ self.reset_bit_index_indirect_wz(0x20, offset, WideRegister::IY, Some(Register::E), bus),
            0xAC => /* res 5, (iy+*), h */ self.reset_bit_index_indirect_wz(0x20, offset, WideRegister::IY, Some(Register::H), bus),
            0xAD => /* res 5, (iy+*), l */ self.reset_bit_index_indirect_wz(0x20, offset, WideRegister::IY, Some(Register::L), bus),
            0xAE => /* res 5, (iy+*)    */ self.reset_bit_index_indirect_wz(0x20, offset, WideRegister::IY, None, bus),
            0xAF => /* res 5, (iy+*), a */ self.reset_bit_index_indirect_wz(0x20, offset, WideRegister::IY, Some(Register::A), bus),

            0xB0 => /* res 6, (iy+*), b */ self.reset_bit_index_indirect_wz(0x40, offset, WideRegister::IY, Some(Register::B), bus),
            0xB1 => /* res 6, (iy+*), c */ self.reset_bit_index_indirect_wz(0x40, offset, WideRegister::IY, Some(Register::C), bus),
            0xB2 => /* res 6, (iy+*), d */ self.reset_bit_index_indirect_wz(0x40, offset, WideRegister::IY, Some(Register::D), bus),
            0xB3 => /* res 6, (iy+*), e */ self.reset_bit_index_indirect_wz(0x40, offset, WideRegister::IY, Some(Register::E), bus),
            0xB4 => /* res 6, (iy+*), h */ self.reset_bit_index_indirect_wz(0x40, offset, WideRegister::IY, Some(Register::H), bus),
            0xB5 => /* res 6, (iy+*), l */ self.reset_bit_index_indirect_wz(0x40, offset, WideRegister::IY, Some(Register::L), bus),
            0xB6 => /* res 6, (iy+*)    */ self.reset_bit_index_indirect_wz(0x40, offset, WideRegister::IY, None, bus),
            0xB7 => /* res 6, (iy+*), a */ self.reset_bit_index_indirect_wz(0x40, offset, WideRegister::IY, Some(Register::A), bus),
            0xB8 => /* res 7, (iy+*), b */ self.reset_bit_index_indirect_wz(0x80, offset, WideRegister::IY, Some(Register::B), bus),
            0xB9 => /* res 7, (iy+*), c */ self.reset_bit_index_indirect_wz(0x80, offset, WideRegister::IY, Some(Register::C), bus),
            0xBA => /* res 7, (iy+*), d */ self.reset_bit_index_indirect_wz(0x80, offset, WideRegister::IY, Some(Register::D), bus),
            0xBB => /* res 7, (iy+*), e */ self.reset_bit_index_indirect_wz(0x80, offset, WideRegister::IY, Some(Register::E), bus),
            0xBC => /* res 7, (iy+*), h */ self.reset_bit_index_indirect_wz(0x80, offset, WideRegister::IY, Some(Register::H), bus),
            0xBD => /* res 7, (iy+*), l */ self.reset_bit_index_indirect_wz(0x80, offset, WideRegister::IY, Some(Register::L), bus),
            0xBE => /* res 7, (iy+*)    */ self.reset_bit_index_indirect_wz(0x80, offset, WideRegister::IY, None, bus),
            0xBF => /* res 7, (iy+*), a */ self.reset_bit_index_indirect_wz(0x80, offset, WideRegister::IY, Some(Register::A), bus),

            0xC0 => /* set 0, (iy+*), b */ self.set_bit_index_indirect_wz(0x01, offset, WideRegister::IY, Some(Register::B), bus),
            0xC1 => /* set 0, (iy+*), c */ self.set_bit_index_indirect_wz(0x01, offset, WideRegister::IY, Some(Register::C), bus),
            0xC2 => /* set 0, (iy+*), d */ self.set_bit_index_indirect_wz(0x01, offset, WideRegister::IY, Some(Register::D), bus),
            0xC3 => /* set 0, (iy+*), e */ self.set_bit_index_indirect_wz(0x01, offset, WideRegister::IY, Some(Register::E), bus),
            0xC4 => /* set 0, (iy+*), h */ self.set_bit_index_indirect_wz(0x01, offset, WideRegister::IY, Some(Register::H), bus),
            0xC5 => /* set 0, (iy+*), l */ self.set_bit_index_indirect_wz(0x01, offset, WideRegister::IY, Some(Register::L), bus),
            0xC6 => /* set 0, (iy+*)    */ self.set_bit_index_indirect_wz(0x01, offset, WideRegister::IY, None, bus),
            0xC7 => /* set 0, (iy+*), a */ self.set_bit_index_indirect_wz(0x01, offset, WideRegister::IY, Some(Register::A), bus),
            0xC8 => /* set 1, (iy+*), b */ self.set_bit_index_indirect_wz(0x02, offset, WideRegister::IY, Some(Register::B), bus),
            0xC9 => /* set 1, (iy+*), c */ self.set_bit_index_indirect_wz(0x02, offset, WideRegister::IY, Some(Register::C), bus),
            0xCA => /* set 1, (iy+*), d */ self.set_bit_index_indirect_wz(0x02, offset, WideRegister::IY, Some(Register::D), bus),
            0xCB => /* set 1, (iy+*), e */ self.set_bit_index_indirect_wz(0x02, offset, WideRegister::IY, Some(Register::E), bus),
            0xCC => /* set 1, (iy+*), h */ self.set_bit_index_indirect_wz(0x02, offset, WideRegister::IY, Some(Register::H), bus),
            0xCD => /* set 1, (iy+*), l */ self.set_bit_index_indirect_wz(0x02, offset, WideRegister::IY, Some(Register::L), bus),
            0xCE => /* set 1, (iy+*)    */ self.set_bit_index_indirect_wz(0x02, offset, WideRegister::IY, None, bus),
            0xCF => /* set 1, (iy+*), a */ self.set_bit_index_indirect_wz(0x02, offset, WideRegister::IY, Some(Register::A), bus),

            0xD0 => /* set 2, (iy+*), b */ self.set_bit_index_indirect_wz(0x04, offset, WideRegister::IY, Some(Register::B), bus),
            0xD1 => /* set 2, (iy+*), c */ self.set_bit_index_indirect_wz(0x04, offset, WideRegister::IY, Some(Register::C), bus),
            0xD2 => /* set 2, (iy+*), d */ self.set_bit_index_indirect_wz(0x04, offset, WideRegister::IY, Some(Register::D), bus),
            0xD3 => /* set 2, (iy+*), e */ self.set_bit_index_indirect_wz(0x04, offset, WideRegister::IY, Some(Register::E), bus),
            0xD4 => /* set 2, (iy+*), h */ self.set_bit_index_indirect_wz(0x04, offset, WideRegister::IY, Some(Register::H), bus),
            0xD5 => /* set 2, (iy+*), l */ self.set_bit_index_indirect_wz(0x04, offset, WideRegister::IY, Some(Register::L), bus),
            0xD6 => /* set 2, (iy+*)    */ self.set_bit_index_indirect_wz(0x04, offset, WideRegister::IY, None, bus),
            0xD7 => /* set 2, (iy+*), a */ self.set_bit_index_indirect_wz(0x04, offset, WideRegister::IY, Some(Register::A), bus),
            0xD8 => /* set 3, (iy+*), b */ self.set_bit_index_indirect_wz(0x08, offset, WideRegister::IY, Some(Register::B), bus),
            0xD9 => /* set 3, (iy+*), c */ self.set_bit_index_indirect_wz(0x08, offset, WideRegister::IY, Some(Register::C), bus),
            0xDA => /* set 3, (iy+*), d */ self.set_bit_index_indirect_wz(0x08, offset, WideRegister::IY, Some(Register::D), bus),
            0xDB => /* set 3, (iy+*), e */ self.set_bit_index_indirect_wz(0x08, offset, WideRegister::IY, Some(Register::E), bus),
            0xDC => /* set 3, (iy+*), h */ self.set_bit_index_indirect_wz(0x08, offset, WideRegister::IY, Some(Register::H), bus),
            0xDD => /* set 3, (iy+*), l */ self.set_bit_index_indirect_wz(0x08, offset, WideRegister::IY, Some(Register::L), bus),
            0xDE => /* set 3, (iy+*)    */ self.set_bit_index_indirect_wz(0x08, offset, WideRegister::IY, None, bus),
            0xDF => /* set 3, (iy+*), a */ self.set_bit_index_indirect_wz(0x08, offset, WideRegister::IY, Some(Register::A), bus),

            0xE0 => /* set 4, (iy+*), b */ self.set_bit_index_indirect_wz(0x10, offset, WideRegister::IY, Some(Register::B), bus),
            0xE1 => /* set 4, (iy+*), c */ self.set_bit_index_indirect_wz(0x10, offset, WideRegister::IY, Some(Register::C), bus),
            0xE2 => /* set 4, (iy+*), d */ self.set_bit_index_indirect_wz(0x10, offset, WideRegister::IY, Some(Register::D), bus),
            0xE3 => /* set 4, (iy+*), e */ self.set_bit_index_indirect_wz(0x10, offset, WideRegister::IY, Some(Register::E), bus),
            0xE4 => /* set 4, (iy+*), h */ self.set_bit_index_indirect_wz(0x10, offset, WideRegister::IY, Some(Register::H), bus),
            0xE5 => /* set 4, (iy+*), l */ self.set_bit_index_indirect_wz(0x10, offset, WideRegister::IY, Some(Register::L), bus),
            0xE6 => /* set 4, (iy+*)    */ self.set_bit_index_indirect_wz(0x10, offset, WideRegister::IY, None, bus),
            0xE7 => /* set 4, (iy+*), a */ self.set_bit_index_indirect_wz(0x10, offset, WideRegister::IY, Some(Register::A), bus),
            0xE8 => /* set 5, (iy+*), b */ self.set_bit_index_indirect_wz(0x20, offset, WideRegister::IY, Some(Register::B), bus),
            0xE9 => /* set 5, (iy+*), c */ self.set_bit_index_indirect_wz(0x20, offset, WideRegister::IY, Some(Register::C), bus),
            0xEA => /* set 5, (iy+*), d */ self.set_bit_index_indirect_wz(0x20, offset, WideRegister::IY, Some(Register::D), bus),
            0xEB => /* set 5, (iy+*), e */ self.set_bit_index_indirect_wz(0x20, offset, WideRegister::IY, Some(Register::E), bus),
            0xEC => /* set 5, (iy+*), h */ self.set_bit_index_indirect_wz(0x20, offset, WideRegister::IY, Some(Register::H), bus),
            0xED => /* set 5, (iy+*), l */ self.set_bit_index_indirect_wz(0x20, offset, WideRegister::IY, Some(Register::L), bus),
            0xEE => /* set 5, (iy+*)    */ self.set_bit_index_indirect_wz(0x20, offset, WideRegister::IY, None, bus),
            0xEF => /* set 5, (iy+*), a */ self.set_bit_index_indirect_wz(0x20, offset, WideRegister::IY, Some(Register::A), bus),

            0xF0 => /* set 6, (iy+*), b */ self.set_bit_index_indirect_wz(0x40, offset, WideRegister::IY, Some(Register::B), bus),
            0xF1 => /* set 6, (iy+*), c */ self.set_bit_index_indirect_wz(0x40, offset, WideRegister::IY, Some(Register::C), bus),
            0xF2 => /* set 6, (iy+*), d */ self.set_bit_index_indirect_wz(0x40, offset, WideRegister::IY, Some(Register::D), bus),
            0xF3 => /* set 6, (iy+*), e */ self.set_bit_index_indirect_wz(0x40, offset, WideRegister::IY, Some(Register::E), bus),
            0xF4 => /* set 6, (iy+*), h */ self.set_bit_index_indirect_wz(0x40, offset, WideRegister::IY, Some(Register::H), bus),
            0xF5 => /* set 6, (iy+*), l */ self.set_bit_index_indirect_wz(0x40, offset, WideRegister::IY, Some(Register::L), bus),
            0xF6 => /* set 6, (iy+*)    */ self.set_bit_index_indirect_wz(0x40, offset, WideRegister::IY, None, bus),
            0xF7 => /* set 6, (iy+*), a */ self.set_bit_index_indirect_wz(0x40, offset, WideRegister::IY, Some(Register::A), bus),
            0xF8 => /* set 7, (iy+*), b */ self.set_bit_index_indirect_wz(0x80, offset, WideRegister::IY, Some(Register::B), bus),
            0xF9 => /* set 7, (iy+*), c */ self.set_bit_index_indirect_wz(0x80, offset, WideRegister::IY, Some(Register::C), bus),
            0xFA => /* set 7, (iy+*), d */ self.set_bit_index_indirect_wz(0x80, offset, WideRegister::IY, Some(Register::D), bus),
            0xFB => /* set 7, (iy+*), e */ self.set_bit_index_indirect_wz(0x80, offset, WideRegister::IY, Some(Register::E), bus),
            0xFC => /* set 7, (iy+*), h */ self.set_bit_index_indirect_wz(0x80, offset, WideRegister::IY, Some(Register::H), bus),
            0xFD => /* set 7, (iy+*), l */ self.set_bit_index_indirect_wz(0x80, offset, WideRegister::IY, Some(Register::L), bus),
            0xFE => /* set 7, (iy+*)    */ self.set_bit_index_indirect_wz(0x80, offset, WideRegister::IY, None, bus),
            0xFF => /* set 7, (iy+*), a */ self.set_bit_index_indirect_wz(0x80, offset, WideRegister::IY, Some(Register::A), bus),
        })
    }
}
