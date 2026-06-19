# AST2700 BootMCU — Next Steps

Status as of 2026-05: Full silicon bring-up complete and hardware-verified
on AST2750-A1 DCSCM.  DRAM, SLI, SSP, TSP, and CA35 release all confirmed
working.  Current production binary: `rot_ast2700_bootmcu`.

## Completed

| Item | Status |
|------|--------|
| Bare-metal UART12 tick from SPI flash | ✅ `spi_minimal_bootmcu` |
| riscv-rt startup (`.init` stub, `__pre_init`) | ✅ `spi_diag_bootmcu` |
| Correct SRAM-only linker script (0x14B80A00) | ✅ `ast2700-bootmcu.x` |
| Vectored interrupt table for ibex | ✅ `boot_rv.rs` |
| Embassy async executor | ✅ `hello_uart_bootmcu` |
| 64-bit timer driver (1 MHz, IRQ 7) | ✅ `time_driver_rv.rs` |
| UART12 TX driver (ROM-configured, no reinit) | ✅ `uart.rs` |
| Caliptra driver (mailbox, IFC, TRNG) | ✅ `cptra.rs` |
| Caliptra FW_INFO / CAPABILITIES / FIPS_VERSION | ✅ `caliptra_hello_bootmcu` (hardware-verified) |
| Caliptra RDY_FOR_RT handshake + SCU1 mirror | ✅ `cptra.rs::is_rdy_for_rt()` |
| SPI flash image tooling | ✅ `gen-spi-image.py` |
| Hardware constraints documented | ✅ `bootmcu-bringup.md` |
| WDT / EXTRST reset masks | ✅ `wdt_ast2700.rs`, `extrst.rs` |
| SCU register security policy | ✅ `scu::apply_ibex_default_register_policy()` |
| SLI calibration (SLI0 + SLI1) | ✅ `sli::init_f()`, `sli::init_r()` (hardware-verified) |
| DDR4 DRAM training — full Rust port | ✅ `sdrammc::init()` — AC timing, PHY Step C/F/I tables, MRS, BIST (hardware-verified) |
| DDR5 DRAM training — tables implemented | ✅ `sdrammc::init()` — DDR5 path present, not yet hardware-tested |
| Fabric / PCI E2M / UFS AXI path | ✅ `ca35::init_vendor_runtime_fabric()`, `init_pci_e2m()`, `init_ufs_axi_path()` |
| DRAM MPU regions (vendor parity) | ✅ `ca35::enable_vendor_mpu_regions()` |
| CA35 RVBAR + release | ✅ `ca35::set_rvbar()`, `ca35::release()` (hardware-verified) |
| SSP / TSP init and release | ✅ `ssp_tsp::init_ssp/tsp()`, `enable_ssp/tsp()` (hardware-verified) |
| TamaGo CA35 hello payload | ✅ `cmd/ast2700test` in `tamago` repo — prints silicon revision over UART12 |
| Clean production boot binary | ✅ `rot_ast2700_bootmcu` — 51 KB FMC, full bring-up sequence |

## Phase 2: Caliptra interaction ✅ COMPLETE

**Verified 2025-05-08 on AST2750-A1.**

The SoC ROM handles the full Caliptra boot before FMC runs.  FMC arrives
with Caliptra already in `RDY_FOR_RT`.  No fuse writing or FW upload
needed from FMC.

Verified:
- `FW_INFO`: rom_rev, fmc_rev, rt_rev, pl0_pauser=0x21212121
- `CAPABILITIES`: FIPS capability bit 48 set
- `FIPS_VERSION`: "Caliptra RTM", mode="FIPS", rev 0x06010011

Remaining (tracked in top-level ROADMAP.md Phase G):
- `STASH_MEASUREMENT`: record BootMCU FW hash into Caliptra's DPE
- `QUOTE_PCRS`: attest current firmware state
- Embassy task: continuous TRNG keepalive while CA35 runs

## Phase 3: DRAM training ✅ COMPLETE

