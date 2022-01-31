use std::time::{Instant};
use crate::bus::TestBus;
use super::*;

const ZEXDOC: (&'static str, &'static [u8]) = ("zexdoc", include_bytes!("zexdoc.com"));
const ZEXALL: (&'static str, &'static [u8]) = ("zexall", include_bytes!("zexall.com"));

#[inline]
fn flags(cpu: &Cpu, flags: &[Flag]) -> bool {
    let mut flag = 0;
    for f in flags {
        flag |= *f as u8;
    }
    // mask off x and y
    (((cpu.af as u8) & !((Flag::X as u8) | (Flag::Y as u8))) as u8) == flag
}

fn bios_call(cpu: &mut Cpu, bus: &mut impl Bus) {
    match cpu.register(Register::C) {
        2 => print!("{}", cpu.register(Register::E) as char),
        9 => {
            let mut addr = cpu.wide_register(WideRegister::DE);
            loop {
                let c = bus.read(addr) as char;
                addr = addr.carrying_add(1, false).0;
                if c == '$' {
                    break;
                }
                print!("{}", c);
            }
        }
        c => unimplemented!("Unexpected syscall: {}", c),
    }
    cpu.return_wz(bus);
}

#[test]
fn zextests() {
    for (name, test) in [ZEXDOC, ZEXALL] {
        println!("zextest \"{name}\":");
        let mut bus = TestBus::new();
        for (i, b) in test.iter().enumerate() {
            bus.mem_mut()[0x100 + i] = *b;
        }
        let mut cpu = Cpu::default();
        cpu.pc = 0x0100;
        cpu.sp = 0xF000;
        let mut cycles = 0;
        let start = Instant::now();
        loop {
            cycles += cpu.step(&mut bus);
            match cpu.pc {
                0x0000 => break,
                0x0005 => bios_call(&mut cpu, &mut bus),
                _ => {},
            }
        }
        let duration = Instant::now().duration_since(start).as_nanos() as usize;
        let hz = ((cycles as f64) * 1_000_000_000.0) / (duration as f64);
        let mhz = hz / 1_000_000.0;
        println!("Duration: {duration}ns");
        println!("Cycles: {cycles} ({mhz:.03} MHz)");
    }
}

#[test]
fn nop() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x0001, cpu.ir);
    assert_eq!(0x0001, cpu.pc);
}

#[test]
fn read_wide_immediate() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x01, 0x34, 0x12,                               // ld bc, $1234
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(10, cpu.step(&mut bus));
    assert_eq!(0x0001, cpu.ir);
    assert_eq!(0x0003, cpu.pc);
    assert_eq!(0x1234, cpu.bc);
}

#[test]
fn write_indirect() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x42,                                     // ld a, $42
        0x01, 0x01, 0x00,                               // ld bc, $0001
        0x02,                                           // ld (bc), a
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x4200, cpu.af);
    assert_eq!(10, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x42, bus.mem()[0x0001]);
}

#[test]
fn inc_wide() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x01, 0x01, 0x00,                               // ld bc, $0001
        0x03,                                           // inc bc
        0x01, 0xFF, 0xFF,                               // ld bc, $ffff
        0x03,                                           // inc bc
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(10, cpu.step(&mut bus));
    assert_eq!(6, cpu.step(&mut bus));
    assert_eq!(0x0002, cpu.bc);
    assert_eq!(10, cpu.step(&mut bus));
    assert_eq!(6, cpu.step(&mut bus));
    assert_eq!(0x0000, cpu.bc);
}

