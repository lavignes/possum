# Possum Computer

Possum is a z80-based 8-bit micro-computer emulator.

All peripherals are emulations of real-world hardware
that was available in the 80s (or roughly 80s-adjacent).

The intent of the project is mostly a challenge for me to
write as modern of an operating system and software for
the excruciatingly minimal hardware available. (i.e. fun)

Rather than building this computer physically with a bunch
of chips and a bread-board, the emulator stands as a more
accessible and debuggable means of developing for the
machine.

## Goals

### Hardware Emulation
 
- [ ] z80 CPU: *nearly complete, very usable*
- [ ] z8410 DMA: *usable*
- [ ] z80 SIO: *not started*
- [ ] z80 CTC: *not started*
- [ ] 8-bit ATA drive (Compact Flash technically, but it's 80s tech): *nearly complete*
- [ ] CGA-like graphics adapter (a stretch, probably. It's trivial to emulate though)

### Software

- [ ] Monitor ROM with disk driver that can load the kernel
- [ ] Filesystem (let's make a custom one!)
- [ ] Fuse driver for host access to the mount the filesystem
- [ ] Banked memory mapped IO
- [ ] Preemptive multitasking (yes, really)
- [ ] IPC
- [ ] CP/M-compatability mode (a dream)
- [ ] Text editor (vim-like)
- [ ] Assembler
- [ ] SLIP ethernet driver
- [ ] TCP/IP stack
- [ ] ???

