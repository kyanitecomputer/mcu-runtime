# aspeed-mcu-runtime — Roadmap

## app-rot (Root of Trust firmware)

### Phase A: Foundation (complete)

| Task | Description | Status |
|------|-------------|--------|
| A-1 | Crate structure: lib.rs + module stubs (pfr, protocol, crypto, manifest, flash, provision, event_log, platform) | ✅ |
| A-2 | SoC feature flags: ast1060, ast1080, ast1040, ast2700-bootmcu | ✅ |
| A-3 | Firmware entry points: rot_ast1060, rot_ast1080, rot_ast1040, rot_ast2700_bootmcu | ✅ |
| A-4 | UART boot verified on AST1060 hardware | ✅ |
| A-5 | QEMU test (ast1030-evb) in Dagger CI | ✅ |
| A-6 | SysTick fix: clamp reload to MIN_SYSTICK_RELOAD at low HCLK | ✅ |
| A-7 | ast1060-load tool: UART boot loader with ROM handshake (wait for 'U') | ✅ |

### Phase B: AST1060 PFR core

| Task | Description | Size | Depends on |
|------|-------------|------|-----------|
| B-1 | SPI flash driver (FMC read/write/erase) | M | aspeed-rs spi module |
| B-2 | QSPI monitor engine driver (address filter, command whitelist) | M | aspeed-rs |
| B-3 | PFR state machine (Verify → Release → Monitor → Recover) | L | B-1, B-2 |
| B-4 | PFM parser (at least generic format) | M | |
| B-5 | Image manager (A/B bank, recovery copy) | M | B-1 |
| B-6 | Hash-chained event log | S | |
| B-7 | SMBus filter driver | M | aspeed-rs i2c |

### Phase C: Crypto and identity

| Task | Description | Size | Depends on |
|------|-------------|------|-----------|
| C-1 | HACE SHA-256/384/512 driver | M | aspeed-rs |
| C-2 | HACE RSA 2048/4096 driver | M | aspeed-rs |
| C-3 | HACE ECDSA P-256/P-384 driver | M | aspeed-rs |
| C-4 | Async crypto service task (queue + dispatch) | M | C-1..C-3 |
| C-5 | DICE layer (DeviceID → Alias certificate chain) | L | C-1, C-3 |
| C-6 | Anti-rollback (OTP monotonic counters) | S | |

### Phase D: Protocol stack

| Task | Description | Size | Depends on |
|------|-------------|------|-----------|
| D-1 | MCTP transport layer (I2C/SMBus binding) | L | aspeed-rs i2c |
| D-2 | SPDM responder | L | D-1, C-4, C-5 |
| D-3 | OCP Cerberus Challenge (PA-RoT role) | L | D-1, C-4 |
| D-4 | PLDM Type 5 firmware update agent | L | D-1, B-5 |
| D-5 | Provisioning service (vendor-defined MCTP) | M | D-1, C-6 |

### Phase E: Platform implementations

| Task | Description | Size | Depends on |
|------|-------------|------|-----------|
| E-1 | Generic PFR platform (custom manifest, own sequencer) | M | B-3, B-4 |
| E-2 | Intel PFR platform (Intel PFM parser, ME/CSME handshake) | L | B-3 |
| E-3 | AMD PFR platform (AMD manifest parser, PSP handshake) | L | B-3 |

### Phase F: New SoC support

*Requires linker scripts and HAL support in aspeed-rs first.*

| Task | Description | Size | Depends on |
|------|-------------|------|-----------|
| F-1 | AST1080 HAL (linker script, boot code, HyperRAM, Caliptra) | L | aspeed-rs |
| F-2 | AST1080 anti-tamper drivers (glitch, temp, volt, clock, PUF, TRNG) | L | F-1 |
| F-3 | AST1040 HAL (linker script, boot code, HyperRAM, eSPI, USB) | L | aspeed-rs |
| F-4 | AST2700 BootMCU Caliptra lifecycle management | L | aspeed-rs |

---

## app-coprocessor/ssp (Embassy Rust — Cortex-M3/M4)

### Phase A: Foundation (complete)

| Task | Description | Status |
|------|-------------|--------|
| A-1 | Crate structure: lib.rs + module stubs (sensor, fan_control, mailbox, protocol, board_config) | ✅ |
| A-2 | SoC feature flags: ast2600-ssp, ast2700-ssp, ast2700-tsp | ✅ |
| A-3 | Firmware entry points: ssp_ast2600, ssp_ast2700, tsp_ast2700 | ✅ |
| A-4 | Diagnostics: hello_uart (AST2600 SSP), ipc_echo | ✅ |

