//! Atari 7800 cartridge digital signatures.
//!
//! Rust port of `sign7800.c` ("7800sign", version 1.0 of 2004-06-20) by Bruce
//! Tomlin, distributed under the LGPL (see the `LICENSE` file in this crate).
//!
//! A signed cartridge carries a 120-byte signature at `$FF80..$FFF8`. The
//! scheme is a Rabin-style signature: the cartridge is reduced to a 120-byte
//! hash, and the signature is a square root of that hash modulo `n = p * q`.
//! Verifying therefore only needs `signature² mod n`, while signing needs the
//! factors `p` and `q`, which are built into this tool (as they are in
//! `sign7800.c`).
//!
//! The original implements its big-number arithmetic with hand-rolled
//! byte-array routines and global state; this port uses [`num_bigint`]
//! instead. The results are identical (the test suite pins a signature
//! produced by the original C tool).

mod tables;

use num_bigint::BigUint;
use std::fmt;
use std::io::{self, Seek, SeekFrom, Write};
use tables::{AP, AQ, N, P, PERM, P_EXP, Q, Q_EXP};

/// Length in bytes of a cartridge signature (and of the hash it encrypts).
pub const SIGNATURE_LEN: usize = 0x78;

/// The 120-byte hash of a cartridge, which a valid signature decrypts to.
pub type Digest = [u8; SIGNATURE_LEN];

/// Size of the 6502 address space the cartridge is mapped into.
const MEM_SIZE: usize = 0x1_0000;
/// Only the last 48 KB of an image can ever be hashed.
const MAX_HASHED: usize = 0xC000;
/// Address of the signature within the address space.
const SIGNATURE_ADDR: usize = 0xFF80;
/// Offset from the end of an image (whose last byte is `$FFFF`) back to the
/// start of the signature area at `$FF80`.
const SIGNATURE_END_OFFSET: i64 = -128;

/// Errors from loading, validating or signing a cartridge image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SignError {
    /// The image size is not a multiple of 4 KB (plus an optional 128-byte header).
    NotMultipleOf4K,
    /// The image is smaller than 4 KB.
    TooSmall,
    /// `$FFF8` does not have the required `$F1` bits set.
    InvalidFff8,
    /// The hash start page in `$FFF9` lies below the start of the image.
    HashAreaTooLarge,
    /// The low nibble of `$FFF9` is not 3 or 7.
    InvalidFff9Nibble,
    /// The reset vector points below the hashed area.
    ResetOutsideHash,
    /// No hash with a square root modulo `n` was found in 256 attempts.
    NoSignableHash,
}

impl fmt::Display for SignError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignError::NotMultipleOf4K => write!(f, "Cartridge size not a multiple of 4K bytes!"),
            SignError::TooSmall => write!(f, "Cartridge data file must be at least 4K!"),
            SignError::InvalidFff8 => write!(f, "Invalid byte at $FFF8, should be $FF!"),
            SignError::HashAreaTooLarge => write!(
                f,
                "Invalid byte at $FFF9, hash space larger than cartridge image!"
            ),
            SignError::InvalidFff9Nibble => {
                write!(f, "Invalid byte at $FFF9, low nibble should be 3 or 7!")
            }
            SignError::ResetOutsideHash => {
                write!(f, "Cartridge reset vector points outside hashed area!")
            }
            SignError::NoSignableHash => {
                write!(f, "Could not find a signable hash for this cartridge.")
            }
        }
    }
}

impl std::error::Error for SignError {}

/// A 120-byte cartridge signature, stored at `$FF80..$FFF8`.
///
/// Its [`Display`](fmt::Display) output is a hex dump, 16 bytes per line,
/// matching the original tool (including the trailing blank line).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature([u8; SIGNATURE_LEN]);

impl Signature {
    #[must_use]
    pub fn from_bytes(bytes: [u8; SIGNATURE_LEN]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8; SIGNATURE_LEN] {
        &self.0
    }