**Verified on AST2750-A1 DCSCM (DDR4 board).**

DRAM training is a **full Rust port** in `aspeed-rs/embassy-aspeed/src/sdrammc.rs`
(4107 lines).  No FFI, no vendor C code.  The implementation was derived from
vendor Zephyr/U-Boot sources and validated byte-for-byte against vendor ELF
disassembly.

### PHY firmware prebuilt table (ASTH entries)

| Type | Content | Size |
|------|---------|------|
| 0x01 | DDR4 PMU Train IMEM | 30 KB |
| 0x02 | DDR4 PMU Train DMEM | 64 KB |
| 0x03 | DDR4 2D PMU Train IMEM | 30 KB |
| 0x04 | DDR4 2D PMU Train DMEM | 64 KB |
| 0x05 | DDR5 PMU Train IMEM | 53 KB |
| 0x06 | DDR5 PMU Train DMEM | 64 KB |

Prebuilt binaries are sourced from `bmc-pb/ast2700a1/` — **not** from
`tools/prebuilt/` (that directory is stale and should be deleted).

### Implemented init sequence

1. DDR type detection via SCU1 hardware strap (bit 10)
2. SDRAMMC unlock (`0x1688A8A8`)
3. AC timing registers (DDR4: ACTIME1–7 from vendor values)
4. PHY Step C config table (225 entries for DDR4, 253 for DDR5)
5. PHY cold reset sequence
6. Load IMEM/DMEM to PHY SRAM at `0x13050000`/`0x13058000`
7. PHY training trigger + DDRPHY_INIT_DONE poll
8. Self-refresh exit + SELF_REF_DONE poll
9. MRS sequence (DDR4: MR3→6→5→4→2→1→0)
10. Refresh enable + ZQ control
11. BIST over first 64 KB — passes
12. Size detect
13. QoS init + DRAMC_INIT_DONE flag

### Verified AC timing (DDR4-3200, from vendor)

```
ACTIME1 = 0x0805040C
ACTIME2 = 0x180B1A0B
ACTIME3 = 0x0C061606
ACTIME4 = 0x00000A10
ACTIME7 = 0x0000A1FF
```

### Known remaining gap

DDR5 path is implemented from vendor tables but **not hardware-tested** on
the current board (DDR4 only).

## Phase 4: SLI + peripheral init ✅ COMPLETE

**Verified on AST2750-A1.**

The full vendor init order is implemented in `rot_ast2700_bootmcu.rs`:

```
WDT masks → EXTRST masks → SLI init_f → SLI init_r →
SCU policy → DRAM → fabric/PCI/UFS → load payloads →
MPU → RVBAR → release CA35 → release SSP/TSP
```

SLI0 (system-die ↔ IO-die) and SLI1 (IO-die configuration) are both
calibrated in `sli::init_f()` / `sli::init_r()`.

## Phase 5: SSP/TSP coprocessor support (partially complete)

**BootMCU side: verified.**  SSP and TSP are loaded from flash, initialised,
and confirmed alive via UART probe on hardware.

| Item | Status |
|------|--------|
| BootMCU loads SSP firmware to `0xAC000000` | ✅ `rot_ast2700_bootmcu` |
| BootMCU loads TSP firmware to `0xAE000000` | ✅ `rot_ast2700_bootmcu` |
| `ssp_tsp::init_ssp()` / `init_tsp()` | ✅ hardware-verified |
| `ssp_tsp::enable_ssp()` / `enable_tsp()` | ✅ hardware-verified |
| SSP/TSP Embassy HAL (`ast2700-ssp`, `ast2700-tsp` features) | ❌ not started |
| `clock_ast2700_v1.yaml` — required for SSP/TSP HAL | ❌ see ROADMAP D3 |
| INTC0/INTC1 YAML — required for SSP/TSP IRQs | ❌ see ROADMAP D4 |
| IPC0 YAML — SSP↔CA35 data channel | ❌ see ROADMAP D5 |

### Flash offsets for dev payloads

