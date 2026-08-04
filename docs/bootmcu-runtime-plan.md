# BootMCU Runtime — Completion Plan

Plan for maturing the AST2700 BootMCU RoT runtime (`app-rot`) and its HAL
(`embassy-aspeed`) across five workstreams:

1. Structured logging via the `log` crate (+ `ufmt`), dropping defmt
2. Compressed CA35 payload loading via `m77rip`
3. Hardware mailbox inter-core IPC with protobuf (`buffa`) messages
4. SCU policy control engine (data-driven)
5. Caliptra secure-boot completion (manifest auth + CA35 authorization)

Boot works today; the gaps below are firmware-logic gaps, not silicon issues.
The current boot log shows `Caliptra auth manifest... FAIL` and
`A2 authorize CA35... FAIL`, tolerated only because `EN_SECBOOT` is unset.

---

## Decisions (locked)

- **Logging:** drop `defmt` entirely for now. Use the `log` crate as the façade
  everywhere, backed by a UART global logger; use `ufmt`-style formatting for
  the backend / direct diagnostics, mirroring the vein/vega loader.
- **Compression:** use `m77rip` (`github.com/paddor/m77rip`, as in the
  vein/vega loader), not lz4. **Remove all existing lz4 code.**
- **IPC payloads:** protobuf, generated with `anthropics/buffa` (Rust) and
  protoc-gen-go (Go). Data structures are defined once in the `./schema` repo
  (`schema/schema/v1/*.proto`) and consumed as `kyanite-schema` (Rust, on the
  BootMCU/ibex and SSP/TSP/CM4) and `schema/gen/go` (Go, on the CA35/PSP).
  Postcard is rejected: it is Rust-only and the CA35 runs Go.
- **Fuses/OTP:** **never blow/burn OTP or fuses during development.** All
  Caliptra work runs with `EN_SECBOOT` unset and against un-fused dev keys.
  Fuse provisioning becomes image-build tooling later; not required now.

---

## Sequencing (dependency order, not effort)

Logging (WS1) lands first — every later workstream emits diagnostics, so doing
it now avoids re-touching those sites. WS2–WS4 are mutually independent. WS5
(Caliptra) depends on WS1 for trace output and on crypto primitives already in
`embassy-aspeed`.

```
WS1 logging ─┬─> WS2 m77rip compression
             ├─> WS3 mailbox IPC (protobuf)
             ├─> WS4 policy engine
             └─> WS5 Caliptra secure boot
```

---

## WS1 — Structured logging (`log` + `ufmt`, drop defmt)

**Goal.** One uniform logging façade (`log::{info,warn,error,debug,trace}`)
across `app-rot` and `embassy-aspeed`, backed by a UART global logger with
runtime level filtering, replacing raw `uart.blocking_write(b"...")` and all
defmt paths.

**Current state.**
- `app-rot` uses three mechanisms: raw UART writes (dominant — e.g.
  `rot_ast2700_bootmcu.rs:502,505-512,417,439-475`, all diagnostic bins),
  `defmt` (default feature, mostly `filter.rs:90-104`), and `defmt-rtt` in ARM
  bins. No `log` crate anywhere.
- `embassy-aspeed` is defmt-only, feature-gated (`ca35.rs`, `sli.rs`,
  `sdrammc.rs`, …), with types deriving `defmt::Format`. A defmt-over-UART12
  global logger exists at `embassy-aspeed/src/defmt_uart.rs`.
- Reference: vein's Rust loader uses `ufmt` (`Uart: ufmt::uWrite`, `uwriteln!`).

**Missing pieces.**
1. Add `log` dep to `embassy-aspeed` and `app-rot`.
2. New `embassy-aspeed/src/log_uart.rs`: `struct UartLogger` implementing
   `log::Log`, rendering `Record`s into UART12 (critical-section-guarded, reuse
   the write path from `defmt_uart.rs`); `static LOGGER` installed via
   `log::set_logger` + `log::set_max_level`.
3. Convert `embassy-aspeed` internal `defmt::{info,warn,error}` sites to
   `log::*`; drop `derive(defmt::Format)` and the defmt/defmt-rtt/defmt-uart
   features and deps.
4. Convert `app-rot` raw-UART boot messages to `log` macros (keep the exact
   human-readable strings, e.g. `info!("DRAM... OK")`); init the logger at boot
   before first use.
5. Cargo features: remove `defmt`/`ast2700-bootmcu-defmt`; make the UART logger
   the default. Update `docs/bootmcu-next-steps.md` logging section.

