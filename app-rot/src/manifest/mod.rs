//! Manifest management: PFM, CFM, PCD.
//!
//! All manifests are signed. The PFR core never operates without a verified
//! manifest. Platform implementations provide parsers via
//! [`crate::platform::ManifestParser`].
//!
//! | Manifest | Purpose |
//! |----------|---------|
//! | **PFM** | Expected SPI flash layout, address ranges, permissions |
//! | **CFM** | Component topology and expected measurements |
//! | **PCD** | Platform-specific configuration (GPIO, I2C, timeouts) |

// ── RegionPermissions ─────────────────────────────────────────────────────────

/// Access policy for a PFM flash region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionPermissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl RegionPermissions {
    pub const READ_ONLY: Self = Self {
        read: true,
        write: false,
        execute: false,
    };
    pub const READ_EXECUTE: Self = Self {
        read: true,
        write: false,
        execute: true,
    };
    pub const READ_WRITE: Self = Self {
        read: true,
        write: true,
        execute: false,
    };

    /// Return `true` if at least one access bit is set.
    pub const fn is_any_set(&self) -> bool {
        self.read || self.write || self.execute
    }
}

// ── Algorithm enums ───────────────────────────────────────────────────────────

/// Hash algorithm named by a manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Sha256,
    Sha384,
    Sha512,
}

impl HashAlgorithm {
    /// Expected digest length in bytes for this algorithm.
    pub const fn digest_len(self) -> usize {
        match self {
            Self::Sha256 => 32,
            Self::Sha384 => 48,
            Self::Sha512 => 64,
        }
    }
}

/// Signature algorithm named by a manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureAlgorithm {
    EcdsaP384,
    Rsa2048,
    Rsa3072,
    Rsa4096,
}

// ── ProtectedRegion / ImageDescriptor ─────────────────────────────────────────

/// One protected address range from a PFM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtectedRegion {
    pub offset: u32,
    pub size: u32,
    pub permissions: RegionPermissions,
}

impl ProtectedRegion {
    /// Exclusive end offset using checked arithmetic.
    ///
    /// Returns `None` on overflow.
    pub const fn checked_end(&self) -> Option<u32> {
        self.offset.checked_add(self.size)
    }

    /// Exclusive end using saturating arithmetic.
    ///
    /// Prefer [`checked_end`](Self::checked_end) when overflow detection matters.
    #[deprecated(since = "0.1.0", note = "use `checked_end()` to detect overflow")]
    pub const fn end(&self) -> u32 {
        self.offset.saturating_add(self.size)
    }

    fn contains_image(&self, image: &ImageDescriptor) -> bool {
        match (self.checked_end(), image.checked_end()) {
            (Some(region_end), Some(image_end)) => {
                image.offset >= self.offset && image_end <= region_end
            }
            _ => false,
        }
    }
}

/// Image metadata required for hashing and signature verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageDescriptor {
    pub offset: u32,
    pub size: u32,
    pub hash: HashAlgorithm,
    pub signature: SignatureAlgorithm,
    pub svn: u32,
}

impl ImageDescriptor {
    /// Exclusive end offset; `None` on overflow.
    pub const fn checked_end(&self) -> Option<u32> {
        self.offset.checked_add(self.size)
    }
}

// ── PlatformFirmwareManifest ──────────────────────────────────────────────────

/// Minimal PFM model independent of any wire format.
///
/// Holds borrowed slices so no allocation is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformFirmwareManifest<'a> {
    pub version: u32,
    pub protected_regions: &'a [ProtectedRegion],
    pub images: &'a [ImageDescriptor],
}

impl<'a> PlatformFirmwareManifest<'a> {
    /// Return the [`VerificationTask`] at `index`, if it exists.
    pub fn verification_task(&self, index: usize) -> Option<VerificationTask> {
        self.images.get(index).copied().map(VerificationTask::from)
    }

    /// Number of image verification tasks named by this manifest.
    pub const fn verification_task_count(&self) -> usize {
        self.images.len()
    }
}

// ── VerificationTask / VerificationRequest / VerificationOutcome ──────────────

/// Parser-independent work item consumed by the verification engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationTask {
    pub offset: u32,
    pub size: u32,
    pub hash: HashAlgorithm,
    pub signature: SignatureAlgorithm,
    pub svn: u32,
}

impl From<ImageDescriptor> for VerificationTask {
    fn from(image: ImageDescriptor) -> Self {
        Self {
            offset: image.offset,
            size: image.size,
            hash: image.hash,
            signature: image.signature,
            svn: image.svn,
        }
    }
}

