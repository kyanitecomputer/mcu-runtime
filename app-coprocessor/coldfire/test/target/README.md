# Target Smoke Tests

On-hardware verification for the AST2400/AST2500 ColdFire SDK.
These tests require real hardware and an ARM-side test harness.

## Status

Planned (M7).  Host-native tests cover all platform-independent logic:
```
make check   # 24 tests: executor, async.h, mailbox ring
```

## Planned tests

| Test | SC | Description | Pass Condition |
|------|----|-------------|----------------|
| blinky | SC-2 | GPIO toggle at 1 Hz via sdk_sleep_ms | Logic analyser or scope confirms 1 Hz ±5% |
| mbox-echo | SC-3 | ARM sends 1000 messages; CF echoes each back | Zero drops, sequence numbers correct |
| timer-accuracy | SC-5 | ARM measures sdk_sleep_ms(1000) × 10 | All within 950–1050 ms |
| idle-current | SC-8 | ARM measures CF current during all-tasks-blocked | CF enters STOP instruction; current drops |

## ARM-side harness requirements

The ARM-side harness (separate project, runs as Linux userspace on the BMC):
1. Programs SCU segment registers for the CF memory map.
2. Loads `firmware.bin` into reserved DRAM.
3. Releases CF from reset.
4. Waits for SDK header `STARTED` flag at SRAM offset `0x38`.
5. Drives the test sequence and reports pass/fail.

See ROADMAP §5 for SRAM layout and segment register values.
