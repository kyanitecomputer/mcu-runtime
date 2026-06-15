//! AST1060 runtime backends for flash and crypto services.

use embassy_aspeed::ecdsa::{Ecdsa, EcdsaError};
use embassy_aspeed::hace::{Hace, HaceError, HashAlgo};
use embassy_aspeed::rsa::{DigestAlgo, Rsa, RsaError};
use embassy_aspeed::spi::{Controller, SpiBus, SpiError};

use crate::flash::Slot;
use crate::manifest::{HashAlgorithm, SignatureAlgorithm};
use crate::platform::{FlashReader, FlashWriter, Hasher, SignatureVerifier};

/// Mapping from a logical firmware slot to one AST1060 SPI chip-select range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlashSlot {
    pub slot: Slot,
    pub controller: Controller,
    pub ce: u8,
    pub offset: u32,
    pub size: u32,
}

/// AST1060 flash backend over FMC/SPI memory windows.
pub struct Ast1060Flash<const N: usize> {
    slots: [FlashSlot; N],
}

impl<const N: usize> Ast1060Flash<N> {
    /// Create a flash backend from static slot mappings.
    pub const fn new(slots: [FlashSlot; N]) -> Self {
        Self { slots }
    }

    fn find(&self, slot: Slot, offset: u32, len: usize) -> Result<FlashSlot, Ast1060FlashError> {
        let len = u32::try_from(len).map_err(|_| Ast1060FlashError::OutOfRange)?;
        let end = offset
            .checked_add(len)
            .ok_or(Ast1060FlashError::OutOfRange)?;
        self.slots
            .iter()
            .find(|mapping| mapping.slot == slot && end <= mapping.size)
            .copied()
            .ok_or(Ast1060FlashError::OutOfRange)
    }

    fn bus(mapping: FlashSlot) -> Result<SpiBus, Ast1060FlashError> {
        if mapping.ce > 1 {
            return Err(Ast1060FlashError::InvalidChipSelect);
        }
        Ok(SpiBus::new(mapping.controller, mapping.ce))
    }
}

impl<const N: usize> FlashReader for Ast1060Flash<N> {
    type Error = Ast1060FlashError;

    fn read(&self, slot: Slot, offset: u32, buf: &mut [u8]) -> Result<(), Self::Error> {
        let mapping = self.find(slot, offset, buf.len())?;
        let bus = Self::bus(mapping)?;
        let physical_offset = mapping
            .offset
            .checked_add(offset)
            .ok_or(Ast1060FlashError::OutOfRange)?;
        bus.read_memory_mapped(physical_offset, buf);
        Ok(())
    }

    fn region_size(&self, slot: Slot) -> Option<u32> {
        self.slots
            .iter()
            .find(|mapping| mapping.slot == slot)
            .map(|mapping| mapping.size)
    }
}

impl<const N: usize> FlashWriter for Ast1060Flash<N> {
    async fn write(
        &mut self,
        slot: Slot,
        mut offset: u32,
        mut data: &[u8],
    ) -> Result<(), Self::Error> {
        let mapping = self.find(slot, offset, data.len())?;
        let bus = Self::bus(mapping)?;

        while !data.is_empty() {
            let page_remaining = 256 - (offset as usize & 0xff);
            let chunk_len = data.len().min(page_remaining);
            let physical_offset = mapping
                .offset
                .checked_add(offset)
                .ok_or(Ast1060FlashError::OutOfRange)?;

            bus.write_page(physical_offset, &data[..chunk_len]).await?;
            offset = offset
                .checked_add(chunk_len as u32)
                .ok_or(Ast1060FlashError::OutOfRange)?;
            data = &data[chunk_len..];
        }

        Ok(())
    }

    async fn erase_sector(&mut self, slot: Slot, sector_offset: u32) -> Result<(), Self::Error> {
        let mapping = self.find(slot, sector_offset, 4096)?;
        let bus = Self::bus(mapping)?;
        let physical_offset = mapping
            .offset
            .checked_add(sector_offset)
            .ok_or(Ast1060FlashError::OutOfRange)?;
        bus.erase_sector(physical_offset).await?;
        Ok(())
    }
}

/// AST1060 flash backend error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ast1060FlashError {
    OutOfRange,
    InvalidChipSelect,
    Spi(SpiError),
}

impl From<SpiError> for Ast1060FlashError {
    fn from(error: SpiError) -> Self {
        Self::Spi(error)
    }
}

/// AST1060 crypto backend over HACE, RSA, and ECDSA hardware.
pub struct Ast1060Crypto {
    hace: Hace,
}