    /// Write the signature into the signature area of a cartridge image: the
    /// 120 bytes starting 128 bytes before the end of `dest`. Nothing else in
    /// `dest` is modified.
    ///
    /// # Errors
    ///
    /// Returns an error if `dest` is shorter than 128 bytes or cannot be
    /// written to.
    pub fn write_to<W: Write + Seek>(&self, dest: &mut W) -> io::Result<()> {
        dest.seek(SeekFrom::End(SIGNATURE_END_OFFSET))?;
        dest.write_all(&self.0)
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for line in self.0.chunks(16) {
            for (i, byte) in line.iter().enumerate() {
                match i {
                    0 => {}
                    8 => write!(f, "  ")?,
                    _ => write!(f, " ")?,
                }
                write!(f, "{byte:02x}")?;
            }
            writeln!(f)?;
        }
        writeln!(f)
    }
}

/// The state of the signature already present in a cartridge image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureStatus {
    /// The signature decrypts to the cartridge's hash.
    Valid,
    /// The signature area is all `$00` or `$FF` bytes: never signed.
    Empty,
    /// The signature area holds something that is not a valid signature.
    Invalid,
}

/// Convert a value below `n` to a fixed-size big-endian array.
fn to_bytes(value: &BigUint) -> [u8; SIGNATURE_LEN] {
    let bytes = value.to_bytes_be();
    let mut out = [0u8; SIGNATURE_LEN];
    out[SIGNATURE_LEN - bytes.len()..].copy_from_slice(&bytes);
    out
}

/// Zero out the bits that are "don't care" when comparing hashes: the top
/// five bits (so the value stays below `n`) and byte 4, which is the byte
/// varied by [`encrypt_hash`] to find a hash that has a square root.
fn mask_hash(hash: &mut Digest) {
    hash[0] &= 0x07;
    hash[4] = 0;
}

/// Decrypt a signature (`signature² mod n`) and mask the result so it can be
/// compared against [`Cartridge::hash`].
#[must_use]
pub fn decrypt_signature(signature: &Signature) -> Digest {
    let n = BigUint::from_bytes_be(&N);
    let s = BigUint::from_bytes_be(&signature.0);
    let mut decrypted = to_bytes(&(&s * &s % &n));
    mask_hash(&mut decrypted);
    decrypted
}

/// The 960-bit Rabin/RSA private key used to sign Atari 7800 cartridges.
#[derive(Debug, Clone)]
pub struct SigningKey {
    n: BigUint,
    p: BigUint,
    q: BigUint,
    p_exp: BigUint,
    q_exp: BigUint,
    ap: BigUint,
    aq: BigUint,
}

impl Default for SigningKey {
    fn default() -> Self {
        Self::new()
    }
}

impl SigningKey {
    /// Construct the signing key from built-in Atari/GCC private key parameters.
    #[must_use]
    pub fn new() -> Self {
        Self {
            n: BigUint::from_bytes_be(&N),
            p: BigUint::from_bytes_be(&P),
            q: BigUint::from_bytes_be(&Q),
            p_exp: BigUint::from_bytes_be(&P_EXP),
            q_exp: BigUint::from_bytes_be(&Q_EXP),
            ap: BigUint::from_bytes_be(&AP),
            aq: BigUint::from_bytes_be(&AQ),
        }
    }

    /// Sign a 120-byte cartridge digest.
    ///
    /// # Errors
    /// Returns [`SignError::NoSignableHash`] if no modular square root is found within 256 attempts.
    pub fn sign_hash(
        &self,
        hash: &Digest,
        mut on_try: impl FnMut(u8),
    ) -> Result<Signature, SignError> {
        let mut hash = *hash;
        for _ in 0..256 {
            on_try(hash[4]);

            // Square root modulo each prime (valid because p, q ≡ 3 mod 4),
            // combined into a root modulo n with the Chinese remainder theorem.
            let x = BigUint::from_bytes_be(&hash);
            let root_p = x.modpow(&self.p_exp, &self.p);
            let root_q = x.modpow(&self.q_exp, &self.q);
            let signature = (&self.ap * root_p + &self.aq * root_q) % &self.n;

            // Only accept it if it really decrypts back to the hash.
            if &signature * &signature % &self.n == x {
                return Ok(Signature(to_bytes(&signature)));
            }
            hash[4] = hash[4].wrapping_add(1);
        }
        Err(SignError::NoSignableHash)
    }
}

