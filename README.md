# Possum Computer

```                                     
MM"""""""`YM                                                               
MM  mmmmm  M                                                               
M'        .M .d8888b. .d8888b. .d8888b. dP    dP 88d8b.d8b.                
MM  MMMMMMMM 88'  `88 Y8ooooo. Y8ooooo. 88    88 88'`88'`88                
MM  MMMMMMMM 88.  .88       88       88 88.  .88 88  88  88                
MM  MMMMMMMM `88888P' `88888P' `88888P' `88888P' dP  dP  dP                
MMMMMMMMMMMM                                                               
                                                                           
MM'""""'YMM                                         dP                     
M' .mmm. `M                                         88                     
M  MMMMMooM .d8888b. 88d8b.d8b. 88d888b. dP    dP d8888P .d8888b. 88d888b. 
M  MMMMMMMM 88'  `88 88'`88'`88 88'  `88 88    88   88   88ooood8 88'  `88 
M. `MMM' .M 88.  .88 88  88  88 88.  .88 88.  .88   88   88.  ... 88       
MM.     .dM `88888P' dP  dP  dP 88Y888P' `88888P'   dP   `88888P' dP       
MMMMMMMMMMM                     88                                         
                                dP                                            
```

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
- [ ] z8410 DMA: *usable (though not used yet since the timing emulation is a little sketchy)*
- [ ] z80 SIO: *not started*
- [ ] z80 CTC: *not started*
- [X] 8-bit ATA drive(s)*
- [ ] MOS 8563 VDC**: *nearly complete, very usable*

\**2 disk images can be mounted on the ATA bus.
The interface is Compact Flash actually, but it is 80s tech.*

\*\**The 80-column display chip from the venerable Commodore 128!
16KB video RAM installed ;-)*

### Software

- [ ] Monitor ROM with disk driver that can load the kernel: *WIP*
- [ ] Filesystem (let's make a custom one!): *WIP*
- [ ] Fuse driver for host access to the mount the filesystem
- [ ] Banked memory-mapped IO
- [ ] Preemptive multitasking (yes, really)
- [ ] IPC
- [ ] CP/M-compatability mode (a dream)
- [ ] Text editor (vim-like)
- [ ] Assembler
- [ ] SLIP ethernet driver
- [ ] TCP/IP stack
- [ ] ???

## What's working

```
z80asm ../rom/hello_hd.z80 -o ../rom/hello_hd.bin

cargo run --release --bin possum-emu -- ../rom/hello_hd.bin --hd0 ../img/blank.img
```

```
Model Name:	POSSUM-CF-CARD-EMULATOR-01
Serial #:	0-12345-67890-123456
Disk Size:	$00000080
Hello! Successfully wrote to the disk!
```
