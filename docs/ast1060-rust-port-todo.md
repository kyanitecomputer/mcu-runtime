# AST1060 Non-Hardware Rust Port TODO

This list intentionally excludes board sequencing, GPIO/SGPIO, SPI monitor
muxing, I2C/I3C transport, and other hardware-facing work.

## PFR core

- [x] Define state, event, recovery reason, and transition action types.
- [x] Add a pure state machine that emits hardware-independent actions.
- [x] Add `Lockdown` terminal state; `RecoveryFailed` transitions to it.
- [x] Add `EventRecord` with component + code metadata.
- [x] Add `event_for()` free function for ergonomic record construction.
- [x] Add `Transition::is_ignored()` and `needs_action()` helpers.
- [x] Add `Machine::is_locked_down()`.

## Manifest core

- [x] Define protected-region, image-descriptor, hash, and signature models.
- [x] Add `HashAlgorithm::digest_len()`.
- [x] Validate empty ranges, overflow, empty permissions, and overlapping protected regions.
- [x] Add verification-task extraction via `PlatformFirmwareManifest::verification_task()`.
- [x] Add `VerificationRequest` work item pairing task with key slot.
- [x] Add `VerificationOutcome` enum with `is_pass()`/`is_fail()`.
- [x] Add `VerificationQueue<N>` fixed-capacity FIFO.
- [x] Add `KeyDescriptor`, `KeyKind`, `KeyManifest`.
- [x] Add `AfmDescriptor`, `AfmManifest`.
- [x] Add `AntiRollbackPolicy` and `validate_anti_rollback()`.
- [x] Add `validate_key_descriptor()` and `validate_afm_descriptor()`.
- [x] Deprecate `ProtectedRegion::end()` in favour of `checked_end()`.

## Flash/recovery core

- [x] Define flash slots, regions, layouts, and layout validation.
- [x] Add region lookup by slot and address range.
- [x] Add `RecoveryPair` and `RecoveryPlan<N>`.
- [x] Add `RecoveryLevel` with escalation logic.
- [x] Add `defmt::Format` derives.
- [x] Deprecate `Region::end()` in favour of `checked_end()`.

## Platform abstraction traits

- [x] Define `FlashReader` and `FlashWriter` traits.
- [x] Define `ManifestParser` trait.
- [x] Define `Hasher` and `SignatureVerifier` traits.
- [x] Define `PlatformPolicy` trait with `DefaultPolicy` impl.
- [x] Define `PlatformCommand` enum with `#[non_exhaustive]`.

## Protocol-independent policy

- [x] Define `MailboxPeer`, `MailboxCommand`, `ProtocolEvent`.
- [x] Add `ProtocolEvent::to_pfr_event()`, `pfr_component()`, `to_event_record()`.
- [x] Define `CheckpointKind`, `CheckpointProgress`, `CheckpointEvent`.
- [x] Add `CheckpointEvent::to_pfr_event()`, `pfr_component()`, `to_event_record()`.
- [x] Define `UpdateIntent` with `any_update()` and `to_pfr_event()`.
- [x] Define `SecureState`.

## Provisioning pure model

- [x] Define `ProvisionCommand`, `ProvisionStatus`, `ProvisionError`.
- [x] Define `ProvisionState` with `apply()` state-transition function.

## Still to do

- [ ] Wire `pfr::Machine` + `platform::PlatformCommand` into `rot_ast1060` main loop.
- [ ] Add unit tests for PFR state machine (host-side).
- [ ] Add unit tests for manifest/flash validation (host-side).
- [ ] Add unit tests for provisioning state transitions (host-side).
