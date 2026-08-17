# aspeed-mcu-runtime

Firmware applications for ASPEED MCU cores.

Part of the [Kyanite](https://github.com/kyanitecomputer) stack.

> **Status:** experimental — expect breaking changes.

```
https://github.com/kyanitecomputer/aspeed-mcu-runtime
```

## System architecture

ASPEED BMC SoCs contain multiple processor cores spanning three ISAs and
four runtime environments:

```
┌──────────────────────────────────────────────────────────────────┐
│  AP Cores (main application processors)                          │
│  AST2600: Cortex-A7      │  AST2700: Cortex-A35 ×4              │
│  Runtime: bare-metal Go (TamaGo)                                 │
│  Repo: tamago  (HAL library: aspeed-go)                          │
├────────────────────────────────┬─────────────────────────────────┤
│  RoT Cores                    │  Coprocessor Cores               │
│  (Root of Trust / PFR)        │  (Sensor DAQ / Fan / IPC)        │
│                                │                                  │
│  AST1060 Cortex-M4F (ARM)    │  AST2600 SSP Cortex-M3 (ARM)    │
│  AST1080 Cortex-M4F (ARM)    │  AST2700 SSP Cortex-M4 (ARM)    │
│  AST1040 Cortex-M4F (ARM)    │  AST2700 TSP Cortex-M4 (ARM)    │
│  AST2700 BootMCU RV32 (RISC-V)│ AST2400/2500 ColdFire V1 (M68K)│
│  Runtime: Rust + Embassy       │  Runtime: Rust + Embassy (ARM)  │
│  Crate: app-rot                │           C + async.h (ColdFire)│
│                                │  Crates: app-coprocessor/ssp    │
│                                │          app-coprocessor/coldfire│
├────────────────────────────────┴─────────────────────────────────┤
│  Shared: aspeed-rs (HAL) → aspeed-data (PAC)                     │
└──────────────────────────────────────────────────────────────────┘
```

| ISA | Cores | Runtime | Language |
|-----|-------|---------|----------|
| ARMv7-A / ARMv8-A | AP (Cortex-A7, A35) | TamaGo bare-metal | Go |
| ARMv7-M / ARMv7E-M | RoT + Coprocessor (Cortex-M3, M4F) | Embassy-rs async | Rust |
| RV32IMC | AST2700 BootMCU (ibex) | Embassy-rs async | Rust |
| ColdFire V1 (M68K) | AST2400/2500 coprocessor | async.h protothreads | C |

## Structure

```
aspeed-mcu-runtime/
├── app-rot/                    Root of Trust firmware
│   ├── src/
│   │   ├── lib.rs              PFR core, protocols, crypto, manifests
│   │   ├── pfr/                NIST SP 800-193 state machine
│   │   ├── protocol/           MCTP, SPDM, Cerberus, PLDM
│   │   ├── crypto/             HACE / Caliptra crypto service
│   │   ├── manifest/           PFM, CFM, PCD management
│   │   ├── flash/              SPI flash + A/B image management
│   │   ├── provision/          MCTP-based provisioning
│   │   ├── event_log/          Hash-chained attestation log
│   │   ├── platform/           Intel / AMD / generic PFR
│   │   └── bin/                firmware entries + diagnostics
│   └── Cargo.toml
├── app-coprocessor/
│   ├── ssp/                    Coprocessor firmware (Embassy Rust)
│   │   ├── src/
│   │   │   ├── lib.rs          sensor DAQ, fan control, IPC, protocols
│   │   │   ├── sensor/         Sensor polling + data model
│   │   │   ├── fan_control/    PID / thermal table per zone
│   │   │   ├── mailbox/        HW mailbox IPC (AP ↔ coprocessor)
│   │   │   ├── protocol/       MCTP, PLDM Type 2, SPDM
│   │   │   ├── board_config/   Per-board sensor tables
│   │   │   └── bin/            firmware entries + diagnostics
│   │   └── Cargo.toml
│   └── coldfire/               ColdFire V1 C SDK (AST2400/AST2500)
│       ├── include/            HAL + PAC headers
│       ├── src/                HAL drivers + async runtime
│       └── Makefile
└── tools/
    ├── ast1060-load/           UART boot loader (Go)
    ├── aspeed-uart-load/       AST10x0/AST2700 UART loader (Go)
    └── imgtools/               Flash/UART/AST2700 image tools (Go)
```

## Supported SoCs

### Root of Trust (app-rot)

| SoC | CPU | ISA | Role | Feature | Status |
|-----|-----|-----|------|---------|--------|
| **AST1060** | Cortex-M4F 200MHz | ARMv7E-M | PFR processor | `ast1060` | HW verified |
| **AST1080** | Cortex-M4F 400MHz | ARMv7E-M | Hardened RoT (Caliptra, PUF, anti-tamper) | `ast1080` | Stub |
| **AST1040** | Cortex-M4F 400MHz | ARMv7E-M | BIC / BMC (Caliptra, eSPI, USB) | `ast1040` | Stub |
| **AST2700 BootMCU** | ibex RV32IMC 400MHz | RISC-V | Secure boot MCU (Caliptra lifecycle) | `ast2700-bootmcu` | HW verified |

### Coprocessor — Embassy Rust (app-coprocessor/ssp)

| SoC | CPU | ISA | Role | Feature | Status |
|-----|-----|-----|------|---------|--------|
| **AST2600 SSP** | Cortex-M3 200MHz | ARMv7-M | Sensor/fan/IPC | `ast2600-ssp` | Compiles |
| **AST2700 SSP** | Cortex-M4 400MHz | ARMv7E-M | Sensor/fan/IPC (I3C) | `ast2700-ssp` | Stub |
| **AST2700 TSP** | Cortex-M4 400MHz | ARMv7E-M | CAN/LTPI/PSU | `ast2700-tsp` | Stub |

### Coprocessor — C (app-coprocessor/coldfire)

| SoC | CPU | ISA | Role | Status |
|-----|-----|-----|------|--------|
| **AST2400** | ColdFire V1 | M68K ISA_A+C | Sensor/fan/IPC | Complete (24/24 tests) |
| **AST2500** | ColdFire V1 | M68K ISA_A+C | Sensor/fan/IPC | Complete (24/24 tests) |

## Dependencies

| Repo | Role | Used by |
|------|------|---------|
| [`aspeed-rs`](https://github.com/kyanitecomputer/aspeed-rs) | Embassy HAL | app-rot, app-ssp |
| [`aspeed-data`](https://github.com/kyanitecomputer/aspeed-data) | PAC + generated headers | transitive (Rust + C) |
| `tamago` | TamaGo fork — AST SoC/board packages for CA35 bare-metal Go | CA35 payload |
| [`aspeed-go`](https://github.com/kyanitecomputer/aspeed-go) | Go MMIO primitives and HAL library consumed by TamaGo | transitive (Go) |

## Build

```sh
# Full CI (RoT + SSP + ColdFire + QEMU test)
dagger call ci --aspeed-rs ../aspeed-rs --aspeed-data ../aspeed-data

# Individual steps
dagger call check-rot --aspeed-rs ../aspeed-rs --aspeed-data ../aspeed-data
dagger call check-ssp --aspeed-rs ../aspeed-rs --aspeed-data ../aspeed-data
dagger call check-coldfire
dagger call qemu-test --aspeed-rs ../aspeed-rs --aspeed-data ../aspeed-data
```

### AST2700 full flash image with NATS CA35 payload

The hardware-test image is built through Dagger using StageX containers. It
builds:

- AST2700 BootMCU firmware from `app-rot`
- CA35 payload from `../cmd/nats`
- AST2700 flash image using `tools/imgtools`

Default build, without SSP/TSP payloads:

```sh
dagger call build-ast-2700-image \
    --aspeed-rs ../aspeed-rs \
    --aspeed-data ../aspeed-data \
    --tamago ../tamago \
    --tamago-go ../../tamago/tamago-go \
    --cmd-nats ../cmd/nats \
    --aspeed-go ../aspeed-go \
    --lneto ../lneto \
    --nats-server ../nats-server \
    --scree ../scree \
    --bmc-pb ../bmc-pb/ast2700a1 \
    export --path ./out
```

Useful flags:

```sh
# Output image size/name and CA35 link layout.
--image-size 32M
--output-name ast2700_bootmcu_nats.bin
--ca35-link-address 0x404000000
--ca35-reserve 0x1000
--go-build-tags linkcpuinit,ast2700dcscm

# Optional M4 coprocessor payloads from bmc-pb.
--include-ssp=true
--include-tsp=true
```

The resulting flash image is exported under `./out/`.

For local development without Dagger, the image stitcher is:

```sh
cd tools/imgtools
GOWORK=off go run . spi-image --help
```

## Flashing (AST1060 UART boot)

```sh
cd app-rot
cargo build --bin rot_ast1060 --features ast1060 \
    --target thumbv7em-none-eabihf --release
rust-objcopy --strip-all -O binary \
    target/thumbv7em-none-eabihf/release/rot_ast1060 /tmp/rot_ast1060.bin
GOWORK=off go run ../tools/imgtools uart-image \
    --input /tmp/rot_ast1060.bin --output /tmp/rot_ast1060_uart.bin
../tools/ast1060-load/ast1060-load \
    -port /dev/ttyUSB0 -wait 30s -monitor /tmp/rot_ast1060_uart.bin
```

## Contributing

See the org-wide [CONTRIBUTING guide](https://github.com/kyanitecomputer/.github/blob/main/CONTRIBUTING.md).
Contributions are dual-licensed.

## Security

See the org-wide [SECURITY policy](https://github.com/kyanitecomputer/.github/blob/main/SECURITY.md).

## License

Dual-licensed under either of Apache-2.0 ([LICENSE-APACHE](LICENSE-APACHE)) or
MIT ([LICENSE-MIT](LICENSE-MIT)) at your option.