impl Ast1060Crypto {
    /// Create an AST1060 crypto backend.
    pub const fn new() -> Self {
        Self { hace: Hace::new() }
    }
}

impl Default for Ast1060Crypto {
    fn default() -> Self {
        Self::new()
    }
}

impl Hasher for Ast1060Crypto {
    type Error = Ast1060CryptoError;

    async fn digest(
        &self,
        algorithm: HashAlgorithm,
        data: &[u8],
        out: &mut [u8],
    ) -> Result<usize, Self::Error> {
        let algo = match algorithm {
            HashAlgorithm::Sha256 => HashAlgo::Sha256,
            HashAlgorithm::Sha384 => HashAlgo::Sha384,
            HashAlgorithm::Sha512 => HashAlgo::Sha512,
        };
        self.hace.hash(algo, data, out).await?;
        Ok(algorithm.digest_len())
    }
}

impl SignatureVerifier for Ast1060Crypto {
    type Error = Ast1060CryptoError;

    async fn verify(
        &self,
        algorithm: SignatureAlgorithm,
        digest: &[u8],
        signature: &[u8],
        public_key: &[u8],
    ) -> Result<(), Self::Error> {
        match algorithm {
            SignatureAlgorithm::EcdsaP384 => verify_ecdsa_p384(digest, signature, public_key).await,
            SignatureAlgorithm::Rsa2048 => verify_rsa(digest, signature, public_key, 2048).await,
            SignatureAlgorithm::Rsa3072 => verify_rsa(digest, signature, public_key, 3072).await,
            SignatureAlgorithm::Rsa4096 => verify_rsa(digest, signature, public_key, 4096).await,
        }
    }
}

async fn verify_rsa(
    digest: &[u8],
    signature: &[u8],
    public_key: &[u8],
    mod_bits: u32,
) -> Result<(), Ast1060CryptoError> {
    let modulus_len = (mod_bits / 8) as usize;
    if signature.len() != modulus_len || public_key.len() <= modulus_len {
        return Err(Ast1060CryptoError::InvalidLength);
    }
    let digest_algo = digest_algo_from_len(digest.len())?;
    let modulus = &public_key[..modulus_len];
    let exponent = &public_key[modulus_len..];
    let exp_bits =
        u32::try_from(exponent.len() * 8).map_err(|_| Ast1060CryptoError::InvalidLength)?;

    if Rsa::verify(
        signature,
        digest,
        digest_algo,
        modulus,
        exponent,
        mod_bits,
        exp_bits,
    )
    .await?
    {
        Ok(())
    } else {
        Err(Ast1060CryptoError::InvalidSignature)
    }
}

async fn verify_ecdsa_p384(
    digest: &[u8],
    signature: &[u8],
    public_key: &[u8],
) -> Result<(), Ast1060CryptoError> {
    if digest.len() != 48 || signature.len() != 96 || public_key.len() != 96 {
        return Err(Ast1060CryptoError::InvalidLength);
    }

    let qx = array_ref::<48>(&public_key[..48])?;
    let qy = array_ref::<48>(&public_key[48..])?;
    let sig_r = array_ref::<48>(&signature[..48])?;
    let sig_s = array_ref::<48>(&signature[48..])?;
    let digest = array_ref::<48>(digest)?;

    if Ecdsa::verify(qx, qy, sig_r, sig_s, digest).await? {
        Ok(())
    } else {
        Err(Ast1060CryptoError::InvalidSignature)
    }
}

fn array_ref<const N: usize>(data: &[u8]) -> Result<&[u8; N], Ast1060CryptoError> {
    data.try_into()
        .map_err(|_| Ast1060CryptoError::InvalidLength)
}

fn digest_algo_from_len(len: usize) -> Result<DigestAlgo, Ast1060CryptoError> {
    match len {
        32 => Ok(DigestAlgo::Sha256),
        48 => Ok(DigestAlgo::Sha384),
        64 => Ok(DigestAlgo::Sha512),
        _ => Err(Ast1060CryptoError::InvalidLength),
    }
}

/// AST1060 crypto backend error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ast1060CryptoError {
    InvalidLength,
    InvalidSignature,
    Hace(HaceError),
    Rsa(RsaError),
    Ecdsa(EcdsaError),
}

impl From<HaceError> for Ast1060CryptoError {
    fn from(error: HaceError) -> Self {
        Self::Hace(error)
    }
}

impl From<RsaError> for Ast1060CryptoError {
    fn from(error: RsaError) -> Self {
        Self::Rsa(error)
    }
}

impl From<EcdsaError> for Ast1060CryptoError {
    fn from(error: EcdsaError) -> Self {
        Self::Ecdsa(error)
    }
}
