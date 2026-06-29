//! Firmware image verification engine.

use crate::flash::Slot;
use crate::manifest::{
    validate_anti_rollback, validate_manifest, AntiRollbackPolicy, PlatformFirmwareManifest,
    VerificationOutcome, VerificationTask,
};
use crate::platform::{FlashReader, Hasher, SignatureVerifier};

/// Detached signature material for one verification task.
pub struct VerificationMaterial<'a> {
    pub expected_digest: &'a [u8],
    pub signature: &'a [u8],
    pub public_key: &'a [u8],
}

/// Supplies signature material for manifest verification tasks.
pub trait VerificationMaterialProvider {
    /// Return expected digest, signature, and public key for `task`.
    fn material_for(&self, task: &VerificationTask) -> Option<VerificationMaterial<'_>>;
}

/// Fixed-buffer image verifier.
pub struct ImageVerifier<const IMAGE_BUF: usize, const DIGEST_BUF: usize> {
    image: [u8; IMAGE_BUF],
    digest: [u8; DIGEST_BUF],
}

impl<const IMAGE_BUF: usize, const DIGEST_BUF: usize> ImageVerifier<IMAGE_BUF, DIGEST_BUF> {
    /// Create a verifier with fixed stack/struct-owned buffers.
    pub const fn new() -> Self {
        Self {
            image: [0; IMAGE_BUF],
            digest: [0; DIGEST_BUF],
        }
    }

    /// Verify every image named by `manifest` from `slot`.
    pub async fn verify_manifest<F, C, M>(
        &mut self,
        flash: &F,
        crypto: &C,
        material: &M,
        slot: Slot,
        manifest: &PlatformFirmwareManifest<'_>,
        rollback: AntiRollbackPolicy,
    ) -> Result<(), VerificationError<F::Error, <C as Hasher>::Error>>
    where
        F: FlashReader,
        C: Hasher + SignatureVerifier<Error = <C as Hasher>::Error>,
        M: VerificationMaterialProvider,
    {
        validate_manifest(manifest).map_err(VerificationError::Manifest)?;
        validate_anti_rollback(manifest, rollback).map_err(VerificationError::Manifest)?;

        log::info!(
            "verifying {} firmware image(s)",
            manifest.verification_task_count()
        );

        for image in manifest.images {
            let task = VerificationTask::from(*image);
            self.verify_task(flash, crypto, material, slot, &task)
                .await?;
        }

        log::info!("firmware manifest verification passed");

        Ok(())
    }

    /// Verify `manifest`, or explicitly allow unprovisioned mode when no images exist.
    pub async fn verify_manifest_or_unprovisioned<F, C, M>(
        &mut self,
        flash: &F,
        crypto: &C,
        material: &M,
        slot: Slot,
        manifest: &PlatformFirmwareManifest<'_>,
        rollback: AntiRollbackPolicy,
    ) -> Result<(), VerificationError<F::Error, <C as Hasher>::Error>>
    where
        F: FlashReader,
        C: Hasher + SignatureVerifier<Error = <C as Hasher>::Error>,
        M: VerificationMaterialProvider,
    {
        if manifest.images.is_empty() {
            log::warn!("no firmware manifest images; unprovisioned verification bypass");
            return Ok(());
        }

        self.verify_manifest(flash, crypto, material, slot, manifest, rollback)
            .await
    }

    /// Verify one image task from `slot`.
    pub async fn verify_task<F, C, M>(
        &mut self,
        flash: &F,
        crypto: &C,
        material: &M,
        slot: Slot,
        task: &VerificationTask,
    ) -> Result<(), VerificationError<F::Error, <C as Hasher>::Error>>
    where
        F: FlashReader,
        C: Hasher + SignatureVerifier<Error = <C as Hasher>::Error>,
        M: VerificationMaterialProvider,
    {
        let image_len = usize::try_from(task.size).map_err(|_| VerificationError::ImageTooLarge)?;
        if image_len == 0 || image_len > IMAGE_BUF {
            return Err(VerificationError::ImageTooLarge);
        }
        if task.hash.digest_len() > DIGEST_BUF {
            return Err(VerificationError::DigestBufferTooSmall);
        }

        let material = material
            .material_for(task)
            .ok_or(VerificationError::KeyNotFound)?;
        if material.expected_digest.len() != task.hash.digest_len() {
            return Err(VerificationError::InvalidMaterial);
        }

        flash
            .read(slot, task.offset, &mut self.image[..image_len])
            .map_err(VerificationError::Flash)?;

        let digest_len = crypto
            .digest(task.hash, &self.image[..image_len], &mut self.digest)
            .await
            .map_err(VerificationError::Crypto)?;
        let digest = &self.digest[..digest_len];

        if digest != material.expected_digest {
            log::error!("firmware hash mismatch at offset=0x{:08x}", task.offset);
            return Err(VerificationError::Outcome(
                VerificationOutcome::HashMismatch,
            ));
        }

        crypto
            .verify(
                task.signature,
                digest,
                material.signature,
                material.public_key,
            )
            .await
            .map_err(|_| VerificationError::Outcome(VerificationOutcome::SignatureMismatch))?;

        log::info!(
            "firmware image verified offset=0x{:08x} size={}",
            task.offset,
            task.size
        );

        Ok(())
    }
}

impl<const IMAGE_BUF: usize, const DIGEST_BUF: usize> Default
    for ImageVerifier<IMAGE_BUF, DIGEST_BUF>
{
    fn default() -> Self {
        Self::new()
    }
}

/// Verification engine error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationError<F, C> {
    Manifest(crate::manifest::ManifestError),
    ImageTooLarge,
    DigestBufferTooSmall,
    InvalidMaterial,
    KeyNotFound,
    Flash(F),
    Crypto(C),
    Outcome(VerificationOutcome),
}

/// Material provider for unprovisioned development mode.
pub struct NoVerificationMaterial;

impl VerificationMaterialProvider for NoVerificationMaterial {
    fn material_for(&self, _task: &VerificationTask) -> Option<VerificationMaterial<'_>> {
        None
    }
}