**Files.** `embassy-aspeed/{Cargo.toml, src/lib.rs, src/log_uart.rs (new),
src/defmt_uart.rs (remove), internal modules}`, `app-rot/{Cargo.toml,
src/bin/*, src/*}`.

**Verify.** Build `rot_ast2700_bootmcu`; confirm identical boot log on hardware;
confirm `log::set_max_level` gates `debug!`/`trace!`.

---

## WS2 — Compressed CA35 payload via m77rip

**Goal.** Ship the CA35 SoC image m77rip-compressed in flash; decompress
XIP→DRAM at boot, mirroring the vein/vega loader.

**Current state.** Loads **raw**: `A2_CA35_LZ4 = false`
(`rot_ast2700_bootmcu.rs:94`); active path word-copies via `manifest::copy32`
(`:462-473`). A dead lz4 branch exists (`:446-461`) plus the whole
`embassy-aspeed::lz4` module (`src/lz4.rs`, `lz4_flex` + a bump allocator).
Reference: vein `loader/build.rs:39` `m77rip::compress`, decoded at boot by
`m77rip_decode::decompress_into` (`loader/src/main.rs`).

**Missing pieces.**
1. Remove `embassy-aspeed/src/lz4.rs`, its `lz4` feature, the `lz4_flex` dep,
   and the dead lz4 branch + `A2_CA35_LZ4` flag in `rot_ast2700_bootmcu.rs`.
   Also reassess the bump allocator (only needed if m77rip needs alloc — the
   vein decoder is `default-features = false`, i.e. no_std/no-alloc).
2. Add `m77rip-decode` (git, `default-features = false`) to the bootmcu build;
   decompress the post-header payload into the `A35_PAYLOAD_SIZE` DRAM window.
3. `imgtools`: add an `m77rip::compress` step so `gen-a2-image.sh` emits
   `header ‖ m77rip(payload)`; reconcile the header `payload_len` semantics
   (compressed length) with the loader.
4. Keep a raw fallback path/flag for bring-up if useful.

**Files.** `embassy-aspeed/{Cargo.toml, src/lib.rs, src/lz4.rs (remove)}`,
`app-rot/{Cargo.toml, src/bin/rot_ast2700_bootmcu.rs}`, `tools/imgtools/*`,
`../cairn/tools/a2/gen-a2-image.sh`.

**Verify.** Flash an m77rip image; confirm CA35 boots to the same entry.