#[test]
fn inc() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x0F,                                     // ld a, $0f
        0x3C,                                           // inc a
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x0F, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x10, cpu.register(Register::A));
    assert!(!cpu.flag(Flag::C));
    assert!(!cpu.flag(Flag::N));
    assert!(!cpu.flag(Flag::PV));
    assert!(!cpu.flag(Flag::X));
    assert!(cpu.flag(Flag::H));
    assert!(!cpu.flag(Flag::Y));
    assert!(!cpu.flag(Flag::Z));
    assert!(!cpu.flag(Flag::S));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x7F,                                     // ld a, $7f
        0x3C,                                           // inc a
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x7F, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x80, cpu.register(Register::A));
    assert!(!cpu.flag(Flag::C));
    assert!(!cpu.flag(Flag::N));
    assert!(cpu.flag(Flag::PV));
    assert!(!cpu.flag(Flag::X));
    assert!(cpu.flag(Flag::H));
    assert!(!cpu.flag(Flag::Y));
    assert!(!cpu.flag(Flag::Z));
    assert!(cpu.flag(Flag::S));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0xFF,                                     // ld a, $ff
        0x3C,                                           // inc a
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0xFF, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::A));
    assert!(!cpu.flag(Flag::C));
    assert!(!cpu.flag(Flag::N));
    assert!(!cpu.flag(Flag::PV));
    assert!(!cpu.flag(Flag::X));
    assert!(cpu.flag(Flag::H));
    assert!(!cpu.flag(Flag::Y));
    assert!(cpu.flag(Flag::Z));
    assert!(!cpu.flag(Flag::S));
}

#[test]
fn dec() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x10,                                     // ld a, $10
        0x3D,                                           // dec a
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x10, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x0F, cpu.register(Register::A));
    assert!(!cpu.flag(Flag::C));
    assert!(cpu.flag(Flag::N));
    assert!(!cpu.flag(Flag::PV));
    assert!(cpu.flag(Flag::X));
    assert!(cpu.flag(Flag::H));
    assert!(!cpu.flag(Flag::Y));
    assert!(!cpu.flag(Flag::Z));
    assert!(!cpu.flag(Flag::S));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x80,                                     // ld a, $80
        0x3D,                                           // dec a
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x80, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x7F, cpu.register(Register::A));
    assert!(!cpu.flag(Flag::C));
    assert!(cpu.flag(Flag::N));
    assert!(cpu.flag(Flag::PV));
    assert!(cpu.flag(Flag::X));
    assert!(cpu.flag(Flag::H));
    assert!(cpu.flag(Flag::Y));
    assert!(!cpu.flag(Flag::Z));
    assert!(!cpu.flag(Flag::S));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x00,                                     // ld a, $00
        0x3D,                                           // inc a
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0xFF, cpu.register(Register::A));
    assert!(!cpu.flag(Flag::C));
    assert!(cpu.flag(Flag::N));
    assert!(!cpu.flag(Flag::PV));
    assert!(cpu.flag(Flag::X));
    assert!(cpu.flag(Flag::H));
    assert!(cpu.flag(Flag::Y));
    assert!(!cpu.flag(Flag::Z));
    assert!(cpu.flag(Flag::S));
}

#[test]
fn rlca() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x80,                                     // ld a, $80
        0x07,                                           // rlca
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x80, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x01, cpu.register(Register::A));
    assert!(cpu.flag(Flag::C));
    assert!(!cpu.flag(Flag::N));
    assert!(!cpu.flag(Flag::PV));
    assert!(!cpu.flag(Flag::X));
    assert!(!cpu.flag(Flag::H));
    assert!(!cpu.flag(Flag::Y));
    assert!(!cpu.flag(Flag::Z));
    assert!(!cpu.flag(Flag::S));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x80,                                     // ld a, $80
        0x07,                                           // rlca
        0x07,                                           // rlca
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x80, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x01, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x02, cpu.register(Register::A));
    assert!(!cpu.flag(Flag::C));
    assert!(!cpu.flag(Flag::N));
    assert!(!cpu.flag(Flag::PV));
    assert!(!cpu.flag(Flag::X));
    assert!(!cpu.flag(Flag::H));
    assert!(!cpu.flag(Flag::Y));
    assert!(!cpu.flag(Flag::Z));
    assert!(!cpu.flag(Flag::S));
}

#[test]
fn exchange() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x21, 0xFF, 0xFF,                               // ld hl, $ffff
        0xE5,                                           // push hl
        0xF1,                                           // pop af
        0x08,                                           // ex af, af'
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(10, cpu.step(&mut bus));
    assert_eq!(0xFFFF, cpu.hl);
    assert_eq!(11, cpu.step(&mut bus));
    assert_eq!(10, cpu.step(&mut bus));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x0000, cpu.af);
    assert_eq!(0xFFFF, cpu.af_prime);
}

