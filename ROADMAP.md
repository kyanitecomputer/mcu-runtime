# aspeed-mcu-runtime — Roadmap

## app-rot (Rust Embassy firmware)

### Phase A: Initial population (complete)

| Task | Description | Status |
|------|-------------|--------|
| A-1 | 7 firmware binaries from ast-ssp-rs examples | ✅ |
| A-2 | All binaries compile for their respective chip targets | ✅ |

### Phase B: AST2700 SSP firmware
*Requires `aspeed-rs` Phase P (AST2700 SSP/TSP HAL) first.*

| Task | Description | Size |
|------|-------------|------|
| B-1 | `hello_uart_ast2700.rs` — print chip ID from GPR bank + 1 Hz counter | S |
| B-2 | `blinky_ast2700.rs` — GPIO toggle via embassy_time | S |
| B-3 | `ipc_echo_ast2700.rs` — IPC0 SSP↔CA35 echo | S |

### Phase C: Hardware-verified baselines

Each binary explicitly verified on hardware (EVB) with output documented in `tests/`:

| Task | Binary | Target | Expected output |
|------|--------|--------|----------------|
| C-1 | `hello_uart` | AST2600 EVB | `Hello from embassy on AST2600 SSP!` + counter |
| C-2 | `hello_uart_ast1060` | AST1060 EVB | `Hello from embassy on AST1060!` + counter |
| C-3 | `hello_uart_bootmcu` | AST2700 EVB | `AST2700 BootMCU - embassy-aspeed hello` |
| C-4 | `ipc_echo` | AST2600 EVB | CA35 sender → SSP echo (needs Python sender script) |

### Phase D: Advanced examples

| Task | Description | Prerequisite |
|------|-------------|-------------|
| D-1 | SHA256 via HACE hardware, print hex digest | aspeed-rs Phase R-2 |
| D-2 | I2C bus scan example for AST2600 | aspeed-rs Phase R-1 |

---

## app-coprocessor/coldfire (ColdFire V1 C SDK)

### Current state: All milestones complete

The SDK is feature-complete for the original scope. 24/24 host unit tests pass.

### Phase B: PAC header migration

The hand-written headers (`cvic.h`, `gpio.h`, `timer.h`, `uart.h`) predate the
`aspeed-data` YAML pipeline. Migrate the HAL source to use the generated
single-file headers (`ast2400.h`, `ast2500.h`):

| Task | Description | Size |
|------|-------------|------|
| B-1 | Audit struct naming differences between hand-written and generated headers | S |
| B-2 | Update `src/hal/*.c` and `include/hal/*.h` to reference generated struct names | M |
| B-3 | Remove `include/pac/cvic.h`, `gpio.h`, `timer.h`, `uart.h` | S |

### Phase C: Future coprocessor code

| Task | Description |
|------|-------------|
| C-1 | `app-coprocessor/ssp-tsp/` — bare-metal C for AST2700 SSP/TSP (if not using Embassy) |

---

## Post-push dependency migration

```toml
# Before (local development):
embassy-aspeed = { path = "../../aspeed-rs/embassy-aspeed" }

# After (post-push):
embassy-aspeed = { git = "https://github.com/kyanitecomputer/aspeed-rs", rev = "<sha>" }
```
