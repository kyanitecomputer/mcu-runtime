//! Flash storage and image management.
//!
//! # Image management
//!
//! - **A/B bank scheme**: Active, staging, and recovery image slots.
//! - **Anti-rollback**: Monotonic counters in OTP prevent downgrade attacks.
//! - **Recovery**: Autonomous restore from golden image on failure.
//!
//! # SPI filter engine
//!
//! Address-range write protection is derived from PFM policies; the
//! [`Layout`] and [`RecoveryPlan`] types are the hardware-agnostic source of
//! truth. AST1060 uses four QSPI monitor instances; AST1080 uses three.

/// Logical flash image slot.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Active,
    Recovery,
    Staging,
    State,
    Key,
}

/// A byte range within a flash device.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub slot: Slot,
    pub offset: u32,
    pub size: u32,
}

impl Region {
    /// Create a region.
    pub const fn new(slot: Slot, offset: u32, size: u32) -> Self {
        Self { slot, offset, size }
    }

    /// Exclusive end offset using checked arithmetic.
    ///
    /// Returns `None` if `offset + size` would overflow `u32`.
    pub const fn checked_end(&self) -> Option<u32> {
        self.offset.checked_add(self.size)
    }

    /// Exclusive end offset using saturating arithmetic.
    ///
    /// Prefer [`checked_end`](Self::checked_end) when overflow detection matters.
    #[deprecated(since = "0.1.0", note = "use `checked_end()` to detect overflow")]
    pub const fn end(&self) -> u32 {
        self.offset.saturating_add(self.size)
    }

    /// Return `true` if `[offset, offset+len)` is entirely within this region.
    pub const fn contains(&self, offset: u32, len: u32) -> bool {
        match (self.checked_end(), offset.checked_add(len)) {
            (Some(region_end), Some(end)) => offset >= self.offset && end <= region_end,
            _ => false,
        }
    }

    /// Return `true` if the region has zero size.
    pub const fn is_empty(&self) -> bool {
        self.size == 0
    }
}

/// Flash layout validation error.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutError {
    EmptyRegion,
    RegionOverflow,
    OverlappingRegions,
}

/// Static flash layout used by manifest and recovery logic.
///
/// `N` is the number of regions; use the smallest `N` that covers your
/// flash partition table so the type fits in on-stack space.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout<const N: usize> {
    regions: [Region; N],
}

impl<const N: usize> Layout<N> {
    /// Create a layout from a fixed array of regions.
    pub const fn new(regions: [Region; N]) -> Self {
        Self { regions }
    }

    /// Return all regions.
    pub const fn regions(&self) -> &[Region; N] {
        &self.regions
    }

    /// Return the first region matching `slot`, if any.
    pub fn find(&self, slot: Slot) -> Option<Region> {
        self.regions.iter().find(|r| r.slot == slot).copied()
    }

    /// Return the first region that fully contains `[offset, offset+len)`.
    pub fn find_containing(&self, offset: u32, len: u32) -> Option<Region> {
        self.regions
            .iter()
            .find(|r| r.contains(offset, len))
            .copied()
    }

    /// Validate that no region is empty, overflows, or overlaps another.
    pub fn validate(&self) -> Result<(), LayoutError> {
        let mut i = 0;
        while i < N {
            let a = self.regions[i];
            if a.is_empty() {
                return Err(LayoutError::EmptyRegion);
            }
            let a_end = match a.checked_end() {
                Some(end) => end,
                None => return Err(LayoutError::RegionOverflow),
            };

            let mut j = i + 1;
            while j < N {
                let b = self.regions[j];
                let b_end = match b.checked_end() {
                    Some(end) => end,
                    None => return Err(LayoutError::RegionOverflow),
                };
                if a.offset < b_end && b.offset < a_end {
                    return Err(LayoutError::OverlappingRegions);
                }
                j += 1;
            }
            i += 1;
        }
        Ok(())
    }
}

// ── RecoveryPair / RecoveryPlan ───────────────────────────────────────────────

/// Pairing between an active image and its recovery source.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryPair {
    pub active: Region,
    pub recovery: Region,
}

impl RecoveryPair {
    /// Create a recovery pair.
    pub const fn new(active: Region, recovery: Region) -> Self {
        Self { active, recovery }
    }

    /// Return `true` if the recovery region is large enough to replace active.
    pub const fn is_compatible(&self) -> bool {
        !self.active.is_empty()
            && !self.recovery.is_empty()
            && self.recovery.size >= self.active.size
    }
}

/// Backend-independent recovery plan.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryPlan<const N: usize> {
    pairs: [RecoveryPair; N],
}

/// Recovery-plan validation error.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryPlanError {
    EmptyPlan,
    IncompatiblePair,
}

impl<const N: usize> RecoveryPlan<N> {
    /// Create a recovery plan from fixed pairs.
    pub const fn new(pairs: [RecoveryPair; N]) -> Self {
        Self { pairs }
    }

    /// Return all pairs.
    pub const fn pairs(&self) -> &[RecoveryPair; N] {
        &self.pairs
    }

    /// Return `true` if any pair can restore its active region.
    pub fn validate(&self) -> Result<(), RecoveryPlanError> {
        if N == 0 {
            return Err(RecoveryPlanError::EmptyPlan);
        }
        if self.pairs.iter().any(|p| !p.is_compatible()) {
            return Err(RecoveryPlanError::IncompatiblePair);
        }
        Ok(())
    }
}

// ── RecoveryLevel ─────────────────────────────────────────────────────────────

/// Recovery attempt level independent of storage or OTP backend.
///
/// Level 0 = first attempt; level [`RecoveryLevel::MAX`] = final attempt.
/// After `MAX` the platform should emit [`crate::pfr::Event::RecoveryFailed`].
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RecoveryLevel(u8);

impl RecoveryLevel {
    /// First recovery attempt.
    pub const ZERO: Self = Self(0);
    /// Maximum (final) recovery level before lockdown.
    pub const MAX: Self = Self(2);

    /// Create a level, returning `None` if `value > MAX`.
    pub const fn new(value: u8) -> Option<Self> {
        if value <= Self::MAX.0 {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Return the raw level value.
    pub const fn get(self) -> u8 {
        self.0
    }

    /// Return `true` if another recovery attempt is possible.
    pub const fn can_escalate(self) -> bool {
        self.0 < Self::MAX.0
    }

    /// Return the next level, or `None` if already at [`MAX`](Self::MAX).
    pub const fn escalate(self) -> Option<Self> {
        if self.can_escalate() {
            Some(Self(self.0 + 1))
        } else {
            None
        }
    }
}

impl Default for RecoveryLevel {
    fn default() -> Self {
        Self::ZERO
    }
}
