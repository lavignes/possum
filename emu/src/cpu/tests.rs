use super::*;

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

#[test]
fn nop() {
    #[rustfmt::skip]
    let mut bus = vec![
        0x00,                                           // nop
    ];
    let mut cpu = Cpu::default();
    assert_eq!(4, cpu.step(&mut bus));
    assert_eq!(0x0100, cpu.ir);
    assert_eq!(0x0001, cpu.pc);
}

#[test]
fn read_wide_immediate() {
    #[rustfmt::skip]
    let mut bus = vec![
        0x01, 0x34, 0x12,                               // ld bc, $1234
        0x00,                                           // nop
    ];
    let mut cpu = Cpu::default();
    assert_eq!(10, cpu.step(&mut bus));
    assert_eq!(0x0100, cpu.ir);
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
        0x01, 0xFF, 0xFF,                               // ld bc, $FFFF
        0x03,                                           // inc bc
        0x00,                                           // nop
    ];
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
        0x3E, 0x0F,                                     // ld a, $0F
        0x3C,                                           // inc a
        0x00,                                           // nop
    ];
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
        0x3E, 0x7F,                                     // ld a, $7F
        0x3C,                                           // inc a
        0x00,                                           // nop
    ];
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
        0x3E, 0xFF,                                     // ld a, $FF
        0x3C,                                           // inc a
        0x00,                                           // nop
    ];
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