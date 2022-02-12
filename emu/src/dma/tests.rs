use super::*;
use crate::bus::TestBus;

#[test]
fn simple_transfer() {
    let mut bus = TestBus::with_mem(vec![
        0x12, 0x34, 0x56, 0x78, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);
    let mut dma = Dma::default();

    dma.write(0, 0b0111_1101); // wr0: transfer a -> b
    dma.write(0, 0x00); // a address: 0x0000
    dma.write(0, 0x00); //
    dma.write(0, 0x05); // length: 5
    dma.write(0, 0x00); //

    dma.write(0, 0b0001_0100); // wr1: a is memory, increment

    dma.write(0, 0b0001_0000); // wr2: b is memory, increment

    dma.write(0, 0b1011_1101); // wr4: byte mode
    dma.write(0, 0x05); // b address: 0x0005
    dma.write(0, 0x00); //
    dma.write(0, 0b010_0010); // status affects vector, interrupt at end

    dma.write(0, 0xCF); // Load
    dma.write(0, 0xAB); // Enable interrupts
    dma.write(0, 0x87); // Enable DMA

    while !dma.interrupting() {
        dma.tick(&mut bus);
    }
    assert_eq!(0x06, dma.ack_interrupt());
    assert_eq!(bus.mem()[0..5], bus.mem()[5..10]); // !
}