```
0x800000  CA35 payload (TamaGo or ATF)  — 4 MB window
0xC10000  SSP payload                   — 128 KB window
0xC30000  TSP payload                   — 128 KB window
```

## Phase 6: Secure boot and RoT policy

- Caliptra lifecycle management (manufacturing → production)
- Firmware measurement and attestation via Caliptra
- PFR-style SPI flash monitoring (SSP)
- SPDM/MCTP responder for host BMC
- OTP provisioning workflow

## Build/flash/test reference

```sh
# Build production BootMCU firmware
RUSTFLAGS="-C link-arg=-Tmemory.x -C link-arg=-Tlink.x -C link-arg=-Tdefmt.x -C link-arg=--nmagic" \
cargo build \
    --manifest-path aspeed-mcu-runtime/app-rot/Cargo.toml \
    --target-dir aspeed-mcu-runtime/aspeed-mcu-target \
    --target riscv32imc-unknown-none-elf \
    --release --bin rot_ast2700_bootmcu --features ast2700-bootmcu

# Extract flat binary
llvm-objcopy -O binary \
    aspeed-mcu-runtime/aspeed-mcu-target/riscv32imc-unknown-none-elf/release/rot_ast2700_bootmcu \
    aspeed-mcu-runtime/tools/rot_ast2700_bootmcu.fmc.bin

# Build TamaGo CA35 payload (from tamago/ repo root)
GOOS=tamago GOARCH=arm64 GOOSPKG=github.com/usbarmory/tamago \
    go tool tamago build \
    -tags linkcpuinit -ldflags "-T 0x430000000 -R 0x1000" \
    -o aspeed-mcu-runtime/tools/ast2700_tamago_hello.bin ./cmd/ast2700test
llvm-objcopy -O binary \
    aspeed-mcu-runtime/tools/ast2700_tamago_hello.bin \
    aspeed-mcu-runtime/tools/ast2700_tamago_hello.raw.bin

# Generate SPI image (prebuilts from bmc-pb/ast2700a1/)
python3 aspeed-mcu-runtime/tools/gen-spi-image.py \
    --caliptra bmc-pb/ast2700a1/caliptra-fw.bin \
    --fmc     aspeed-mcu-runtime/tools/rot_ast2700_bootmcu.fmc.bin \
    --prebuilt 1:bmc-pb/ast2700a1/ddr4_pmu_train_imem.bin \
    --prebuilt 2:bmc-pb/ast2700a1/ddr4_pmu_train_dmem.bin \
    --prebuilt 3:bmc-pb/ast2700a1/ddr4_2d_pmu_train_imem.bin \
    --prebuilt 4:bmc-pb/ast2700a1/ddr4_2d_pmu_train_dmem.bin \
    --prebuilt 5:bmc-pb/ast2700a1/ddr5_pmu_train_imem.bin \
    --prebuilt 6:bmc-pb/ast2700a1/ddr5_pmu_train_dmem.bin \
    --prebuilt 7:bmc-pb/ast2700a1/dp_fw.bin \
    --a35-payload aspeed-mcu-runtime/tools/ast2700_tamago_hello.raw.bin \
    --size 16M \
    -o aspeed-mcu-runtime/tools/ast2700_bootmcu_tamago_hello.bin

# Flash + monitor (run from kyanite/ root)
flashrom -p <programmer> -w aspeed-mcu-runtime/tools/ast2700_bootmcu_tamago_hello.bin
picocom /dev/ttyUSB3 -b 115200
```

## UART defmt logging

`rot_ast2700_bootmcu` emits binary `defmt` frames on ROM-configured UART12.
Decode with the matching `defmt-print` version and the BootMCU ELF as the symbol
source. If using a serial device directly, select its 115200 8N1 serial backend;
otherwise capture raw UART bytes and pipe them to `defmt-print` with the ELF.

```sh
ELF=aspeed-mcu-runtime/aspeed-mcu-target/riscv32imc-unknown-none-elf/release/rot_ast2700_bootmcu
defmt-print --help
```
