//! Generic PFR platform implementation.
//!
//! Custom manifest format and sequencer for OEM platforms or evaluation
//! boards that don't use Intel or AMD PFR flows.

#[cfg(feature = "ast1060")]
use crate::filter::{apply_ast1060_spi_policy, SpiFilterPolicy};
#[cfg(feature = "ast1060")]
use embassy_aspeed::gpio::{pin, Level, Output};
#[cfg(feature = "ast1060")]
use embassy_aspeed::sgpio::Sgpio;
#[cfg(feature = "ast1060")]
use embassy_aspeed::spi::{Controller, SpiBus};
#[cfg(feature = "ast1060")]
use embassy_time::{Duration, Timer};

#[cfg(feature = "ast1060")]
const SCU_SPIM_MODE_CTRL: usize = 0x7e6e_20f0;

#[cfg(feature = "ast1060")]
pub struct Ast1060DcscmPlatform {
    sgpio: Sgpio,
    flash_power_l2: Output,
    flash_power_l3: Output,
    bmc_srst: Output,
    bmc_extrst: Output,
    first_release: bool,
}

#[cfg(feature = "ast1060")]
const SGPIO_E_H_BASE: u8 = 32;
#[cfg(feature = "ast1060")]
const SGPO_AUX_PWRGD_CPU0: u8 = SGPIO_E_H_BASE + 5;
#[cfg(feature = "ast1060")]
const SGPO_PLTRST_CPU0: u8 = SGPIO_E_H_BASE + 6;
#[cfg(feature = "ast1060")]
const SGPO_AUX_PWRGD_CPU1: u8 = SGPIO_E_H_BASE + 21;
#[cfg(feature = "ast1060")]
const SGPO_PLTRST_CPU1: u8 = SGPIO_E_H_BASE + 22;

#[cfg(feature = "ast1060")]
impl Ast1060DcscmPlatform {
    pub fn new() -> Self {
        let sgpio = Sgpio::new(16, 24);
        sgpio.enable();

        sgpio.set_output_pin(SGPO_AUX_PWRGD_CPU0, false);
        sgpio.set_output_pin(SGPO_AUX_PWRGD_CPU1, false);
        sgpio.set_output_pin(SGPO_PLTRST_CPU0, false);
        sgpio.set_output_pin(SGPO_PLTRST_CPU1, false);

        Self {
            sgpio,
            flash_power_l2: Output::new(pin('L', 2), Level::High),
            flash_power_l3: Output::new(pin('L', 3), Level::High),
            bmc_srst: Output::new(pin('M', 5), Level::Low),
            bmc_extrst: Output::new(pin('H', 2), Level::Low),
            first_release: true,
        }
    }

    pub fn hold_bootmcu_in_reset(&mut self) {
        for inst in 1..=4u8 {
            set_spim_ext_mux_rot(inst);
        }

        self.flash_power_l2.set_high();
        self.flash_power_l3.set_high();
        self.bmc_extrst.set_low();
        self.bmc_srst.set_low();
        self.sgpio.set_output_pin(SGPO_AUX_PWRGD_CPU0, false);
        self.sgpio.set_output_pin(SGPO_AUX_PWRGD_CPU1, false);
        self.sgpio.set_output_pin(SGPO_PLTRST_CPU0, false);
        self.sgpio.set_output_pin(SGPO_PLTRST_CPU1, false);
    }

    pub async fn release_bootmcu_from_reset(&mut self) {
        self.flash_power_l2.set_high();
        self.flash_power_l3.set_high();

        reset_target_flash(Controller::Spi1, 0).await;
        reset_target_flash(Controller::Spi2, 0).await;
        reset_target_flash(Controller::Spi2, 1).await;

        for inst in 1..=4u8 {
            set_spim_ext_mux_target(inst);
        }

        Timer::after(Duration::from_millis(10)).await;

        if self.first_release {
            self.bmc_srst.set_high();
            self.first_release = false;
            Timer::after(Duration::from_millis(10)).await;
        }

        self.bmc_extrst.set_high();
        Timer::after(Duration::from_millis(10)).await;

        self.sgpio.set_output_pin(SGPO_AUX_PWRGD_CPU0, true);
        self.sgpio.set_output_pin(SGPO_AUX_PWRGD_CPU1, true);
        Timer::after(Duration::from_millis(10)).await;
    }

    pub fn apply_spi_filter_policy(&mut self, policy: &SpiFilterPolicy) {
        apply_ast1060_spi_policy(policy);
    }
}

#[cfg(feature = "ast1060")]
async fn reset_target_flash(controller: Controller, ce: u8) {
    let bus = SpiBus::new(controller, ce);
    bus.reset_by_command().await;
}

#[cfg(feature = "ast1060")]
fn set_spim_ext_mux_rot(inst: u8) {
    set_spim_ext_mux(inst, true);
}

#[cfg(feature = "ast1060")]
fn set_spim_ext_mux_target(inst: u8) {
    set_spim_ext_mux(inst, false);
}

#[cfg(feature = "ast1060")]
fn set_spim_ext_mux(inst: u8, rot_owner: bool) {
    if !(1..=4).contains(&inst) {
        return;
    }

    let bit = 1u32 << (12 + inst - 1);
    let reg = SCU_SPIM_MODE_CTRL as *mut u32;
    // SAFETY: SCU_SPIM_MODE_CTRL is the AST1060 SCU SPIM mode control register.
    // This read-modify-write only updates the documented external mux select bit
    // for one SPIPF instance and runs during single-threaded boot sequencing.
    unsafe {
        let mut value = core::ptr::read_volatile(reg);
        if rot_owner {
            value |= bit;
        } else {
            value &= !bit;
        }
        core::ptr::write_volatile(reg, value);
    }
}