### Phase B: AST2600 SSP sensor subsystem

| Task | Description | Size | Depends on |
|------|-------------|------|-----------|
| B-1 | Sensor data model: SensorDescriptor, SensorReading, SensorStore | M | |
| B-2 | ADC poller task | S | aspeed-rs ADC driver |
| B-3 | I2C/PMBus poller task (board temperature, voltage/current) | M | aspeed-rs I2C |
| B-4 | PECI poller task (CPU temperature) | M | aspeed-rs PECI |
| B-5 | Board-config crate pattern (compiled-in sensor table) | S | B-1 |

### Phase C: Fan control + mailbox IPC

| Task | Description | Size | Depends on |
|------|-------------|------|-----------|
| C-1 | Fan control task (PID loop, thermal zone mapping) | M | B-1, aspeed-rs PWM |
| C-2 | Hardware mailbox driver (SSP ↔ AP doorbell + shared SRAM) | M | aspeed-rs |
| C-3 | Mailbox IPC responder (command set: read sensors, set fan, set LED) | M | B-1, C-2 |
| C-4 | Threshold alerting (sensor store → mailbox notification) | S | B-1, C-2 |

### Phase D: Protocol stack

| Task | Description | Size | Depends on |
|------|-------------|------|-----------|
| D-1 | MCTP transport over I2C/SMBus | L | aspeed-rs I2C |
| D-2 | PLDM Type 2 terminus (PDR repo, GetSensorReading, effecters) | L | D-1, B-1 |
| D-3 | SPDM responder for coprocessor attestation (optional) | L | D-1 |

### Phase E: AST2700 SSP/TSP

*Requires linker scripts and HAL support in aspeed-rs first.*

| Task | Description | Size | Depends on |
|------|-------------|------|-----------|
| E-1 | AST2700 SSP HAL (linker script, boot code, I3C) | L | aspeed-rs |
| E-2 | AST2700 SSP sensor firmware (MCTP over I3C) | M | E-1, D-1 |
| E-3 | AST2700 TSP HAL (CAN bus, LTPI) | L | aspeed-rs |
| E-4 | AST2700 TSP PSU telemetry over CAN | M | E-3 |
| E-5 | AST2700 TSP LTPI link management | M | E-3 |

---

## app-coprocessor/coldfire (C + async.h — ColdFire V1 / M68K)

### Current state: All milestones complete

24/24 host unit tests pass. SDK is feature-complete for original scope.
Covers AST2400 and AST2500 coprocessors (ColdFire V1 ISA_A+C, ~40KB SRAM).

The ColdFire coprocessors share the same sensor/fan/IPC mission as the ARM
SSP cores but are too constrained for Rust or a full MCTP/PLDM stack. They
use raw mailbox IPC with the AP side.

### Phase B: PAC header migration

| Task | Description | Size |
|------|-------------|------|
| B-1 | Audit struct naming differences (hand-written vs generated) | S |
| B-2 | Update HAL to use generated `ast2400.h` / `ast2500.h` | M |
| B-3 | Remove hand-written `cvic.h`, `gpio.h`, `timer.h`, `uart.h` | S |

### Phase C: Sensor subsystem (C)

| Task | Description | Size |
|------|-------------|------|
| C-1 | Sensor data model (C structs, ISR-guarded store) | S |
| C-2 | ADC + I2C poller tasks (async.h) | M |
| C-3 | Fan control task (PID/table) | M |
| C-4 | Mailbox IPC responder (raw protocol, cmd 0x01–0x12) | M |

---

## Tools

| Tool | Description | Status |
|------|-------------|--------|
| `ast1060-load` | Go UART boot loader (ROM handshake, throttled xfer, monitor) | ✅ |
| `gen-uart-image.sh` | UART boot image generator (LE32 size header + payload) | ✅ |
| `gen-flash-image.sh` | SPI flash image padder | ✅ |

---

## Post-push dependency migration

```toml
# Before (local development):
embassy-aspeed = { path = "../../aspeed-rs/embassy-aspeed" }

# After (post-push):
embassy-aspeed = { git = "https://github.com/kyanitecomputer/aspeed-rs", rev = "<sha>" }
```
