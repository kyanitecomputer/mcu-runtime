# AST2700 BootMCU Bring-Up Notes

Hardware-verified findings from Rust firmware execution on the
AST2700 BootMCU (lowRISC ibex RV32IMC) on AST2750-A1 silicon.

Validated: 2025-05-08 on AST2750 A1 DCSCM via SPI flash programmer.
Status: **Embassy async runtime + Caliptra mailbox fully operational.**

## Hardware overview

| Property | Value |
|----------|-------|
| CPU | lowRISC ibex, RV32IMC (no FPU, no MMU) |
| I-cache | Yes (small, implementation-defined) |
| D-cache | **None** (`CONFIG_SYS_DCACHE_OFF=y` in U-Boot) |
| Interrupt mode | **Vectored only** (ibex hardwires mtvec.MODE=1) |
| Clock (pre-PLL) | ~50 MHz (calibrated from busy-wait timing) |
| Clock (post-PLL) | Up to 200 MHz (HCLK-IO) |
| GSRAM | 192 KB usable at `0x14B8_0000` (256 KB total, upper region inaccessible) |
| ECC SRAM | 128 KB at `0x1000_0000` (system-die, available pre-DRAM) |
| SDRAM | 1 GB at `0x8000_0000` — requires BootMCU DRAM training |
| UART12 | `0x14C3_3B00`, 115200 8N1, ROM-configured and locked |
| Timer | `0x14C3_6000`, 1 MHz free-running 64-bit, IRQ 7 (MTIP) |
| Caliptra | `0x14C6_0000` (mailbox), `0x14C7_0000` (IFC) |
| SPI flash (FMC) | Memory-mapped at `0x2000_0000`, 512 MB XIP window |
| SDRAMMC | `0x12C0_0000`, DDR PHY at `0x1300_0000` |
| ROM version | `0x0204` (reported as `B  0204` on UART12) |

## Memory model (corrected)

The ROM copies the FMC binary from SPI flash into **GSRAM at 0x14B80A00**,
NOT SDRAM.  SDRAM at 0x80000000 is **not usable** until the BootMCU
performs DRAM training.  All firmware code and data must reside in SRAM.

Evidence:
- `ast2700_secure_boot_v0203.md`: "BootMCU ROM loads the SoC FMC from boot media to SRAM."
- `u-boot/configs/ibex-ast2700_defconfig`: `CONFIG_SPL_TEXT_BASE=0x14b80a00`
- `u-boot/arch/riscv/cpu/ast2700/u-boot-spl.lds`: all of .text/.rodata/.data in SRAM
- `zephyr/boards/.../ast2700_evb_ast2700_a1_bootmcu.overlay`: `sram: memory@14b80a00 { reg = <0x14b80a00 0x2ee00>; }`

### Memory map (BootMCU view, pre-DRAM training)

| Address | Size | Type | Notes |
|---------|------|------|-------|
| `0x1000_0000` | 128 KB | ECC SRAM | System-die. Available for DMA, heap. |
| `0x12C0_0000` | 4 KB | SDRAMMC regs | DRAM controller configuration |
| `0x1300_0000` | 512 KB | DDR PHY | PHY training firmware IMEM/DMEM |
| `0x14B8_0000` | 2560 B | GSRAM (reserved) | ASTH header placed by ROM |
| `0x14B8_0A00` | ~187 KB | GSRAM (usable) | FMC binary: .text+.rodata+.data+.bss+stack |
| `0x14C0_2000` | 8 KB | SCU1 | IO-die system control |
| `0x14C3_3B00` | 256 B | UART12 | ROM-configured 115200 8N1 (locked) |
| `0x14C3_6000` | 64 B | Timer | 1 MHz, 64-bit, vectored IRQ 7 |
| `0x14C6_0000` | 4 KB | Caliptra Mbox | Mailbox command interface |
| `0x14C7_0000` | 4 KB | Caliptra IFC | Boot flow, TRNG, fuse control |
| `0x2000_0000` | 512 MB | SPI FMC (XIP) | Read-only flash mapping |
| `0x8000_0000` | 1 GB | SDRAM | **Unavailable until DRAM training** |

### SRAM layout (A1 silicon)

```
0x14B80000 ┬─ ASTH header (2560 bytes, placed by ROM)
0x14B80A00 ├─ .text (FMC code, entry point)
           ├─ .rodata (read-only data, string literals)
           ├─ .data (mutable globals)
           ├─ .bss (zero-initialized globals)
           ├─ heap (currently unused)
           ├─ ↓ stack grows downward
0x14BAF800 └─ _stack_start (Zephyr A1 limit, end of 192 KB usable)
```

## SPI flash layout (AST2700 A1 — dev image)