/// Find a signature for `hash`.
///
/// Only a quarter of all hashes have a square root modulo `n`, so if `hash`
/// has none, byte 4 is incremented and the next value tried, up to 256 times
/// in total. `on_try` is called with the value of byte 4 before each attempt.
///
/// # Errors
///
/// Returns [`SignError::NoSignableHash`] if no attempt succeeds.
pub fn encrypt_hash(hash: &Digest, on_try: impl FnMut(u8)) -> Result<Signature, SignError> {
    SigningKey::new().sign_hash(hash, on_try)
}

/// The 8-byte cartridge hardware trailer stored at `$FFF8..$FFFF`.
///
/// Contains Atari 7800 signature control, encryption start page/BIOS flags,
/// and standard 6502 hardware interrupt and reset vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CartridgeTrailer {
    /// Byte at `$FFF8`: Signature control byte; bits 7..4 and bit 0 must be 1 (`$F1`).
    pub sig_control: u8,
    /// Byte at `$FFF9`: High nibble is the hash start page; low nibble holds BIOS flags (must be 3 or 7).
    pub hash_control: u8,
    /// `$FFFA..$FFFB`: 6502 NMI vector (little-endian).
    pub nmi: u16,
    /// `$FFFC..$FFFD`: 6502 Reset vector (little-endian).
    pub reset: u16,
    /// `$FFFE..$FFFF`: 6502 IRQ/BRK vector (little-endian).
    pub irq: u16,
}

impl CartridgeTrailer {
    /// Parse the 8-byte cartridge trailer from an 8-byte array.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        Self {
            sig_control: bytes[0],
            hash_control: bytes[1],
            nmi: u16::from_le_bytes([bytes[2], bytes[3]]),
            reset: u16::from_le_bytes([bytes[4], bytes[5]]),
            irq: u16::from_le_bytes([bytes[6], bytes[7]]),
        }
    }

    /// First address of the hashed area (high nibble of `$FFF9` as a 6502 memory page).
    #[must_use]
    pub fn hash_start_address(&self) -> usize {
        usize::from(self.hash_control & 0xF0) << 8
    }

    /// Hash start page number (0x00..=0xFF).
    #[must_use]
    pub fn hash_start_page(&self) -> usize {
        usize::from(self.hash_control & 0xF0)
    }

    /// Low nibble of `$FFF9` (BIOS flags).
    #[must_use]
    pub fn bios_flags(&self) -> u8 {
        self.hash_control & 0x0F
    }

    /// Validate the trailer according to Atari 7800 BIOS requirements.
    ///
    /// # Errors
    /// Returns [`SignError`] if signature control byte, hash start page,
    /// BIOS flags, or reset vector violate 7800 BIOS specifications.
    pub fn validate(&self, loaded_len: usize) -> Result<(), SignError> {
        if self.sig_control & 0xF1 != 0xF1 {
            return Err(SignError::InvalidFff8);
        }
        if self.hash_start_address() < MEM_SIZE - loaded_len {
            return Err(SignError::HashAreaTooLarge);
        }
        if self.hash_control & 0x0B != 3 {
            return Err(SignError::InvalidFff9Nibble);
        }
        if usize::from(self.reset) < self.hash_start_address() {
            return Err(SignError::ResetOutsideHash);
        }
        Ok(())
    }
}

/// A cartridge image mapped into the 64 KB address space, aligned so that its
/// last byte is at `$FFFF`. Bytes below the image are zero.
#[derive(Debug, Clone)]
pub struct Cartridge {
    mem: Vec<u8>,
    loaded: usize,
}