#[test]
fn add_wide() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x21, 0xFF, 0x0F,                               // ld hl, $0fff
        0x01, 0x01, 0x00,                               // ld bc, 1
        0x09,                                           // add hl, bc
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(10, cpu.step(&mut bus));
    assert_eq!(0x0FFF, cpu.hl);
    assert_eq!(10, cpu.step(&mut bus));
    assert_eq!(0x0001, cpu.bc);
    assert_eq!(11, cpu.step(&mut bus));
    assert_eq!(0x1000, cpu.hl);
    assert!(!cpu.flag(Flag::C));
    assert!(!cpu.flag(Flag::N));
    assert!(!cpu.flag(Flag::PV));
    assert!(!cpu.flag(Flag::X));
    assert!(cpu.flag(Flag::H));
    assert!(!cpu.flag(Flag::Y));
    assert!(!cpu.flag(Flag::Z));
    assert!(!cpu.flag(Flag::S));
}

#[test]
fn rrca() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x01,                                     // ld a, $01
        0x0F,                                           // rrca
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x01, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x80, cpu.register(Register::A));
    assert!(cpu.flag(Flag::C));
    assert!(!cpu.flag(Flag::N));
    assert!(!cpu.flag(Flag::PV));
    assert!(!cpu.flag(Flag::X));
    assert!(!cpu.flag(Flag::H));
    assert!(!cpu.flag(Flag::Y));
    assert!(!cpu.flag(Flag::Z));
    assert!(!cpu.flag(Flag::S));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x01,                                     // ld a, $01
        0x0F,                                           // rrca
        0x0F,                                           // rrca
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x01, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x80, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x40, cpu.register(Register::A));
    assert!(!cpu.flag(Flag::C));
    assert!(!cpu.flag(Flag::N));
    assert!(!cpu.flag(Flag::PV));
    assert!(!cpu.flag(Flag::X));
    assert!(!cpu.flag(Flag::H));
    assert!(!cpu.flag(Flag::Y));
    assert!(!cpu.flag(Flag::Z));
    assert!(!cpu.flag(Flag::S));
}

#[test]
fn jr() {
    #[rustfmt::skip]
        let mut bus = TestBus::with_mem(vec![
        0x00,                                           // nop
        0x18, 0x04,                                     // jr +4
        0x00,                                           // nop
        0x00,                                           // nop
        0x00,                                           // nop
        0x00,                                           // nop
        0x18, 0xF7,                                     // jr -9
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(12, cpu.step(&mut bus));
    assert_eq!(12, cpu.step(&mut bus));
    assert_eq!(0x0000, cpu.pc);
}

#[test]
fn add_immediate() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x0F,                                     // ld a, $0f
        0xC6, 0x01,                                     // add a, 1
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x0F, cpu.register(Register::A));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x10, cpu.register(Register::A));
    assert!(!cpu.flag(Flag::C));
    assert!(!cpu.flag(Flag::N));
    assert!(!cpu.flag(Flag::PV));
    assert!(!cpu.flag(Flag::X));
    assert!(cpu.flag(Flag::H));
    assert!(!cpu.flag(Flag::Y));
    assert!(!cpu.flag(Flag::Z));
    assert!(!cpu.flag(Flag::S));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0xFF,                                     // ld a, $ff
        0xC6, 0x01,                                     // add a, 1
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0xFF, cpu.register(Register::A));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::A));
    assert!(cpu.flag(Flag::C));
    assert!(!cpu.flag(Flag::N));
    assert!(!cpu.flag(Flag::PV));
    assert!(!cpu.flag(Flag::X));
    assert!(cpu.flag(Flag::H));
    assert!(!cpu.flag(Flag::Y));
    assert!(cpu.flag(Flag::Z));
    assert!(!cpu.flag(Flag::S));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x7F,                                     // ld a, $7f
        0xC6, 0x7F,                                     // add a, $7f
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x7F, cpu.register(Register::A));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0xFE, cpu.register(Register::A));
    assert!(!cpu.flag(Flag::C));
    assert!(!cpu.flag(Flag::N));
    assert!(cpu.flag(Flag::PV));
    assert!(cpu.flag(Flag::X));
    assert!(cpu.flag(Flag::H));
    assert!(cpu.flag(Flag::Y));
    assert!(!cpu.flag(Flag::Z));
    assert!(cpu.flag(Flag::S));
}

