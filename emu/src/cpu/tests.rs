use std::time::{Duration, Instant, SystemTime};
use super::*;

const ZEXDOC: (&'static str, &'static [u8]) = ("zexdoc", include_bytes!("zexdoc.com"));
const ZEXALL: (&'static str, &'static [u8]) = ("zexall", include_bytes!("zexall.com"));

impl Bus for Vec<u8> {
    fn read(&mut self, addr: u16) -> u8 {
        self[addr as usize]
    }

    fn write(&mut self, addr: u16, data: u8) {
        self[addr as usize] = data;
    }

    fn input(&mut self, port: u16) -> u8 {
        self[port as usize]
    }

    fn output(&mut self, port: u16, data: u8) {
        self[port as usize] = data;
    }
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
        let mut bus = vec![0u8; 65536];
        for (i, b) in test.iter().enumerate() {
            bus[0x100 + i] = *b;
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
    let mut bus = vec![
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
    let mut cpu = Cpu::default();
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x0001, cpu.ir);
    assert_eq!(0x0001, cpu.pc);
}

#[test]
fn read_wide_immediate() {
    #[rustfmt::skip]
    let mut bus = vec![
        0x01, 0x34, 0x12,                               // ld bc, $1234
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
    let mut cpu = Cpu::default();
    assert_eq!(10, cpu.step(&mut bus));
    assert_eq!(0x0001, cpu.ir);
    assert_eq!(0x0003, cpu.pc);
    assert_eq!(0x1234, cpu.bc);
}

#[test]
fn write_indirect() {
    #[rustfmt::skip]
    let mut bus = vec![
        0x3E, 0x42,                                     // ld a, $42
        0x01, 0x01, 0x00,                               // ld bc, $0001
        0x02,                                           // ld (bc), a
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
    let mut cpu = Cpu::default();
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x4200, cpu.af);
    assert_eq!(10, cpu.step(&mut bus));
    assert_eq!(7, cpu.step(&mut bus));
    assert_eq!(0x42, bus[0x0001]);
}

#[test]
fn inc_wide() {
    #[rustfmt::skip]
    let mut bus = vec![
        0x01, 0x01, 0x00,                               // ld bc, $0001
        0x03,                                           // inc bc
        0x01, 0xFF, 0xFF,                               // ld bc, $ffff
        0x03,                                           // inc bc
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
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
    let mut bus = vec![
        0x3E, 0x0F,                                     // ld a, $0f
        0x3C,                                           // inc a
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
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
    let mut bus = vec![
        0x3E, 0x7F,                                     // ld a, $7f
        0x3C,                                           // inc a
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
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
    let mut bus = vec![
        0x3E, 0xFF,                                     // ld a, $ff
        0x3C,                                           // inc a
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
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
    let mut bus = vec![
        0x3E, 0x10,                                     // ld a, $10
        0x3D,                                           // dec a
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
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
    let mut bus = vec![
        0x3E, 0x80,                                     // ld a, $80
        0x3D,                                           // dec a
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
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
    let mut bus = vec![
        0x3E, 0x00,                                     // ld a, $00
        0x3D,                                           // inc a
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
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
    let mut bus = vec![
        0x3E, 0x80,                                     // ld a, $80
        0x07,                                           // rlca
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
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
    let mut bus = vec![
        0x3E, 0x80,                                     // ld a, $80
        0x07,                                           // rlca
        0x07,                                           // rlca
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
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
    let mut bus = vec![
        0x21, 0xFF, 0xFF,                               // ld hl, $ffff
        0xE5,                                           // push hl
        0xF1,                                           // pop af
        0x08,                                           // ex af, af'
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
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
    let mut bus = vec![
        0x21, 0xFF, 0x0F,                               // ld hl, $0fff
        0x01, 0x01, 0x00,                               // ld bc, 1
        0x09,                                           // add hl, bc
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
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
    let mut bus = vec![
        0x3E, 0x01,                                     // ld a, $01
        0x0F,                                           // rrca
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
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
    let mut bus = vec![
        0x3E, 0x01,                                     // ld a, $01
        0x0F,                                           // rrca
        0x0F,                                           // rrca
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
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
        let mut bus = vec![
        0x00,                                           // nop
        0x18, 0x04,                                     // jr +4
        0x00,                                           // nop
        0x00,                                           // nop
        0x00,                                           // nop
        0x00,                                           // nop
        0x18, 0xF7,                                     // jr -9
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
    let mut cpu = Cpu::default();
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(12, cpu.step(&mut bus));
    assert_eq!(12, cpu.step(&mut bus));
    assert_eq!(0x0000, cpu.pc);
}

#[test]
fn add_immediate() {
    #[rustfmt::skip]
    let mut bus = vec![
        0x3E, 0x0F,                                     // ld a, $0f
        0xC6, 0x01,                                     // add a, 1
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
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
    let mut bus = vec![
        0x3E, 0xFF,                                     // ld a, $ff
        0xC6, 0x01,                                     // add a, 1
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
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
    let mut bus = vec![
        0x3E, 0x7F,                                     // ld a, $7f
        0xC6, 0x7F,                                     // add a, $7f
        0x00,                                           // nop
    ];
    bus.resize(65536, 0);
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