impl Cartridge {
    /// Map a cartridge image into memory. The image may carry a 128-byte
    /// `.a78` header; only the last 48 KB of a larger image are kept.
    ///
    /// # Errors
    ///
    /// Returns [`SignError::NotMultipleOf4K`] if the image size isn't a
    /// multiple of 4 KB (plus an optional 128-byte header), or
    /// [`SignError::TooSmall`] if it is smaller than 4 KB.
    pub fn load(image: &[u8]) -> Result<Self, SignError> {
        let size = image.len();
        if size & 0xFFF != 0 && size & 0xFFF != 128 {
            return Err(SignError::NotMultipleOf4K);
        }
        if size < 0x1000 {
            return Err(SignError::TooSmall);
        }

        let loaded = size.min(MAX_HASHED);
        let mut mem = vec![0u8; MEM_SIZE];
        mem[MEM_SIZE - loaded..].copy_from_slice(&image[size - loaded..]);
        Ok(Self { mem, loaded })
    }

    /// Number of image bytes mapped into memory.
    #[must_use]
    pub fn loaded_len(&self) -> usize {
        self.loaded
    }

    /// The 8-byte cartridge hardware trailer and 6502 vectors at `$FFF8..$FFFF`.
    #[must_use]
    pub fn trailer(&self) -> CartridgeTrailer {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.mem[0xFFF8..0x10000]);
        CartridgeTrailer::from_bytes(bytes)
    }

    /// First address of the hashed area (`$FFF9`'s high nibble as a page).
    #[must_use]
    pub fn hash_start(&self) -> usize {
        self.trailer().hash_start_address()
    }

    /// Check the signature-related bytes of the cartridge header
    /// (`$FFF8`, `$FFF9` and the reset vector).
    ///
    /// # Errors
    ///
    /// Returns the first problem found: [`SignError::InvalidFff8`],
    /// [`SignError::HashAreaTooLarge`], [`SignError::InvalidFff9Nibble`] or
    /// [`SignError::ResetOutsideHash`].
    pub fn validate(&self) -> Result<(), SignError> {
        self.trailer().validate(self.loaded)
    }

    /// The signature currently stored in the cartridge.
    #[must_use]
    pub fn signature(&self) -> Signature {
        let mut sig = [0u8; SIGNATURE_LEN];
        sig.copy_from_slice(&self.mem[SIGNATURE_ADDR..SIGNATURE_ADDR + SIGNATURE_LEN]);
        Signature(sig)
    }

    /// Classify the signature currently stored in the cartridge.
    #[must_use]
    pub fn status(&self) -> SignatureStatus {
        let signature = self.signature();
        if decrypt_signature(&signature) == self.hash() {
            SignatureStatus::Valid
        } else if signature.0.iter().all(|&b| b == 0x00 || b == 0xFF) {
            SignatureStatus::Empty
        } else {
            SignatureStatus::Invalid
        }
    }

    /// Generate a signature for this cartridge; see [`encrypt_hash`].
    ///
    /// # Errors
    ///
    /// Returns [`SignError::NoSignableHash`] if no signature could be found.
    pub fn sign(&self, on_try: impl FnMut(u8)) -> Result<Signature, SignError> {
        encrypt_hash(&self.hash(), on_try)
    }

    /// Hash the cartridge (all pages from [`Self::hash_start`] to `$FFFF`,
    /// with the signature area treated as zero).
    #[must_use]
    pub fn hash(&self) -> Digest {
        let start_page = self.trailer().hash_start_page();
        let mut hasher = CartridgeHasher::new(&self.mem[0xFF00..]);

        let s = &PERM[..256];
        let t = &PERM[8..];

        for page in start_page..=0xFE {
            hasher.scramble_page(&self.mem[page << 8..(page << 8) + 256], s);
            hasher.carry = 0;
        }

        hasher.carry = 1;
        hasher.rotate_left();
        hasher.rotate_left();

        for page in (start_page..=0xFE).rev() {
            hasher.scramble_page(&self.mem[page << 8..(page << 8) + 256], t);
            hasher.carry = 1;
        }

        hasher.finish()
    }
}

/// 256-byte hash accumulator for Atari 7800 signature digest generation.
struct CartridgeHasher {
    acc: [u8; 256],
    a: u8,
    carry: u32,
}

impl CartridgeHasher {
    fn new(initial_page: &[u8]) -> Self {
        let mut acc = [0u8; 256];
        acc.copy_from_slice(initial_page);
        acc[0x80..0xF8].fill(0);
        Self {
            acc,
            a: 0,
            carry: 1,
        }
    }