#[test]
fn rotates() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x01,                                     // ld a, $01
        0x06, 0xFF,                                     // ld b, $ff
        0x0E, 0x03,                                     // ld c, $03
        0x16, 0xFE,                                     // ld d, $fe
        0x1E, 0x11,                                     // ld e, $11
        0x26, 0x3F,                                     // ld h, $3f
        0x2E, 0x70,                                     // ld l, $70
        0xCB, 0x0F,                                     // rrc a
        0xCB, 0x07,                                     // rlc a
        0xCB, 0x08,                                     // rrc b
        0xCB, 0x00,                                     // rlc b
        0xCB, 0x01,                                     // rlc c
        0xCB, 0x09,                                     // rrc c
        0xCB, 0x02,                                     // rrc d
        0xCB, 0x0A,                                     // rlc d
        0xCB, 0x0B,                                     // rrc e
        0xCB, 0x03,                                     // rlc e
        0xCB, 0x04,                                     // rlc h
        0xCB, 0x0C,                                     // rrc h
        0xCB, 0x05,                                     // rlc l
        0xCB, 0x0D,                                     // rrc l
        0xCB, 0x1F,                                     // rr a
        0xCB, 0x17,                                     // rl a
        0xCB, 0x18,                                     // rr b
        0xCB, 0x10,                                     // rl b
        0xCB, 0x11,                                     // rr c
        0xCB, 0x19,                                     // rl c
        0xCB, 0x12,                                     // rr d
        0xCB, 0x1A,                                     // rl d
        0xCB, 0x1B,                                     // rr e
        0xCB, 0x13,                                     // rl e
        0xCB, 0x14,                                     // rl h
        0xCB, 0x1C,                                     // rr h
        0xCB, 0x15,                                     // rl l
        0xCB, 0x1D,                                     // rr l
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x80, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::S, Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x01, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0xFF, cpu.register(Register::B));
    assert!(flags(&cpu, &[Flag::S, Flag::PV, Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0xFF, cpu.register(Register::B));
    assert!(flags(&cpu, &[Flag::S, Flag::PV, Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x06, cpu.register(Register::C));
    assert!(flags(&cpu, &[Flag::PV]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x03, cpu.register(Register::C));
    assert!(flags(&cpu, &[Flag::PV]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0xFD, cpu.register(Register::D));
    assert!(flags(&cpu, &[Flag::S, Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0xFE, cpu.register(Register::D));
    assert!(flags(&cpu, &[Flag::S, Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x88, cpu.register(Register::E));
    assert!(flags(&cpu, &[Flag::S, Flag::PV, Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x11, cpu.register(Register::E));
    assert!(flags(&cpu, &[Flag::PV, Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x7E, cpu.register(Register::H));
    assert!(flags(&cpu, &[Flag::PV]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x3F, cpu.register(Register::H));
    assert!(flags(&cpu, &[Flag::PV]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0xE0, cpu.register(Register::L));
    assert!(flags(&cpu, &[Flag::S]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x70, cpu.register(Register::L));
    assert!(flags(&cpu, &[]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::Z, Flag::PV, Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x01, cpu.register(Register::A));
    assert!(flags(&cpu, &[]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x7F, cpu.register(Register::B));
    assert!(flags(&cpu, &[Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0xFF, cpu.register(Register::B));
    assert!(flags(&cpu, &[Flag::S, Flag::PV]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x06, cpu.register(Register::C));
    assert!(flags(&cpu, &[Flag::PV]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x03, cpu.register(Register::C));
    assert!(flags(&cpu, &[Flag::PV]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0xFC, cpu.register(Register::D));
    assert!(flags(&cpu, &[Flag::S, Flag::PV, Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0xFE, cpu.register(Register::D));
    assert!(flags(&cpu, &[Flag::S]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x08, cpu.register(Register::E));
    assert!(flags(&cpu, &[Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x11, cpu.register(Register::E));
    assert!(flags(&cpu, &[Flag::PV]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x7E, cpu.register(Register::H));
    assert!(flags(&cpu, &[Flag::PV]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x3F, cpu.register(Register::H));
    assert!(flags(&cpu, &[Flag::PV]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0xE0, cpu.register(Register::L));
    assert!(flags(&cpu, &[Flag::S]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x70, cpu.register(Register::L));
    assert!(flags(&cpu, &[]));
}

#[test]
fn sla() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x01,                                     // ld a, $01
        0x06, 0x80,                                     // ld b, $80
        0x0E, 0xAA,                                     // ld c, $aa
        0x16, 0xFE,                                     // ld d, $fe
        0x1E, 0x7F,                                     // ld e, $7f
        0x26, 0x11,                                     // ld h, $11
        0x2E, 0x00,                                     // ld l, $00
        0xCB, 0x27,                                     // sla a
        0xCB, 0x20,                                     // sla b
        0xCB, 0x21,                                     // sla c
        0xCB, 0x22,                                     // sla d
        0xCB, 0x23,                                     // sla e
        0xCB, 0x24,                                     // sla h
        0xCB, 0x25,                                     // sla l
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x02, cpu.register(Register::A));
    assert!(flags(&cpu, &[]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::B));
    assert!(flags(&cpu, &[Flag::Z, Flag::PV, Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x54, cpu.register(Register::C));
    assert!(flags(&cpu, &[Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0xFC, cpu.register(Register::D));
    assert!(flags(&cpu, &[Flag::S, Flag::PV, Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0xFE, cpu.register(Register::E));
    assert!(flags(&cpu, &[Flag::S]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x22, cpu.register(Register::H));
    assert!(flags(&cpu, &[Flag::PV]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::L));
    assert!(flags(&cpu, &[Flag::Z, Flag::PV]));
}

#[test]
fn sla_wide() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x21, 0x00, 0x10,                               // ld hl, $1000
        0xDD, 0x21, 0x00, 0x10,                         // ld ix, $1001
        0xFD, 0x21, 0x03, 0x10,                         // ld iy, $1003
        0xCB, 0x26,                                     // sla (hl)
        0x7E,                                           // ld a, (hl)
        0xDD, 0xCB, 0x01, 0x26,                         // sla (ix+1)
        0xDD, 0x7E, 0x01,                               // ld a, (ix+1)
        0xFD, 0xCB, 0xFF, 0x26,                         // sla (iy-1)
        0xFD, 0x7E, 0xFF,                               // ld a, (iy-1)
        0x00,                                           // nop
    ]);
    bus.mem_mut()[0x1000] = 0x01;
    bus.mem_mut()[0x1001] = 0x80;
    bus.mem_mut()[0x1002] = 0xAA;
    let mut cpu = Cpu::default();
    assert_eq!(10, cpu.step(&mut bus));
    assert_eq!(14, cpu.step(&mut bus));
    assert_eq!(14, cpu.step(&mut bus));
    assert_eq!(15, cpu.step(&mut bus));
    assert_eq!(0x02, bus.mem()[0x1000]);
    assert!(flags(&cpu, &[]));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x02, cpu.register(Register::A));
    assert_eq!(23, cpu.step(&mut bus));
    assert_eq!(0x00, bus.mem()[0x1001]);
    assert!(flags(&cpu, &[Flag::Z, Flag::PV, Flag::C]));
    assert_eq!(19, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::A));
    assert_eq!(23, cpu.step(&mut bus));
    assert_eq!(0x54, bus.mem()[0x1002]);
    assert!(flags(&cpu, &[Flag::C]));
    assert_eq!(19, cpu.step(&mut bus));
    assert_eq!(0x54, cpu.register(Register::A));
}

#[test]
fn sra() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x01,                                     // ld a, $01
        0x06, 0x80,                                     // ld b, $80
        0x0E, 0xAA,                                     // ld c, $aa
        0x16, 0xFE,                                     // ld d, $fe
        0x1E, 0x7F,                                     // ld e, $7f
        0x26, 0x11,                                     // ld h, $11
        0x2E, 0x00,                                     // ld l, $00
        0xCB, 0x2F,                                     // sra a
        0xCB, 0x28,                                     // sra b
        0xCB, 0x29,                                     // sra c
        0xCB, 0x2A,                                     // sra d
        0xCB, 0x2B,                                     // sra e
        0xCB, 0x2C,                                     // sra h
        0xCB, 0x2D,                                     // sra l
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::Z, Flag::PV, Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0xC0, cpu.register(Register::B));
    assert!(flags(&cpu, &[Flag::S, Flag::PV]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0xD5, cpu.register(Register::C));
    assert!(flags(&cpu, &[Flag::S]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0xFF, cpu.register(Register::D));
    assert!(flags(&cpu, &[Flag::S, Flag::PV]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x3F, cpu.register(Register::E));
    assert!(flags(&cpu, &[Flag::PV, Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x08, cpu.register(Register::H));
    assert!(flags(&cpu, &[Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::L));
    assert!(flags(&cpu, &[Flag::Z, Flag::PV]));
}

#[test]
fn srl() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x01,                                     // ld a, $01
        0x06, 0x80,                                     // ld b, $80
        0x0E, 0xAA,                                     // ld c, $aa
        0x16, 0xFE,                                     // ld d, $fe
        0x1E, 0x7F,                                     // ld e, $7f
        0x26, 0x11,                                     // ld h, $11
        0x2E, 0x00,                                     // ld l, $00
        0xCB, 0x3F,                                     // srl a
        0xCB, 0x38,                                     // srl b
        0xCB, 0x39,                                     // srl c
        0xCB, 0x3A,                                     // srl d
        0xCB, 0x3B,                                     // srl e
        0xCB, 0x3C,                                     // srl h
        0xCB, 0x3D,                                     // srl l
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::Z, Flag::PV, Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x40, cpu.register(Register::B));
    assert!(flags(&cpu, &[]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x55, cpu.register(Register::C));
    assert!(flags(&cpu, &[Flag::PV]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x7F, cpu.register(Register::D));
    assert!(flags(&cpu, &[]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x3F, cpu.register(Register::E));
    assert!(flags(&cpu, &[Flag::PV, Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x08, cpu.register(Register::H));
    assert!(flags(&cpu, &[Flag::C]));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::L));
    assert!(flags(&cpu, &[Flag::Z, Flag::PV]));
}

#[test]
fn daa() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x15,                                     // ld a, $15
        0x06, 0x27,                                     // ld b, $27
        0x80,                                           // add a, b
        0x27,                                           // daa
        0x90,                                           // sub b
        0x27,                                           // daa
        0x3E, 0x90,                                     // ld a, $90
        0x06, 0x15,                                     // ld b, $15
        0x80,                                           // add a, b
        0x27,                                           // daa
        0x90,                                           // sub b
        0x27,                                           // daa
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x3C, cpu.register(Register::A));
    assert!(flags(&cpu, &[]));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::H, Flag::PV]));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x1B, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::H, Flag::N]));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x15, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::N]));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0xA5, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::S]));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x05, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::PV, Flag::C]));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0xF0, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::S, Flag::N, Flag::C]));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x90, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::S, Flag::PV, Flag::N, Flag::C]));
}

#[test]
fn cpl() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x97,                                           // sub a
        0x2F,                                           // cpl
        0x2F,                                           // cpl
        0xC6, 0xAA,                                     // add a, $aa
        0x2F,                                           // cpl
        0x2F,                                           // cpl
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::Z, Flag::N]));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0xFF, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::Z, Flag::H, Flag::N]));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::Z, Flag::H, Flag::N]));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0xAA, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::S]));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x55, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::S, Flag::H, Flag::N]));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0xAA, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::S, Flag::H, Flag::N]));
}

#[test]
fn ccf_scf() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x97,                                           // sub a
        0x37,                                           // scf
        0x3F,                                           // ccf
        0xD6, 0xCC,                                     // sub $cc
        0x3F,                                           // ccf
        0x37,                                           // scf
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::Z, Flag::N]));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::Z, Flag::C]));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::Z, Flag::H]));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x34, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::H, Flag::N, Flag::C]));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x34, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::H]));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x34, cpu.register(Register::A));
    assert!(flags(&cpu, &[Flag::C]));
}