/// Work item combining a task with the key needed to verify it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationRequest {
    pub task: VerificationTask,
    pub key_kind: KeyKind,
    pub key_index: u8,
}

impl VerificationRequest {
    /// Create a request pairing a task with a named key slot.
    pub const fn new(task: VerificationTask, key_kind: KeyKind, key_index: u8) -> Self {
        Self {
            task,
            key_kind,
            key_index,
        }
    }
}

/// Outcome of a single image verification attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationOutcome {
    Passed,
    HashMismatch,
    SignatureMismatch,
    KeyNotFound,
    RollbackDetected,
    InvalidDescriptor,
}

impl VerificationOutcome {
    /// Return `true` if verification passed.
    pub const fn is_pass(&self) -> bool {
        match self {
            Self::Passed => true,
            _ => false,
        }
    }

    /// Return `true` if verification failed.
    pub const fn is_fail(&self) -> bool {
        !self.is_pass()
    }
}

/// Fixed-capacity FIFO of verification work items; no heap required.
pub struct VerificationQueue<const N: usize> {
    items: [Option<VerificationRequest>; N],
    len: usize,
}

impl<const N: usize> VerificationQueue<N> {
    /// Create an empty queue.
    pub const fn new() -> Self {
        Self {
            items: [None; N],
            len: 0,
        }
    }

    /// Enqueue a request.  Returns `false` if the queue is full.
    pub fn push(&mut self, request: VerificationRequest) -> bool {
        if self.len >= N {
            return false;
        }
        self.items[self.len] = Some(request);
        self.len += 1;
        true
    }

    /// Dequeue the oldest request, if any.
    pub fn pop(&mut self) -> Option<VerificationRequest> {
        if self.len == 0 {
            return None;
        }
        let item = self.items[0].take();
        let mut i = 1;
        while i < self.len {
            self.items[i - 1] = self.items[i].take();
            i += 1;
        }
        self.len -= 1;
        item
    }

    /// Number of items currently in the queue.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Return `true` if the queue contains no items.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return `true` if the queue has no remaining capacity.
    pub const fn is_full(&self) -> bool {
        self.len >= N
    }
}

impl<const N: usize> Default for VerificationQueue<N> {
    fn default() -> Self {
        Self::new()
    }
}

// ── Key types ─────────────────────────────────────────────────────────────────

/// Public-key class referenced by a signed manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    Root,
    Pfm,
    Afm,
    Recovery,
}

/// Parser-independent key metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyDescriptor {
    pub kind: KeyKind,
    pub index: u8,
    pub hash: HashAlgorithm,
    pub digest_offset: u32,
    pub digest_size: u16,
}

impl KeyDescriptor {
    /// Return `true` if this descriptor has a non-empty digest range.
    pub const fn has_digest(&self) -> bool {
        self.digest_size != 0
    }

    /// Exclusive end of the digest in flash; `None` on overflow.
    pub const fn checked_digest_end(&self) -> Option<u32> {
        self.digest_offset.checked_add(self.digest_size as u32)
    }
}

/// A collection of key descriptors for a signed image set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyManifest<'a> {
    pub keys: &'a [KeyDescriptor],
}

impl<'a> KeyManifest<'a> {
    /// Create a key manifest from a borrowed slice.
    pub const fn new(keys: &'a [KeyDescriptor]) -> Self {
        Self { keys }
    }

    /// Return the first descriptor matching `kind` and `index`.
    pub fn find(&self, kind: KeyKind, index: u8) -> Option<KeyDescriptor> {
        self.keys
            .iter()
            .find(|k| k.kind == kind && k.index == index)
            .copied()
    }

    /// Return `true` if there are no key descriptors.
    pub const fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

// ── AFM types ─────────────────────────────────────────────────────────────────

/// Attestation firmware manifest descriptor independent of wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AfmDescriptor {
    pub offset: u32,
    pub size: u32,
    pub svn: u32,
    pub hash: HashAlgorithm,
}

impl AfmDescriptor {
    /// Exclusive end offset; `None` on overflow.
    pub const fn checked_end(&self) -> Option<u32> {
        self.offset.checked_add(self.size)
    }
}

/// A collection of AFM descriptors for the attestation subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AfmManifest<'a> {
    pub descriptors: &'a [AfmDescriptor],
}