**Status.** Decode path implemented and proven correct (host round-trip +
hardware isolation). Compression is **opt-in** (`A35_COMPRESS=1`); the default
is the verbatim raw payload. A compressed SoC image trips the BootROM/Caliptra
secure-boot manifest stage *before* the FMC launches (the flashed compressed
bytes don't match the runtime image Caliptra authorizes) — deterministically
different Caliptra codes vs. the raw image, same FMC. Enabling compression
depends on the WS5 Caliptra manifest work (authorizing a compressed/relocated
SoC image, or excluding it from ROM-stage authorization).

---

## WS3 — Hardware mailbox inter-core IPC (protobuf)

**Goal.** A HW-mailbox IPC layer carrying protobuf messages defined in `./schema`
so every core speaks the same wire format.

**Codec split (a hardware constraint, not a preference).** The `riscv32imc`
BootMCU has no atomic compare-exchange, and buffa's deps (`bytes`, `once_cell`)
require it — so buffa cannot build there. Therefore:
- CA35 PSP (Go): `protoc-gen-go`.
- SSP/TSP (Cortex-M4, has atomics): buffa (`kyanite-schema`).
- BootMCU (riscv32imc): a hand-rolled protobuf codec (`aspeed-mcu-runtime/ipc-proto`)
  producing the *same* proto3 wire bytes (cross-checked byte-for-byte against
  protoc-gen-go), `no_std` + no alloc.

**TrustZone / TEE.** The BootMCU serves IPC on IPC1 **sub-channel 1
(non-secure CA35)** because the cairn CA35 payload currently runs non-secure.
FUTURE: once TrustZone is enabled, move the BootMCU↔PSP link into a TEE (secure
world) on **sub-channel 0** and require the secure sub-channel.

**Status (first slice landed).** `ipc.proto` (`IpcEnvelope`: Ping/Pong,
GetRotStatus/RotStatus); BootMCU responder over `Ipc1`; CA35 Go client that
pings the RoT and reads `RotStatus` at boot. `[len][protobuf]` framing in one
32-byte slot (segmentation deferred until a message exceeds 31 bytes).
Remaining: shared-SRAM/doorbell transport for large payloads, and WS3c (SSP/TSP
driver + buffa responder, needs the `ast2700-ssp/tsp` HAL features).

**Current state.**
- AST2700 IPC = register block, two instances `ipc0`/`ipc1`, 4 sub-channels ×
  0x200, per-channel `TRIG/ENABLE/STATUS` + 4×32-byte `DATA` slots (ref
  `zephyr/drivers/ipm/ipm_ast2700.c`, `dts .../ast27xx*.dtsi`).
- BootMCU (ibex, **no IRQ**, polling) works: `embassy-aspeed/src/ipc1.rs`
  (`Ipc1`, base `0x14C3_9000`), exercised by `ipc_echo_bootmcu.rs`. Sub-channel
  map: 0=S-CA35, 1=NS-CA35, 2=SSP, 3=TSP.
- SSP/TSP: **no AST2700 driver** (only AST2600 `ipc.rs`).
  `app-coprocessor/ssp/src/mailbox/mod.rs` is a doc-only stub. No shared command
  definition.

**Missing pieces.**
1. Define IPC messages in `schema/schema/v1/ipc.proto` (e.g. sensor read/report,
   fan/LED effecters, threshold/fault/watchdog events; reserve a Caliptra
   channel). Regenerate `kyanite-schema` (Rust) + `schema/gen/go` (Go) with buf.
2. Confirm/enable `buffa` no_std usage for the ibex and CM4 targets; add
   `kyanite-schema` as a dep to `app-rot` and the SSP/TSP crate.
3. Generalize `Ipc1` into a base-parameterized `IpcChannel` (BootMCU
   `0x14C3_9000`, SSP/TSP `0x72C1_C000`/`0x74C3_9000`); parameterize the TX/RX
   half role (offsets invert per link end).
4. AST2700 SSP/TSP IPC driver with IRQ-driven RX (`AtomicWaker`/`on_interrupt`,
   model `ipc.rs:236-253`); add `ast2700-ssp`/`ast2700-tsp` HAL features.
5. Framing over the 32-byte slots: a small header (len, msg-type, seq) plus
   multi-packet reassembly for protobuf payloads larger than one slot, or wire
   the unused `shmem_chN` shared-SRAM path.
6. Replace the `mailbox/mod.rs` stub with a responder built on the generated
   protobuf command set.

**Files.** `schema/schema/v1/ipc.proto`, regenerated `schema/gen/*`,
`embassy-aspeed/src/{ipc1.rs→ipc_ast2700.rs, lib.rs, Cargo.toml}`,
`app-coprocessor/ssp/src/mailbox/mod.rs`, `app-rot/src/bin/ipc_echo_bootmcu.rs`.

**Verify.** BootMCU↔SSP round-trip of a protobuf-encoded command on hardware;
Go CA35 decodes the same message; IRQ-driven RX on SSP, polling on BootMCU.

---

## WS4 — SCU policy control engine (data-driven)

**Goal.** Make the per-register security/access-control policy programmable from
a config table instead of the hardcoded empty lists.

**Current state.** `embassy-aspeed/src/scu.rs::apply_ibex_default_register_policy()`
(`:292-298`) is a faithful 1:1 port of vendor `u-boot .../sys_policy.c` (bases,
bank offsets, lock masks, 7-group/3-bit model all match), but calls
`apply_register_policy()` with four `PolicyList::EMPTY` lists (`:187-197`) —
matching the current empty vendor DT (`ast2700-ibex.dts:157-195`). Called from
`rot_ast2700_bootmcu.rs:536`. (`app-rot/src/filter.rs` is the unrelated AST1060
SPI/I²C monitor filter, not this engine.)

**Missing pieces.**
1. A mechanism to load **non-empty** per-register policy lists (register-ID
   arrays per group: sec-psp/ssp/psp/tsp/psp-ssp/ssp-tsp/bmcu), expressed as a
   Rust const table / board config.
2. A defined default policy for the Kyanite platform (which domain may write
   which registers) rather than "lock with empty assignment."
3. Verification hooks/logging (WS1) around lock application; document the
   security model in `docs/`.

**Files.** `embassy-aspeed/src/scu.rs`, `app-rot/src/bin/rot_ast2700_bootmcu.rs`
(or a board-config module), `docs/`.

**Verify.** Confirm lock registers read back locked; confirm a policy-restricted
register rejects writes from the wrong domain on hardware.

---

## WS5 — Caliptra secure-boot completion

**Goal.** Make `SET_AUTH_MANIFEST` and `AUTHORIZE_AND_STASH` actually PASS so the
CA35 image is cryptographically authorized before release. Runs entirely with
**un-fused dev keys and `EN_SECBOOT` unset** (no OTP writes) until fuse tooling
exists.

**Current state.** Both steps are effectively stubbed:
- `set_auth_manifest()` (`rot_ast2700_bootmcu.rs:261-284`) forwards the **raw**
  `HDR_ID_SOC_MANIFEST` slice to `Caliptra::set_auth_manifest` (`cptra.rs:752-760`,
  which only prepends a length word). Caliptra requires an `AuthManifestPreamble`
  (marker `0x324D_5441`, size `24292`; ref
  `caliptra-sw/.../set_auth_manifest.rs:704-727`). The vendor converts the ASPEED
  preamble first (`manifest_image_sig.c:79-110`) — the Rust side does not.
- `authorize_ca35_a2()` (`:378-409`) sends `AUTHORIZE_AND_STASH` with a **zeroed
  digest** and `ImageHashSource::LoadAddress`, which hashes at the manifest's
  baked `image_load_address`, not the caller's placement. Vendor hashes in
  firmware and uses `source = InRequest` (`manifest_image_sig.c:281-292`). With
  no manifest installed, metadata lookup returns `IMAGE_NOT_AUTHORIZED`.
- IDEVID (`:286-370`) already closely mirrors vendor `cptra_idevid.c` (OTP
  offsets `0x62`/`0x262`, tag `0x8230`) — the most complete of the three.
- Driver `cptra.rs`: FW_INFO/CAPABILITIES/FIPS_VERSION/IDEV-cert implemented;
  `set_auth_manifest`/`authorize_and_stash` transport-only;
  `STASH_MEASUREMENT`/`QUOTE_PCRS`/DPE/EXTEND_PCR declared but unimplemented.
  Crypto primitives exist (`hace.rs` SHA-384 `:154`, `ecdsa.rs` P-384 `:95-109`,
  `rsa.rs`) but are **not** wired into the manifest path; **no LMS verifier**.

**Missing pieces.**
1. **Manifest preamble conversion** (port `cptra_preamble_convert`,
   `manifest_image_sig.c:79-110`): build the exact Caliptra `AuthManifestPreamble`
   byte layout + image-metadata collection; feed that (not the raw slice) to
   `set_auth_manifest`.
2. **SoC-manifest parser** in `embassy-aspeed/src/manifest.rs`: marker,
   preamble_size, version, ime_count, vendor+owner ECC384 + LMS/PQC keys & sigs,
   `imc[]` entries (mirror vendor `manifest.h:139-221`).
3. **In-firmware image hashing** for `authorize_ca35_a2`: SHA-384 over the CA35
   image via `Hace::sha384`, pass `ImageHashSource::InRequest`.
4. **Manifest SVN / signature verification** (port `cptra_verify_soc_manifest_ver`,
   `manifest_image_sig.c:226-261`): ECDSA-384 (`ecdsa.rs`) + **LMS** (new
   verifier — port or crate) against dev owner keys.
5. **Dev key handling (no fuses)**: use un-fused/software dev keys; keep any
   FAIL gated on `EN_SECBOOT` (unset). Document that real signature enforcement
   waits on the future fuse-provisioning build tooling.
6. **Supporting driver commands**: implement `STASH_MEASUREMENT`, `QUOTE_PCRS`
   for measurement/attestation.
7. **Compressed SoC image authorization** (unblocks WS2 compression): the
   BootROM/Caliptra manifest stage authorizes SoC-image bytes as stored in the
   FLSH container, so an m77rip-compressed CA35 image fails before the FMC runs.
   Resolve by authorizing the compressed image (manifest measures the stored
   bytes) or excluding the CA35 image from ROM-stage authorization and having
   the FMC authorize the decompressed image instead.

**Files.** `embassy-aspeed/src/{cptra.rs, manifest.rs, hace.rs, ecdsa.rs, new
lms.rs}`, `app-rot/src/bin/rot_ast2700_bootmcu.rs`, `app-rot/src/manifest/mod.rs`.

**Verify.** With a converted, dev-signed manifest: `auth manifest... OK` then
`authorize CA35... OK`, all with `EN_SECBOOT` unset and no OTP writes. Tamper
tests deferred until fuse tooling lands.