    /// Mix one 256-byte page of the cartridge into the accumulator.
    fn scramble_page(&mut self, page_data: &[u8], perm: &[u8]) {
        for (acc_byte, &cart_byte) in self.acc.iter_mut().zip(page_data) {
            let n = u32::from(self.a) + u32::from(*acc_byte) + self.carry;
            self.carry = (n >> 8) & 1;
            let n = (n & 0xFF) + u32::from(cart_byte) + self.carry;
            self.a = perm[(n & 0xFF) as usize];
            *acc_byte = self.a;
            self.carry = (n >> 8) & 1;
        }
    }

    /// Shift the accumulator left one bit, treating it as a little-endian number
    /// (byte 0 is the least significant).
    fn rotate_left(&mut self) {
        for byte in &mut self.acc {
            let n = (u32::from(*byte) << 1) + self.carry;
            *byte = (n & 0xFF) as u8;
            self.carry = (n >> 8) & 1;
        }
    }

    /// Produce the final 120-byte digest.
    fn finish(self) -> Digest {
        let mut hash = [0u8; SIGNATURE_LEN];
        for (i, h) in hash.iter_mut().enumerate() {
            *h = self.acc[i] ^ self.acc[i + 0x50] ^ self.acc[i + 0x88];
        }
        mask_hash(&mut hash);
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;
    use std::io::Cursor;

    /// Deterministic pseudo-random cartridge with a valid (zeroed) signature
    /// area, `$FF` at `$FFF8`, `hash_byte` at `$FFF9` and reset vector `$F0xx`.
    fn test_rom(size: usize, hash_byte: u8) -> Vec<u8> {
        let mut state = 0x1234_5678u32;
        let mut rom: Vec<u8> = (0..size)
            .map(|_| {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (state >> 16) as u8
            })
            .collect();
        rom[size - 8] = 0xFF;
        rom[size - 7] = hash_byte;
        rom[size - 3] = 0xF0;
        rom[size - 128..size - 8].fill(0);
        rom
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
    }

    /// Sign `rom` in place and return the signature.
    fn sign(rom: &mut [u8]) -> Signature {
        let cart = Cartridge::load(rom).unwrap();
        cart.validate().unwrap();
        let sig = cart.sign(|_| {}).unwrap();
        sig.write_to(&mut Cursor::new(rom)).unwrap();
        sig
    }

    fn status(rom: &[u8]) -> SignatureStatus {
        Cartridge::load(rom).unwrap().status()
    }

    /// Signatures produced by the original `sign7800` (built from the C
    /// source) for the same cartridges.
    #[test]
    fn test_matches_original_sign7800() {
        let golden = [
            (
                0x8000,
                0x87,
                "092666025bb74806d288fc57e7a5777c69ae88d5282bcfd2924052115c95ef1e23fc326a9f127540\
                 3283f72f3d293bfbdcd8c0bcf5436f0e70996bae65fd1580584617fd01834f09bbb0261f67adc01f\
                 a2366d3c4b3b2c84b4ac29eaaf91463e940454cd42e9050d1ae4a4ab4617e6a61133d4deccfd6c56",
            ),
            (
                0x4000,
                0xC7,
                "07b794e835fe5a0606518ebd7cbafa449fe1082171725baffc73fd5ebfed1b3d7163d57b7b4aa475\
                 4829eee36d13f1fe6ec9227c1678ec2acd2e85a36911daa889fc9e9f1b06149f97a35ad9bc04b7b3\
                 2dccb6a451567c70d7fd6afc22ff844eecec9122b8a4197853c326b1735b2cf8b337139e1b0592c9",
            ),
            (
                0xC000,
                0x47,
                "00c4f364dcc421b49190be72e4f13a51799f0655f81b1a933a72f04b5c9265719a0d03aa5f361aea\
                 6459cd408b3351e659d47ffbf579c4ba601929d02b55107d1651e8f4d0d5e5609cd982791041198c\
                 cc0d08eb4ded0b67aecefdc217c08cdfff0b61a9acd747da5189cc42dffdd97da2d904aa7c38bd44",
            ),
        ];
        for (size, hash_byte, expected) in golden {
            let mut rom = test_rom(size, hash_byte);
            assert_eq!(hex(sign(&mut rom).as_bytes()), expected, "size {size:#x}");
        }
    }

    #[test]
    fn test_sign_then_verify_roundtrip() {
        for (size, hash_byte) in [
            (0x1000, 0xF3),
            (0x2000, 0xE3),
            (0x8000, 0x83),
            (0x8000, 0x87),
            (0x2_0000, 0x47),
        ] {
            let mut rom = test_rom(size, hash_byte);
            assert_eq!(status(&rom), SignatureStatus::Empty);

            sign(&mut rom);
            assert_eq!(status(&rom), SignatureStatus::Valid, "size {size:#x}");
        }
    }

    #[test]
    fn test_status_invalid_for_garbage_and_tampering() {
        let mut rom = test_rom(0x8000, 0x87);
        let end = rom.len();

        // Neither all-zero/FF nor a real signature.
        rom[end - 100] = 0x12;
        assert_eq!(status(&rom), SignatureStatus::Invalid);

        rom[end - 128..end - 8].fill(0xFF);
        assert_eq!(status(&rom), SignatureStatus::Empty);

        sign(&mut rom);
        assert_eq!(status(&rom), SignatureStatus::Valid);

        // Change a byte inside the hashed area.
        let mut tampered = rom.clone();
        tampered[0x1234] ^= 0x01;
        assert_eq!(status(&tampered), SignatureStatus::Invalid);

        // Change the signature itself.
        let mut tampered = rom.clone();
        tampered[end - 100] ^= 0x80;
        assert_eq!(status(&tampered), SignatureStatus::Invalid);

        // A byte below the hash start ($A000) is not covered.
        let mut wide = test_rom(0xC000, 0xA7);
        sign(&mut wide);
        wide[0] ^= 0xFF;
        assert_eq!(status(&wide), SignatureStatus::Valid);
    }

    #[test]
    fn test_a78_header_and_large_images_are_end_aligned() {
        let mut rom = test_rom(0x8000, 0x87);
        sign(&mut rom);

        // A 128-byte .a78 header in front does not change the result.
        let mut with_header = vec![0x41; 128];
        with_header.extend_from_slice(&rom);
        let cart = Cartridge::load(&with_header).unwrap();
        assert_eq!(cart.loaded_len(), with_header.len());
        assert_eq!(cart.status(), SignatureStatus::Valid);

        // Only the last 48 KB of a bigger image are used.
        let mut big = test_rom(0x2_0000, 0x87);
        sign(&mut big);
        let cart = Cartridge::load(&big).unwrap();
        assert_eq!(cart.loaded_len(), 0xC000);
        assert_eq!(cart.status(), SignatureStatus::Valid);
    }

    #[test]
    fn test_sign_reports_each_attempt() {
        let rom = test_rom(0x8000, 0x87);
        let cart = Cartridge::load(&rom).unwrap();
        let mut attempts = Vec::new();
        let sig = cart.sign(|b| attempts.push(b)).unwrap();
        assert!(!attempts.is_empty());
        assert_eq!(attempts, (0..attempts.len() as u8).collect::<Vec<_>>());
        assert_eq!(decrypt_signature(&sig), {
            // The nonce byte is masked when decrypting, so this equals the hash.
            cart.hash()
        });
    }

    #[test]
    fn test_write_to_only_touches_signature_area() {
        let rom = test_rom(0x8000, 0x87);
        let sig = Signature::from_bytes([0xAB; SIGNATURE_LEN]);

        let mut out = rom.clone();
        sig.write_to(&mut Cursor::new(&mut out[..])).unwrap();
        let end = rom.len();
        assert_eq!(out[..end - 128], rom[..end - 128]);
        assert_eq!(out[end - 128..end - 8], [0xAB; SIGNATURE_LEN]);
        assert_eq!(out[end - 8..], rom[end - 8..]);

        // Too short to contain a signature area.
        let mut tiny = [0u8; 100];
        assert!(sig.write_to(&mut Cursor::new(&mut tiny[..])).is_err());
    }

    #[test]
    fn test_load_rejects_bad_sizes() {
        for size in [100, 4095, 4097, 0x8001] {
            assert_eq!(
                Cartridge::load(&vec![0; size]).unwrap_err(),
                SignError::NotMultipleOf4K,
                "size {size}"
            );
        }
        // Size is fine modulo 4K (with a header), but smaller than 4 KB.
        assert_eq!(Cartridge::load(&[0; 128]).unwrap_err(), SignError::TooSmall);
        assert_eq!(Cartridge::load(&[]).unwrap_err(), SignError::TooSmall);

        for size in [0x1000, 0x1080, 0x8000, 0x8080] {
            assert!(Cartridge::load(&vec![0; size]).is_ok(), "size {size}");
        }
    }

    #[test]
    fn test_validate_rejects_bad_headers() {
        let check = |rom: &[u8]| Cartridge::load(rom).unwrap().validate();

        assert_eq!(check(&test_rom(0x8000, 0x87)), Ok(()));

        let mut rom = test_rom(0x8000, 0x87);
        rom[0x8000 - 8] = 0x7F;
        assert_eq!(check(&rom), Err(SignError::InvalidFff8));

        // Low nibble must be 3 or 7.
        assert_eq!(
            check(&test_rom(0x8000, 0x85)),
            Err(SignError::InvalidFff9Nibble)
        );

        // Hash area ($4000) larger than a 16 KB image.
        assert_eq!(
            check(&test_rom(0x4000, 0x43)),
            Err(SignError::HashAreaTooLarge)
        );

        let mut rom = test_rom(0x8000, 0x87);
        rom[0x8000 - 3] = 0x7F;
        assert_eq!(check(&rom), Err(SignError::ResetOutsideHash));
    }

    /// Messages are kept identical to the original `sign7800`.
    #[test]
    fn test_error_messages() {
        assert_eq!(
            SignError::NotMultipleOf4K.to_string(),
            "Cartridge size not a multiple of 4K bytes!"
        );
        assert_eq!(
            SignError::TooSmall.to_string(),
            "Cartridge data file must be at least 4K!"
        );
        assert_eq!(
            SignError::InvalidFff8.to_string(),
            "Invalid byte at $FFF8, should be $FF!"
        );
        assert_eq!(
            SignError::HashAreaTooLarge.to_string(),
            "Invalid byte at $FFF9, hash space larger than cartridge image!"
        );
        assert_eq!(
            SignError::InvalidFff9Nibble.to_string(),
            "Invalid byte at $FFF9, low nibble should be 3 or 7!"
        );
        assert_eq!(
            SignError::ResetOutsideHash.to_string(),
            "Cartridge reset vector points outside hashed area!"
        );
    }

    #[test]
    fn test_signature_display_hex_dump() {
        let mut bytes = [0u8; SIGNATURE_LEN];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = i as u8;
        }
        let dump = Signature::from_bytes(bytes).to_string();
        let lines: Vec<&str> = dump.lines().collect();
        assert_eq!(lines[0], "00 01 02 03 04 05 06 07  08 09 0a 0b 0c 0d 0e 0f");
        assert_eq!(lines[7], "70 71 72 73 74 75 76 77");
        assert_eq!(lines.len(), 9);
        assert_eq!(lines[8], "");
        assert!(dump.ends_with("77\n\n"));
    }

    #[test]
    fn test_cartridge_trailer_parsing_and_validation() {
        let raw = [0xF1, 0x87, 0x00, 0x80, 0x50, 0x80, 0x00, 0xFF];
        let trailer = CartridgeTrailer::from_bytes(raw);
        assert_eq!(trailer.sig_control, 0xF1);
        assert_eq!(trailer.hash_control, 0x87);
        assert_eq!(trailer.nmi, 0x8000);
        assert_eq!(trailer.reset, 0x8050);
        assert_eq!(trailer.irq, 0xFF00);
        assert_eq!(trailer.hash_start_address(), 0x8000);
        assert_eq!(trailer.hash_start_page(), 0x80);
        assert_eq!(trailer.bios_flags(), 0x07);
        assert_eq!(trailer.validate(0x8000), Ok(()));
    }
}
