use std::time::Instant;

use super::*;
use crate::bus::TestBus;

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
                _ => {}
            }
        }
        let duration = Instant::now().duration_since(start).as_secs_f64() as usize;
        let hz = (cycles as f64) / (duration as f64);
        let mhz = hz / 1_000_000.0;
        println!("Duration: {duration:.03}s");
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

#[test]
fn register_loads() {
    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x3E, 0x42,                                     // ld a, $42
        0x7F,                                           // ld a, a
        0x47,                                           // ld b, a
        0x4F,                                           // ld c, a
        0x57,                                           // ld d, a
        0x5F,                                           // ld e, a
        0x67,                                           // ld h, a
        0x6F,                                           // ld l, a
        0xDD, 0x67,                                     // ld ixh, a
        0xDD, 0x6F,                                     // ld ixl, a
        0xFD, 0x67,                                     // ld iyh, a
        0xFD, 0x6F,                                     // ld iyl, a
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::B));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::C));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::D));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::E));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::H));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::L));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IXH));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IXL));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IYH));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IYL));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x06, 0x42,                                     // ld b, $42
        0x78,                                           // ld a, b
        0x40,                                           // ld b, b
        0x48,                                           // ld c, b
        0x50,                                           // ld d, b
        0x58,                                           // ld e, b
        0x60,                                           // ld h, b
        0x68,                                           // ld l, b
        0xDD, 0x60,                                     // ld ixh, b
        0xDD, 0x68,                                     // ld ixl, b
        0xFD, 0x60,                                     // ld iyh, b
        0xFD, 0x68,                                     // ld iyl, b
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::B));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::B));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::C));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::D));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::E));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::H));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::L));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IXH));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IXL));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IYH));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IYL));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x0E, 0x42,                                     // ld c, $42
        0x79,                                           // ld a, c
        0x41,                                           // ld b, c
        0x49,                                           // ld c, c
        0x51,                                           // ld d, c
        0x59,                                           // ld e, c
        0x61,                                           // ld h, c
        0x69,                                           // ld l, c
        0xDD, 0x61,                                     // ld ixh, c
        0xDD, 0x69,                                     // ld ixl, c
        0xFD, 0x61,                                     // ld iyh, c
        0xFD, 0x69,                                     // ld iyl, c
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::C));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::B));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::C));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::D));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::E));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::H));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::L));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IXH));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IXL));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IYH));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IYL));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x16, 0x42,                                     // ld d, $42
        0x7A,                                           // ld a, d
        0x42,                                           // ld b, d
        0x4A,                                           // ld c, d
        0x52,                                           // ld d, d
        0x5A,                                           // ld e, d
        0x62,                                           // ld h, d
        0x6A,                                           // ld l, d
        0xDD, 0x62,                                     // ld ixh, d
        0xDD, 0x6A,                                     // ld ixl, d
        0xFD, 0x62,                                     // ld iyh, d
        0xFD, 0x6A,                                     // ld iyl, d
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::D));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::B));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::C));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::D));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::E));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::H));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::L));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IXH));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IXL));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IYH));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IYL));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x1E, 0x42,                                     // ld e, $42
        0x7B,                                           // ld a, e
        0x43,                                           // ld b, e
        0x4B,                                           // ld c, e
        0x53,                                           // ld d, e
        0x5B,                                           // ld e, e
        0x63,                                           // ld h, e
        0x6B,                                           // ld l, e
        0xDD, 0x63,                                     // ld ixh, e
        0xDD, 0x6B,                                     // ld ixl, e
        0xFD, 0x63,                                     // ld iyh, e
        0xFD, 0x6B,                                     // ld iyl, e
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::E));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::B));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::C));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::D));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::E));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::H));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::L));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IXH));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IXL));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IYH));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IYL));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x26, 0x42,                                     // ld h, $42
        0x7C,                                           // ld a, h
        0x44,                                           // ld b, h
        0x4C,                                           // ld c, h
        0x54,                                           // ld d, h
        0x5C,                                           // ld e, h
        0x64,                                           // ld h, h
        0x6C,                                           // ld l, h
        0xDD, 0x64,                                     // ld ixh, ixh
        0xDD, 0x6C,                                     // ld ixl, ixh
        0xFD, 0x64,                                     // ld iyh, ixh
        0xFD, 0x6C,                                     // ld iyl, ixh
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::H));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::B));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::C));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::D));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::E));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::H));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::L));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::IXH));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::IXL));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::IYH));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::IYL));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0x2E, 0x42,                                     // ld l, $42
        0x7D,                                           // ld a, l
        0x45,                                           // ld b, l
        0x4D,                                           // ld c, l
        0x55,                                           // ld d, l
        0x5D,                                           // ld e, l
        0x65,                                           // ld h, l
        0x6D,                                           // ld l, l
        0xDD, 0x65,                                     // ld ixh, ixl
        0xDD, 0x6D,                                     // ld ixl, ixl
        0xFD, 0x65,                                     // ld iyh, ixl
        0xFD, 0x6D,                                     // ld iyl, ixl
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::L));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::A));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::B));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::C));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::D));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::E));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::H));
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::L));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::IXH));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::IXL));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::IYH));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x00, cpu.register(Register::IYL));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0xDD, 0x26, 0x42,                               // ld ixh, $42
        0xDD, 0x7C,                                     // ld a, ixh
        0xDD, 0x44,                                     // ld b, ixh
        0xDD, 0x4C,                                     // ld c, ixh
        0xDD, 0x54,                                     // ld d, ixh
        0xDD, 0x5C,                                     // ld e, ixh
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(11, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IXH));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::A));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::B));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::C));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::D));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::E));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0xDD, 0x2E, 0x42,                               // ld ixl, $42
        0xDD, 0x7D,                                     // ld a, ixl
        0xDD, 0x45,                                     // ld b, ixl
        0xDD, 0x4D,                                     // ld c, ixl
        0xDD, 0x55,                                     // ld d, ixl
        0xDD, 0x5D,                                     // ld e, ixl
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(11, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IXL));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::A));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::B));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::C));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::D));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::E));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0xFD, 0x26, 0x42,                               // ld iyh, $42
        0xFD, 0x7C,                                     // ld a, iyh
        0xFD, 0x44,                                     // ld b, iyh
        0xFD, 0x4C,                                     // ld c, iyh
        0xFD, 0x54,                                     // ld d, iyh
        0xFD, 0x5C,                                     // ld e, iyh
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(11, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IYH));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::A));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::B));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::C));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::D));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::E));

    #[rustfmt::skip]
    let mut bus = TestBus::with_mem(vec![
        0xFD, 0x2E, 0x42,                               // ld iyl, $42
        0xFD, 0x7D,                                     // ld a, iyl
        0xFD, 0x45,                                     // ld b, iyl
        0xFD, 0x4D,                                     // ld c, iyl
        0xFD, 0x55,                                     // ld d, iyl
        0xFD, 0x5D,                                     // ld e, iyl
        0x00,                                           // nop
    ]);
    let mut cpu = Cpu::default();
    assert_eq!(11, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::IYL));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::A));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::B));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::C));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::D));
    assert_eq!(8, cpu.step(&mut bus));
    assert_eq!(0x42, cpu.register(Register::E));
}
