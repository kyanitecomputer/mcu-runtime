//! SPI/I2C filter policy derived from verified manifests.

use crate::manifest::PlatformFirmwareManifest;

#[cfg(feature = "ast1060")]
use embassy_aspeed::spi_monitor::{CmdEntry, SpiMonitor};

/// Number of 32-bit entries in the AST1060 SPIPF address table.
pub const SPI_ADDRESS_TABLE_ENTRIES: usize = 16;

#[cfg(feature = "ast1060")]
const SCU_SPIM_MODE_CTRL: usize = 0x7e6e_20f0;

/// SPI filter policy for one flash monitor instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpiFilterPolicy {
    write_disable: [u32; SPI_ADDRESS_TABLE_ENTRIES],
    provisioned: bool,
}

impl SpiFilterPolicy {
    /// Create an unprovisioned pass-through policy.
    pub const fn unprovisioned_bypass() -> Self {
        Self {
            write_disable: [0; SPI_ADDRESS_TABLE_ENTRIES],
            provisioned: false,
        }
    }

    /// Create a provisioned policy from protected manifest regions.
    pub fn from_manifest(manifest: &PlatformFirmwareManifest<'_>) -> Self {
        let mut policy = Self {
            write_disable: [0; SPI_ADDRESS_TABLE_ENTRIES],
            provisioned: true,
        };

        for region in manifest.protected_regions {
            if !region.permissions.write {
                policy.protect_range(region.offset, region.size);
            }
        }

        policy
    }

    /// Return true if this policy intentionally bypasses filtering.
    pub const fn is_bypass(&self) -> bool {
        !self.provisioned
    }

    /// Return the write-disable address table.
    pub const fn write_disable_table(&self) -> &[u32; SPI_ADDRESS_TABLE_ENTRIES] {
        &self.write_disable
    }

    fn protect_range(&mut self, offset: u32, size: u32) {
        let Some(end) = offset.checked_add(size) else {
            return;
        };
        if size == 0 {
            return;
        }

        let first = offset / 0x4000;
        let last = end.saturating_sub(1) / 0x4000;
        let mut region = first;
        while region <= last && region < 512 {
            let entry = (region / 32) as usize;
            let bit = region % 32;
            self.write_disable[entry] |= 1u32 << bit;
            region += 1;
        }
    }
}

/// Apply one SPI policy to all AST1060 SPIPF instances.
#[cfg(feature = "ast1060")]
pub fn apply_ast1060_spi_policy(policy: &SpiFilterPolicy) {
    for inst in 1..=4u8 {
        let mon = SpiMonitor::new(inst);
        mon.reset();
        set_ast1060_scu_passthrough(inst, true);
        set_ast1060_scu_monitor(inst, true);
        mon.set_push_pull(true);
        mon.set_passthrough(true, true);
        mon.clear_irq_status();

        if policy.is_bypass() {
            mon.set_filter_enable(false);
            log::warn!("SPIPF{} bypass enabled for unprovisioned mode", inst);
            continue;
        }

        program_common_commands(&mon);
        mon.select_write_disable_table();
        for (entry, regions) in policy.write_disable_table().iter().enumerate() {
            mon.set_address_table(entry as u8, *regions);
        }
        mon.set_irq_enable(true, true, true);
        mon.set_filter_enable(true);

        log::info!("SPIPF{} policy applied", inst);
    }
}

#[cfg(feature = "ast1060")]
fn set_ast1060_scu_passthrough(inst: u8, enable: bool) {
    set_scu_spim_bit(inst, 4, enable);
}

#[cfg(feature = "ast1060")]
fn set_ast1060_scu_monitor(inst: u8, enable: bool) {
    set_scu_spim_bit(inst, 8, enable);
}

#[cfg(feature = "ast1060")]
fn set_scu_spim_bit(inst: u8, base_bit: u8, enable: bool) {
    if !(1..=4).contains(&inst) {
        return;
    }

    let bit = 1u32 << (base_bit + inst - 1);
    let reg = SCU_SPIM_MODE_CTRL as *mut u32;
    // SAFETY: SCU_SPIM_MODE_CTRL is AST1060 SCU0F0. Bits [7:4] enable internal
    // passthrough and bits [11:8] enable monitor output for SPIPF1-4. This runs
    // during single-threaded boot/release sequencing.
    unsafe {
        let mut value = core::ptr::read_volatile(reg);
        if enable {
            value |= bit;
        } else {
            value &= !bit;
        }
        core::ptr::write_volatile(reg, value);
    }
}

#[cfg(feature = "ast1060")]
fn program_common_commands(mon: &SpiMonitor) {
    mon.clear_command_table();
    let mut slot = 0u8;

    allow(mon, &mut slot, cmd(0x03, 3, true, false, true));
    allow(mon, &mut slot, cmd(0x0b, 3, true, false, true));
    allow(mon, &mut slot, cmd(0x3b, 3, true, false, true));
    allow(mon, &mut slot, cmd(0x6b, 3, true, false, true));
    allow(mon, &mut slot, cmd(0xbb, 3, true, false, true));
    allow(mon, &mut slot, cmd(0xeb, 3, true, false, true));
    allow(mon, &mut slot, cmd(0x9f, 0, true, false, false));
    allow(mon, &mut slot, cmd(0x05, 0, true, false, false));
    allow(mon, &mut slot, cmd(0x35, 0, true, false, false));
    allow(mon, &mut slot, cmd(0x06, 0, false, true, false));
    allow(mon, &mut slot, cmd(0x04, 0, false, true, false));
    allow(mon, &mut slot, cmd(0x02, 3, false, true, true));
    allow(mon, &mut slot, erase_cmd(0x20, 1));
    allow(mon, &mut slot, erase_cmd(0x52, 3));
    allow(mon, &mut slot, erase_cmd(0xd8, 5));
    allow(mon, &mut slot, erase_cmd(0xc7, 7));
}

#[cfg(feature = "ast1060")]
fn allow(mon: &SpiMonitor, slot: &mut u8, entry: CmdEntry) {
    mon.allow_command(*slot, entry);
    *slot += 1;
}

#[cfg(feature = "ast1060")]
fn cmd(opcode: u8, addr_bytes: u8, is_read: bool, is_write: bool, is_mem: bool) -> CmdEntry {
    CmdEntry {
        opcode,
        addr_bytes,
        addr_width: if addr_bytes == 0 { 0 } else { 1 },
        data_width: 1,
        dummy_cycles: 0,
        erase_size: 0,
        is_read,
        is_write,
        is_mem,
    }
}

#[cfg(feature = "ast1060")]
fn erase_cmd(opcode: u8, erase_size: u8) -> CmdEntry {
    CmdEntry {
        opcode,
        addr_bytes: if opcode == 0xc7 { 0 } else { 3 },
        addr_width: if opcode == 0xc7 { 0 } else { 1 },
        data_width: 0,
        dummy_cycles: 0,
        erase_size,
        is_read: false,
        is_write: true,
        is_mem: true,
    }
}