```
Offset     Content                          Notes
─────────  ───────────────────────────────  ──────────────────────────────
0x00000000 Caliptra FW (CMAN manifest)      ~106 KB. ROM loads first.
0x00020000 ASTH header (2560 bytes)         version=2 for A1 silicon.
0x00020A00 FMC binary (BootMCU firmware)    ROM copies to SRAM 0x14B80A00.
0x0002D200 DDR4 PHY IMEM (prebuilt 0x01)   ~30 KB
0x000349E0 DDR4 PHY DMEM (prebuilt 0x02)   64 KB
0x000449E0 DDR4 2D PHY IMEM (prebuilt 0x03) ~30 KB
0x0004BEA0 DDR4 2D PHY DMEM (prebuilt 0x04) 64 KB
0x0005BEA0 DDR5 PHY IMEM (prebuilt 0x05)   ~53 KB
0x00068E00 DDR5 PHY DMEM (prebuilt 0x06)   64 KB
0x00078E00 DP FW (prebuilt 0x07)           16 KB
0x00800000 CA35 payload (TamaGo / ATF)     up to 4 MB window
0x00C10000 SSP payload (optional)          up to 128 KB
0x00C30000 TSP payload (optional)          up to 128 KB
```

Prebuilt binaries are sourced from `bmc-pb/ast2700a1/`.  The image is
built by `tools/gen-spi-image.py`.

## ROM boot sequence

```
B  0204     ROM version          Fc XXXX  Caliptra FW load (0000=OK)
T  0000     Timing               Cb 0000  Caliptra boot
Oi 0000     OTP info             Cr 0000  Caliptra ROM ver
Pf 0000     Platform flags       Cf 0000  Caliptra FW ver
Uu 0000     Strap upper          Ff XXXX  FMC load (0000=OK)
Uc 0000     Strap common         Sb XXXX  Secure boot (0000=OK)
...                              J  0000  Jump to FMC in SRAM
```

## Critical hardware constraints (all verified)

### 1. ROM copies FMC to SRAM, not SDRAM

The ROM copies the FMC binary to **GSRAM 0x14B80A00** and jumps there.
SDRAM at 0x80000000 is uninitialized DRAM that the BootMCU must train.

Linking at 0x80000000 appeared to work for trivial binaries (PC-relative
code + hardcoded MMIO addresses), but any absolute data reference
(vtables, function pointers, jump tables) pointed to uninitialized DRAM
→ silent crash.

### 2. ibex hardwires vectored interrupt mode

ibex ignores writes of `mtvec.MODE=0` (Direct) — it always reads back
as `MODE=1` (Vectored).  In vectored mode, interrupt cause N jumps to
`mtvec.BASE + 4*N`, not to `mtvec.BASE`.

Without a vector table, machine timer interrupt (cause 7) lands 28 bytes
into the trap handler (mid-instruction) → crash → silent hang.

Zephyr confirms: `select RISCV_VECTORED_MODE` + `GEN_IRQ_VECTOR_TABLE`
in `zephyr/soc/aspeed/ast27xx/Kconfig` for both A0 and A1 BootMCU.

**Fix**: 256-byte–aligned vector table where every entry is `j default_start_trap`.
Override `_setup_interrupts` to set mtvec in Vectored mode.

### 3. UART12 registers locked after ROM boot

ROM configures UART12 at 115200 8N1, then locks configuration registers:
- **THR (0x00) write**: works — transmit characters
- **LSR (0x14) read**: works — poll THRE bit for TX ready
- **LCR, FCR, IER writes**: kill the UART — no further output

**Fix**: `Uart::new_uart12()` skips `hw_init()`.  Uses THR/LSR only.

### 4. riscv-rt `_start` has interrupt-enable race

riscv-rt's `_start` in `.init.rust` opens with `lui ra / jr` before
`csrwi mie,0`.  If ROM left a pending interrupt, it fires through ROM's
mtvec.

**Fix**: `.section .init` stub executes `csrwi mie,0` as absolute first
instruction, before riscv-rt's `_start`.

### 5. Timer RESET_EN breaks monotonic clock

Timer CTRL bit 3 (`RESET_EN`) resets the counter to 0 on alarm match.
With Embassy's monotonic clock, the ISR reads `now ≈ 0` after every
alarm → thinks no timers have expired → never wakes tasks.

**Fix**: Clear `RESET_EN` in init.  Counter runs freely.  EN stays set
permanently — never toggle EN, just reprogram ALARM registers.

### 6. Timer CTRL register bits