impl<'a> AfmManifest<'a> {
    /// Create an AFM manifest from a borrowed slice.
    pub const fn new(descriptors: &'a [AfmDescriptor]) -> Self {
        Self { descriptors }
    }

    /// Number of AFM descriptors.
    pub const fn len(&self) -> usize {
        self.descriptors.len()
    }

    /// Return `true` if there are no descriptors.
    pub const fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }
}

// ── Anti-rollback ─────────────────────────────────────────────────────────────

/// Anti-rollback policy supplied by storage or OTP-specific code.
///
/// Deliberately agnostic of OTP hardware; the OTP driver resolves the
/// minimum SVN and passes it in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AntiRollbackPolicy {
    pub minimum_svn: u32,
}

impl AntiRollbackPolicy {
    /// Return `true` if the supplied SVN is acceptable.
    pub const fn accepts(&self, svn: u32) -> bool {
        svn >= self.minimum_svn
    }
}

// ── Error types ───────────────────────────────────────────────────────────────

/// Manifest-level policy validation error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestError {
    EmptyImages,
    EmptyProtectedRegion,
    EmptyImage,
    EmptyPermissions,
    RegionOverflow,
    ImageOverflow,
    OverlappingProtectedRegions,
    ImageOutsideProtectedRegion,
    RollbackDetected,
}

/// Key descriptor validation error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestKeyError {
    EmptyDigest,
    DigestOverflow,
}

/// AFM descriptor validation error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfmError {
    EmptyDescriptor,
    Overflow,
}

// ── Validation functions ──────────────────────────────────────────────────────

/// Validate a key descriptor for self-consistency.
pub fn validate_key_descriptor(key: &KeyDescriptor) -> Result<(), ManifestKeyError> {
    if !key.has_digest() {
        return Err(ManifestKeyError::EmptyDigest);
    }
    if key.checked_digest_end().is_none() {
        return Err(ManifestKeyError::DigestOverflow);
    }
    Ok(())
}

/// Validate an AFM descriptor for self-consistency.
pub fn validate_afm_descriptor(afm: &AfmDescriptor) -> Result<(), AfmError> {
    if afm.size == 0 {
        return Err(AfmError::EmptyDescriptor);
    }
    if afm.checked_end().is_none() {
        return Err(AfmError::Overflow);
    }
    Ok(())
}

/// Validate image descriptors against an anti-rollback policy.
pub fn validate_anti_rollback(
    manifest: &PlatformFirmwareManifest<'_>,
    policy: AntiRollbackPolicy,
) -> Result<(), ManifestError> {
    for image in manifest.images {
        if !policy.accepts(image.svn) {
            return Err(ManifestError::RollbackDetected);
        }
    }
    Ok(())
}

/// Validate a PFM for structural correctness.
///
/// Checks: non-empty image list; no empty/overflowing/overlapping regions;
/// every image is covered by a protected region.
pub fn validate_manifest(manifest: &PlatformFirmwareManifest<'_>) -> Result<(), ManifestError> {
    if manifest.images.is_empty() {
        return Err(ManifestError::EmptyImages);
    }

    for region in manifest.protected_regions {
        if region.size == 0 {
            return Err(ManifestError::EmptyProtectedRegion);
        }
        if !region.permissions.is_any_set() {
            return Err(ManifestError::EmptyPermissions);
        }
        if region.checked_end().is_none() {
            return Err(ManifestError::RegionOverflow);
        }
    }

    for image in manifest.images {
        if image.size == 0 {
            return Err(ManifestError::EmptyImage);
        }
        if image.checked_end().is_none() {
            return Err(ManifestError::ImageOverflow);
        }
        if !manifest
            .protected_regions
            .iter()
            .any(|r| r.contains_image(image))
        {
            return Err(ManifestError::ImageOutsideProtectedRegion);
        }
    }

    // O(N²) overlap check — N is small in practice (≤16 regions).
    let mut i = 0;
    while i < manifest.protected_regions.len() {
        let a = manifest.protected_regions[i];
        let a_end = match a.checked_end() {
            Some(end) => end,
            None => return Err(ManifestError::RegionOverflow),
        };
        let mut j = i + 1;
        while j < manifest.protected_regions.len() {
            let b = manifest.protected_regions[j];
            let b_end = match b.checked_end() {
                Some(end) => end,
                None => return Err(ManifestError::RegionOverflow),
            };
            if a.offset < b_end && b.offset < a_end {
                return Err(ManifestError::OverlappingProtectedRegions);
            }
            j += 1;
        }
        i += 1;
    }

    Ok(())
}
