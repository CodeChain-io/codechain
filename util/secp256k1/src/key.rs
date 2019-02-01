// Bitcoin secp256k1 bindings
// Written in 2014 by
//   Dawid Ciężarkiewicz
//   Andrew Poelstra
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the CC0 Public Domain Dedication
// along with this software.
// If not, see <http://creativecommons.org/publicdomain/zero/1.0/>.
//

//! # Public and secret keys

use arrayvec::ArrayVec;
use rand::Rng;

use super::Error::{self, IncapableContext, InvalidPublicKey, InvalidSecretKey};
use super::{ContextFlag, Secp256k1};
use constants;
use ffi;

/// Secret 256-bit key used as `x` in an ECDSA signature
#[repr(C)]
pub struct SecretKey([u8; constants::SECRET_KEY_SIZE]);
impl_array_newtype!(SecretKey, u8, constants::SECRET_KEY_SIZE);
impl_pretty_debug!(SecretKey);

impl From<[u8; constants::SECRET_KEY_SIZE]> for SecretKey {
    fn from(raw: [u8; constants::SECRET_KEY_SIZE]) -> Self {
        SecretKey(raw)
    }
}

/// The number 1 encoded as a secret key
/// Deprecated; `static` is not what I want; use `ONE_KEY` instead
pub static ONE: SecretKey =
    SecretKey([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

/// The number 0 encoded as a secret key
pub const ZERO_KEY: SecretKey =
    SecretKey([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

/// The number 1 encoded as a secret key
pub const ONE_KEY: SecretKey =
    SecretKey([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

/// The number 2 encoded as a secret key
pub const TWO_KEY: SecretKey =
    SecretKey([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);

/// The number -1 encoded as a secret key
pub const MINUS_ONE_KEY: SecretKey = SecretKey([
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe, 0xba, 0xae, 0xdc,
    0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x40,
]);

/// A Secp256k1 public key, used for verification of signatures
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug, Hash)]
pub struct PublicKey(ffi::PublicKey);

fn random_32_bytes<R: Rng>(rng: &mut R) -> [u8; 32] {
    let mut ret = [0u8; 32];
    rng.fill_bytes(&mut ret);
    ret
}

impl SecretKey {
    /// Creates a new random secret key
    #[inline]
    pub fn new<R: Rng>(secp: &Secp256k1, rng: &mut R) -> SecretKey {
        let mut data = random_32_bytes(rng);
        unsafe {
            while ffi::secp256k1_ec_seckey_verify(secp.ctx, data.as_ptr()) == 0 {
                data = random_32_bytes(rng);
            }
        }
        SecretKey(data)
    }

    /// Converts a `SECRET_KEY_SIZE`-byte slice to a secret key
    #[inline]
    pub fn from_slice(secp: &Secp256k1, data: &[u8]) -> Result<SecretKey, Error> {
        match data.len() {
            constants::SECRET_KEY_SIZE => {
                let mut ret = [0; constants::SECRET_KEY_SIZE];
                unsafe {
                    if ffi::secp256k1_ec_seckey_verify(secp.ctx, data.as_ptr()) == 0 {
                        return Err(InvalidSecretKey)
                    }
                }
                ret[..].copy_from_slice(data);
                Ok(SecretKey(ret))
            }
            _ => Err(InvalidSecretKey),
        }
    }

    #[inline]
    /// Adds one secret key to another, modulo the curve order
    pub fn add_assign(&mut self, secp: &Secp256k1, other: &SecretKey) -> Result<(), Error> {
        unsafe {
            if ffi::secp256k1_ec_privkey_tweak_add(secp.ctx, self.as_mut_ptr(), other.as_ptr()) != 1 {
                Err(InvalidSecretKey)
            } else {
                Ok(())
            }
        }
    }

    #[inline]
    /// Multiplies one secret key by another, modulo the curve order
    pub fn mul_assign(&mut self, secp: &Secp256k1, other: &SecretKey) -> Result<(), Error> {
        unsafe {
            if ffi::secp256k1_ec_privkey_tweak_mul(secp.ctx, self.as_mut_ptr(), other.as_ptr()) != 1 {
                Err(InvalidSecretKey)
            } else {
                Ok(())
            }
        }
    }

    #[inline]
    /// Inverts (1 / self) this secret key.
    pub fn inv_assign(&mut self, secp: &Secp256k1) -> Result<(), Error> {
        let original = *self;
        unsafe {
            if ffi::secp256k1_ec_privkey_inverse(secp.ctx, self.as_mut_ptr(), original.as_ptr()) != 1 {
                Err(InvalidSecretKey)
            } else {
                Ok(())
            }
        }
    }
}

impl PublicKey {
    /// Creates a new zeroed out public key
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Determines whether a pubkey is valid
    #[inline]
    pub fn is_valid(&self) -> bool {
        // The only invalid pubkey the API should be able to create is
        // the zero one.
        self.0[..].iter().any(|&x| x != 0)
    }

    /// Obtains a raw pointer suitable for use with FFI functions
    #[inline]
    pub fn as_ptr(&self) -> *const ffi::PublicKey {
        &self.0 as *const _
    }

    /// Creates a new public key from a secret key.
    #[inline]
    pub fn from_secret_key(secp: &Secp256k1, sk: &SecretKey) -> Result<PublicKey, Error> {
        if secp.caps == ContextFlag::VerifyOnly || secp.caps == ContextFlag::None {
            return Err(IncapableContext)
        }
        let mut pk = unsafe { ffi::PublicKey::blank() };
        unsafe {
            // We can assume the return value because it's not possible to construct
            // an invalid `SecretKey` without transmute trickery or something
            let res = ffi::secp256k1_ec_pubkey_create(secp.ctx, &mut pk, sk.as_ptr());
            debug_assert_eq!(res, 1);
        }
        Ok(PublicKey(pk))
    }

    /// Creates a public key directly from a slice
    #[inline]
    pub fn from_slice(secp: &Secp256k1, data: &[u8]) -> Result<PublicKey, Error> {
        let mut pk = unsafe { ffi::PublicKey::blank() };
        unsafe {
            if ffi::secp256k1_ec_pubkey_parse(secp.ctx, &mut pk, data.as_ptr(), data.len() as usize) == 1 {
                Ok(PublicKey(pk))
            } else {
                Err(InvalidPublicKey)
            }
        }
    }

    #[inline]
    /// Serialize the key as a byte-encoded pair of values. In compressed form
    /// the y-coordinate is represented by only a single bit, as x determines
    /// it up to one bit.
    pub fn serialize_vec(&self, secp: &Secp256k1, compressed: bool) -> ArrayVec<[u8; constants::PUBLIC_KEY_SIZE]> {
        let mut ret = ArrayVec::new();

        unsafe {
            let mut ret_len = constants::PUBLIC_KEY_SIZE as usize;
            let compressed = if compressed {
                ffi::SECP256K1_SER_COMPRESSED
            } else {
                ffi::SECP256K1_SER_UNCOMPRESSED
            };
            let err =
                ffi::secp256k1_ec_pubkey_serialize(secp.ctx, ret.as_ptr(), &mut ret_len, self.as_ptr(), compressed);
            debug_assert_eq!(err, 1);
            ret.set_len(ret_len as usize);
        }
        ret
    }

    #[inline]
    /// Adds the pk corresponding to `other` to the pk `self` in place
    pub fn add_exp_assign(&mut self, secp: &Secp256k1, other: &SecretKey) -> Result<(), Error> {
        if secp.caps == ContextFlag::SignOnly || secp.caps == ContextFlag::None {
            return Err(IncapableContext)
        }
        unsafe {
            if ffi::secp256k1_ec_pubkey_tweak_add(secp.ctx, &mut self.0 as *mut _, other.as_ptr()) == 1 {
                Ok(())
            } else {
                Err(InvalidSecretKey)
            }
        }
    }

    #[inline]
    /// Adds another point on the curve in place
    pub fn add_assign(&mut self, secp: &Secp256k1, other: &PublicKey) -> Result<(), Error> {
        let mut public = ffi::PublicKey::new();
        let res = unsafe {
            if ffi::secp256k1_ec_pubkey_combine(
                secp.ctx,
                &mut public as *mut _,
                [other.as_ptr(), self.as_ptr()].as_ptr(),
                2,
            ) == 1
            {
                Ok(())
            } else {
                Err(InvalidSecretKey)
            }
        };
        if res.is_ok() {
            self.0 = public
        }
        res
    }

    #[inline]
    /// Multiplies this point by `secret` scalar
    pub fn mul_assign(&mut self, secp: &Secp256k1, other: &SecretKey) -> Result<(), Error> {
        if secp.caps == ContextFlag::SignOnly || secp.caps == ContextFlag::None {
            return Err(IncapableContext)
        }
        unsafe {
            if ffi::secp256k1_ec_pubkey_tweak_mul(secp.ctx, &mut self.0 as *mut _, other.as_ptr()) == 1 {
                Ok(())
            } else {
                Err(InvalidSecretKey)
            }
        }
    }
}

/// Creates a new public key from a FFI public key
impl From<ffi::PublicKey> for PublicKey {
    #[inline]
    fn from(pk: ffi::PublicKey) -> PublicKey {
        PublicKey(pk)
    }
}

#[cfg(test)]
mod test {
    use super::super::constants;
    use super::super::Error::{IncapableContext, InvalidPublicKey, InvalidSecretKey};
    use super::super::{ContextFlag, Secp256k1};
    use super::{PublicKey, SecretKey};

    use rand::{thread_rng, Error, RngCore};

    #[test]
    fn skey_from_slice() {
        let s = Secp256k1::new();
        let sk = SecretKey::from_slice(&s, &[1; 31]);
        assert_eq!(sk, Err(InvalidSecretKey));

        let sk = SecretKey::from_slice(&s, &[1; 32]);
        assert!(sk.is_ok());
    }

    #[test]
    fn pubkey_from_slice() {
        let s = Secp256k1::new();
        assert_eq!(PublicKey::from_slice(&s, &[]), Err(InvalidPublicKey));
        assert_eq!(PublicKey::from_slice(&s, &[1, 2, 3]), Err(InvalidPublicKey));

        let uncompressed = PublicKey::from_slice(
            &s,
            &[
                4, 54, 57, 149, 239, 162, 148, 175, 246, 254, 239, 75, 154, 152, 10, 82, 234, 224, 85, 220, 40, 100,
                57, 121, 30, 162, 94, 156, 135, 67, 74, 49, 179, 57, 236, 53, 162, 124, 149, 144, 168, 77, 74, 30, 72,
                211, 229, 110, 111, 55, 96, 193, 86, 227, 183, 152, 195, 155, 51, 247, 123, 113, 60, 228, 188,
            ],
        );
        assert!(uncompressed.is_ok());

        let compressed = PublicKey::from_slice(
            &s,
            &[
                3, 23, 183, 225, 206, 31, 159, 148, 195, 42, 67, 115, 146, 41, 248, 140, 11, 3, 51, 41, 111, 180, 110,
                143, 114, 134, 88, 73, 198, 174, 52, 184, 78,
            ],
        );
        assert!(compressed.is_ok());
    }

    #[test]
    fn keypair_slice_round_trip() {
        let s = Secp256k1::new();

        let (sk1, pk1) = s.generate_keypair(&mut thread_rng()).unwrap();
        assert_eq!(SecretKey::from_slice(&s, &sk1[..]), Ok(sk1));
        assert_eq!(PublicKey::from_slice(&s, &pk1.serialize_vec(&s, true)[..]), Ok(pk1));
        assert_eq!(PublicKey::from_slice(&s, &pk1.serialize_vec(&s, false)[..]), Ok(pk1));
    }

    #[test]
    fn invalid_secret_key() {
        let s = Secp256k1::new();
        // Zero
        assert_eq!(SecretKey::from_slice(&s, &[0; 32]), Err(InvalidSecretKey));
        // -1
        assert_eq!(SecretKey::from_slice(&s, &[0xff; 32]), Err(InvalidSecretKey));
        // Top of range
        assert!(SecretKey::from_slice(
            &s,
            &[
                0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE, 0xBA,
                0xAE, 0xDC, 0xE6, 0xAF, 0x48, 0xA0, 0x3B, 0xBF, 0xD2, 0x5E, 0x8C, 0xD0, 0x36, 0x41, 0x40,
            ]
        )
        .is_ok());
        // One past top of range
        assert!(SecretKey::from_slice(
            &s,
            &[
                0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE, 0xBA,
                0xAE, 0xDC, 0xE6, 0xAF, 0x48, 0xA0, 0x3B, 0xBF, 0xD2, 0x5E, 0x8C, 0xD0, 0x36, 0x41, 0x41,
            ]
        )
        .is_err());
    }

    #[test]
    fn pubkey_from_slice_bad_context() {
        let s = Secp256k1::without_caps();
        let sk = SecretKey::new(&s, &mut thread_rng());
        assert_eq!(PublicKey::from_secret_key(&s, &sk), Err(IncapableContext));

        let s = Secp256k1::with_caps(ContextFlag::VerifyOnly);
        assert_eq!(PublicKey::from_secret_key(&s, &sk), Err(IncapableContext));

        let s = Secp256k1::with_caps(ContextFlag::SignOnly);
        assert!(PublicKey::from_secret_key(&s, &sk).is_ok());

        let s = Secp256k1::with_caps(ContextFlag::Full);
        assert!(PublicKey::from_secret_key(&s, &sk).is_ok());
    }

    #[test]
    fn add_exp_bad_context() {
        let s = Secp256k1::with_caps(ContextFlag::Full);
        let (sk, mut pk) = s.generate_keypair(&mut thread_rng()).unwrap();

        assert!(pk.add_exp_assign(&s, &sk).is_ok());

        let s = Secp256k1::with_caps(ContextFlag::VerifyOnly);
        assert!(pk.add_exp_assign(&s, &sk).is_ok());

        let s = Secp256k1::with_caps(ContextFlag::SignOnly);
        assert_eq!(pk.add_exp_assign(&s, &sk), Err(IncapableContext));

        let s = Secp256k1::with_caps(ContextFlag::None);
        assert_eq!(pk.add_exp_assign(&s, &sk), Err(IncapableContext));
    }

    #[test]
    fn out_of_range() {
        struct BadRng(u8);
        impl RngCore for BadRng {
            fn next_u32(&mut self) -> u32 {
                unimplemented!()
            }
            fn next_u64(&mut self) -> u64 {
                unimplemented!()
            }

            // This will set a secret key to a little over the
            // group order, then decrement with repeated calls
            // until it returns a valid key
            fn fill_bytes(&mut self, data: &mut [u8]) {
                let group_order: [u8; 32] = [
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe,
                    0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41,
                ];
                assert_eq!(data.len(), 32);
                data.copy_from_slice(&group_order[..]);
                data[31] = self.0;
                self.0 -= 1;
            }

            fn try_fill_bytes(&mut self, data: &mut [u8]) -> Result<(), Error> {
                self.fill_bytes(data);
                Ok(())
            }
        }

        let s = Secp256k1::new();
        s.generate_keypair(&mut BadRng(0xff)).unwrap();
    }

    #[test]
    fn pubkey_from_bad_slice() {
        let s = Secp256k1::new();
        // Bad sizes
        assert_eq!(PublicKey::from_slice(&s, &[0; constants::COMPRESSED_PUBLIC_KEY_SIZE - 1]), Err(InvalidPublicKey));
        assert_eq!(PublicKey::from_slice(&s, &[0; constants::COMPRESSED_PUBLIC_KEY_SIZE + 1]), Err(InvalidPublicKey));
        assert_eq!(PublicKey::from_slice(&s, &[0; constants::UNCOMPRESSED_PUBLIC_KEY_SIZE - 1]), Err(InvalidPublicKey));
        assert_eq!(PublicKey::from_slice(&s, &[0; constants::UNCOMPRESSED_PUBLIC_KEY_SIZE + 1]), Err(InvalidPublicKey));

        // Bad parse
        assert_eq!(PublicKey::from_slice(&s, &[0xff; constants::UNCOMPRESSED_PUBLIC_KEY_SIZE]), Err(InvalidPublicKey));
        assert_eq!(PublicKey::from_slice(&s, &[0x55; constants::COMPRESSED_PUBLIC_KEY_SIZE]), Err(InvalidPublicKey));
    }

    #[test]
    fn debug_output() {
        struct DumbRng(u32);
        impl RngCore for DumbRng {
            fn next_u32(&mut self) -> u32 {
                self.0 = self.0.wrapping_add(1);
                self.0
            }
            fn next_u64(&mut self) -> u64 {
                (u64::from(self.next_u32()) << 32) | u64::from(self.next_u32())
            }
            fn fill_bytes(&mut self, dest: &mut [u8]) {
                // this could, in theory, be done by transmuting dest to a
                // [u64], but this is (1) likely to be undefined behaviour for
                // LLVM, (2) has to be very careful about alignment concerns,
                // (3) adds more `unsafe` that needs to be checked, (4)
                // probably doesn't give much performance gain if
                // optimisations are on.
                let mut count = 0;
                let mut num = 0;
                for byte in dest.iter_mut() {
                    if count == 0 {
                        // we could micro-optimise here by generating a u32 if
                        // we only need a few more bytes to fill the vector
                        // (i.e. at most 4).
                        num = self.next_u64();
                        count = 8;
                    }

                    *byte = (num & 0xff) as u8;
                    num >>= 8;
                    count -= 1;
                }
            }
            fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
                self.fill_bytes(dest);
                Ok(())
            }
        }

        let s = Secp256k1::new();
        let (sk, _) = s.generate_keypair(&mut DumbRng(0)).unwrap();

        assert_eq!(&format!("{:?}", sk), "SecretKey(0200000001000000040000000300000006000000050000000800000007000000)");
    }

    #[test]
    fn pubkey_serialize() {
        struct DumbRng(u32);
        impl RngCore for DumbRng {
            fn next_u32(&mut self) -> u32 {
                self.0 = self.0.wrapping_add(1);
                self.0
            }
            fn next_u64(&mut self) -> u64 {
                (u64::from(self.next_u32()) << 32) | u64::from(self.next_u32())
            }
            fn fill_bytes(&mut self, dest: &mut [u8]) {
                // this could, in theory, be done by transmuting dest to a
                // [u64], but this is (1) likely to be undefined behaviour for
                // LLVM, (2) has to be very careful about alignment concerns,
                // (3) adds more `unsafe` that needs to be checked, (4)
                // probably doesn't give much performance gain if
                // optimisations are on.
                let mut count = 0;
                let mut num = 0;
                for byte in dest.iter_mut() {
                    if count == 0 {
                        // we could micro-optimise here by generating a u32 if
                        // we only need a few more bytes to fill the vector
                        // (i.e. at most 4).
                        num = self.next_u64();
                        count = 8;
                    }

                    *byte = (num & 0xff) as u8;
                    num >>= 8;
                    count -= 1;
                }
            }
            fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
                self.fill_bytes(dest);
                Ok(())
            }
        }

        let s = Secp256k1::new();
        let (_, pk1) = s.generate_keypair(&mut DumbRng(0)).unwrap();
        assert_eq!(
            &pk1.serialize_vec(&s, false)[..],
            &[
                4, 149, 16, 196, 140, 38, 92, 239, 179, 65, 59, 224, 230, 183, 91, 238, 240, 46, 186, 252, 175, 102,
                52, 249, 98, 178, 123, 72, 50, 171, 196, 254, 236, 1, 189, 143, 242, 227, 16, 87, 247, 183, 162, 68,
                237, 140, 92, 205, 151, 129, 166, 58, 111, 96, 123, 64, 180, 147, 51, 12, 209, 89, 236, 213, 206,
            ][..]
        );
        assert_eq!(
            &pk1.serialize_vec(&s, true)[..],
            &[
                2, 149, 16, 196, 140, 38, 92, 239, 179, 65, 59, 224, 230, 183, 91, 238, 240, 46, 186, 252, 175, 102,
                52, 249, 98, 178, 123, 72, 50, 171, 196, 254, 236,
            ][..]
        );
    }

    #[test]
    fn addition() {
        let s = Secp256k1::new();

        let (mut sk1, mut pk1) = s.generate_keypair(&mut thread_rng()).unwrap();
        let (mut sk2, mut pk2) = s.generate_keypair(&mut thread_rng()).unwrap();

        assert_eq!(PublicKey::from_secret_key(&s, &sk1).unwrap(), pk1);
        assert!(sk1.add_assign(&s, &sk2).is_ok());
        assert!(pk1.add_exp_assign(&s, &sk2).is_ok());
        assert_eq!(PublicKey::from_secret_key(&s, &sk1).unwrap(), pk1);

        assert_eq!(PublicKey::from_secret_key(&s, &sk2).unwrap(), pk2);
        assert!(sk2.add_assign(&s, &sk1).is_ok());
        assert!(pk2.add_exp_assign(&s, &sk1).is_ok());
        assert_eq!(PublicKey::from_secret_key(&s, &sk2).unwrap(), pk2);
    }

    #[test]
    fn multiplication() {
        let s = Secp256k1::new();

        let (mut sk1, mut pk1) = s.generate_keypair(&mut thread_rng()).unwrap();
        let (mut sk2, mut pk2) = s.generate_keypair(&mut thread_rng()).unwrap();

        assert_eq!(PublicKey::from_secret_key(&s, &sk1).unwrap(), pk1);
        assert!(sk1.mul_assign(&s, &sk2).is_ok());
        assert!(pk1.mul_assign(&s, &sk2).is_ok());
        assert_eq!(PublicKey::from_secret_key(&s, &sk1).unwrap(), pk1);

        assert_eq!(PublicKey::from_secret_key(&s, &sk2).unwrap(), pk2);
        assert!(sk2.mul_assign(&s, &sk1).is_ok());
        assert!(pk2.mul_assign(&s, &sk1).is_ok());
        assert_eq!(PublicKey::from_secret_key(&s, &sk2).unwrap(), pk2);
    }

    #[test]
    fn pubkey_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::collections::HashSet;
        use std::hash::{Hash, Hasher};

        fn hash<T: Hash>(t: &T) -> u64 {
            let mut s = DefaultHasher::new();
            t.hash(&mut s);
            s.finish()
        }

        let s = Secp256k1::new();
        let mut set = HashSet::new();
        const COUNT: usize = 1024;
        let count = (0..COUNT)
            .map(|_| {
                let (_, pk) = s.generate_keypair(&mut thread_rng()).unwrap();
                let hash = hash(&pk);
                assert!(!set.contains(&hash));
                set.insert(hash);
            })
            .count();
        assert_eq!(count, COUNT);
    }

    #[test]
    fn pubkey_add() {
        let s = Secp256k1::new();
        let (_, mut pk1) = s.generate_keypair(&mut thread_rng()).unwrap();
        let (_, pk2) = s.generate_keypair(&mut thread_rng()).unwrap();

        let result = pk1.add_assign(&s, &pk2);

        assert!(result.is_ok());
    }

    #[test]
    fn pubkey_mul() {
        let s = Secp256k1::new();
        let (_, mut pk1) = s.generate_keypair(&mut thread_rng()).unwrap();
        let (sk2, _) = s.generate_keypair(&mut thread_rng()).unwrap();

        let result = pk1.mul_assign(&s, &sk2);

        assert!(result.is_ok());
    }

    #[test]
    fn skey_mul() {
        let s = Secp256k1::new();
        let (mut sk1, _) = s.generate_keypair(&mut thread_rng()).unwrap();
        let (sk2, _) = s.generate_keypair(&mut thread_rng()).unwrap();

        let result = sk1.mul_assign(&s, &sk2);

        assert!(result.is_ok());
    }

    #[test]
    fn skey_inv() {
        let s = Secp256k1::new();
        let (mut sk, _) = s.generate_keypair(&mut thread_rng()).unwrap();

        let result = sk.inv_assign(&s);

        assert!(result.is_ok());
    }
}