| Bit | Name | Description |
|-----|------|-------------|
| 0 | EN | Timer enable (counting + interrupt). Single bit controls both. |
| 1 | (reserved) | Reads as 1 on AST2750-A1 hardware. |
| 3 | RESET_EN | Counter resets to 0 on alarm match. **Must be cleared for Embassy.** |
| 4 | COUNT_CLR | Self-clearing strobe: resets counter to 0. |
| 16 | INTR_STS | Interrupt status. Cleared by writing ALARM_L or ALARM_H. |

### 7. SRAM size is 192 KB (not 256 KB)

Datasheet says 256 KB at 0x14B80000.  Zephyr DTS limits to 192 KB
(ending at 0x14BAF800).  Stack above 0x14BAF800 causes store access faults.

### 8. BootMCU clock is ~50 MHz pre-PLL

9M busy-wait iterations ≈ 1 second.  HCLK-IO default = 200 MHz HPLL / 4.

### 9. UART boot stage-2 broken in ROM v02.04

All stage-2 attempts return `0606`.  SPI flash boot is the viable path.

## Caliptra boot sequence (hardware-verified)

The SoC ROM handles the complete Caliptra bring-up before jumping to FMC:
1. Assert CPTRA_PWRGOOD via SCU1_CPTRA_CTRL bit 0
2. Write hardware fuses over APB (UDS seed, field entropy, owner key hash, etc.)
3. Signal FUSE_WR_DONE
4. Upload Caliptra FW via FWLD mailbox command (from SPI flash prebuilt entries)
5. Serve TRNG entropy until Caliptra completes internal boot
6. Set SCU1_CPTRA_CTRL bit 18 (RDY_FOR_RT mirror) when runtime is live

**FMC arrives with Caliptra already in RDY_FOR_RT** — no handshake needed.

### Verified Caliptra state on first FMC instruction

```
FLOW_STS = 0x28000000   bit 29 = RDY_FOR_RT, bit 27 = runtime active
BOOT_STS = 0x00000600   Caliptra internal boot status
SCU1_RDY_RT = 1         SoC ROM mirror bit set
pl0_pauser = 0x21212121 ROM set all 5 MBOX PAUSER slots to 0x21 (BootMCU AXI ID)
```

### Verified Caliptra FW versions

```
rom_rev: 51ff0a89f169bbf8e06ad147...   Caliptra ROM git hash
fmc_rev: f7d557981ac80b67230d07d4...   Caliptra FMC+RT git hash
rt_rev:  f7d557981ac80b67230d07d4...   same as FMC (combined image)
mode:    0x46495053 = "FIPS"            FIPS mode active
name:    "Caliptra RTM"
rev:     0x06010011  0x00000840  0x00000000
```

### Caliptra driver (cptra.rs)

| Method | Description |
|--------|-------------|
| `is_rdy_for_rt()` | Checks FLOW_STS bit 29 + SCU1 bit 18 |
| `wait_rdy_for_rt_with_trng(fn, loops)` | Polls RDY_FOR_RT, feeds TRNG each loop |
| `fw_info()` | FW_INFO mailbox command |
| `capabilities()` | CAPABILITIES command |
| `fips_version()` | FIPS_VERSION command |
| `feed_trng(entropy)` | Write 12 words + set DATA_WR_DONE |

## Working firmware

### rot_ast2700_bootmcu (production)

Full bring-up sequence: WDT/EXTRST → SLI → SCU policy → DRAM → fabric →
load payloads → MPU/RVBAR → release CA35/SSP/TSP → maintenance loop.
- 51 KB FMC binary
- Hardware-verified on AST2750-A1 DCSCM
- See `app-rot/src/bin/rot_ast2700_bootmcu.rs`

### hello_uart_bootmcu (diagnostic)

Embassy async with 1 Hz timer ticks. Key components:
- `boot_rv.rs`: startup stub + vector table + `_setup_interrupts`
- `time_driver_rv.rs`: 64-bit timer at 0x14C36000, vectored IRQ 7
- `uart.rs`: `Uart::new_uart12()` (ROM-configured, no reinit)
- `ast2700-bootmcu.x`: single SRAM region at 0x14B80A00

### caliptra_hello_bootmcu (diagnostic)

Caliptra status query + FW_INFO/CAPABILITIES/FIPS_VERSION commands.
- Checks RDY_FOR_RT immediately (ROM has already completed boot)
- Keeps TRNG fed in async loop after queries

### Building and flashing

See `docs/bootmcu-next-steps.md` for the full build/image/flash command
sequence using the current toolchain and correct prebuilt paths.

## Tools

| Tool | Purpose |
|------|---------|
| `tools/gen-spi-image.py` | Build flashable SPI image (CMAN + ASTH + FMC) |
| `tools/gen-ast-fmc-image.py` | Build ASTH-wrapped FMC image |
| `tools/aspeed-uart-load/` | Multi-SoC UART loader (Go) with probe mode |
