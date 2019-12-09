// Copyright 2019 Kodebox, Inc.
// This file is part of CodeChain.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use ckey::{standard_uncompressed_pubkey, Public};
use primitives::H256;
use vrf::openssl::{Error as VRFError, ECVRF};
use vrf::VRF;

use crate::consensus::{Height, View};

#[derive(Debug, Default, Eq, PartialEq, Clone, Copy, RlpEncodable, RlpDecodable)]
pub struct VRFSeed(H256);

impl AsRef<[u8]> for VRFSeed {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl ::core::ops::Deref for VRFSeed {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl From<H256> for VRFSeed {
    fn from(hash: H256) -> Self {
        VRFSeed(hash)
    }
}

impl From<Vec<u8>> for VRFSeed {
    fn from(hash: Vec<u8>) -> Self {
        H256::from_slice(&hash).into()
    }
}

impl From<VRFSeed> for H256 {
    fn from(seed: VRFSeed) -> Self {
        seed.0
    }
}

#[cfg(test)]
impl From<u64> for VRFSeed {
    fn from(value: u64) -> Self {
        let hash: H256 = value.into();
        hash.into()
    }
}

impl VRFSeed {
    pub fn zero() -> Self {
        VRFSeed(H256::zero())
    }
}

impl VRFSeed {
    /// Calculate the common message used in next seed generation.
    pub fn generate_next_msg(&self, height: Height, view: View) -> Vec<u8> {
        [&self[..], &height.to_be_bytes(), &view.to_be_bytes()].concat()
    }

    pub fn round_msg(&self, view: View) -> Vec<u8> {
        [&self[..], &view.to_be_bytes()].concat()
    }
}

#[derive(Debug, Default, Eq, PartialEq, Clone, RlpEncodable, RlpDecodable)]
pub struct SeedInfo {
    seed_signer_idx: usize,
    seed: VRFSeed,
    proof: Vec<u8>,
}

impl SeedInfo {
    pub fn new(seed_signer_idx: usize, seed: Vec<u8>, proof: Vec<u8>) -> Self {
        Self {
            seed_signer_idx,
            seed: H256::from_slice(&seed).into(),
            proof,
        }
    }

    pub fn signer_idx(&self) -> usize {
        self.seed_signer_idx
    }

    pub fn seed(&self) -> &VRFSeed {
        &self.seed
    }

    pub fn verify(
        &self,
        height: Height,
        view: View,
        prev_seed: &VRFSeed,
        signer_public: &Public,
        vrf_inst: &mut ECVRF,
    ) -> Result<bool, VRFError> {
        let msg = prev_seed.generate_next_msg(height, view);
        let standard_pubkey = standard_uncompressed_pubkey(signer_public);
        vrf_inst.verify(&standard_pubkey, &self.proof, &msg).map(|expected_seed| expected_seed == self.seed.to_vec())
    }
}

#[cfg(test)]
mod seed_tests {
    use ccrypto::sha256;
    use ckey::KeyPair;
    use primitives::H256;
    use rlp::rlp_encode_and_decode_test;
    use vrf::openssl::{CipherSuite, ECVRF};

    use super::super::super::signer::EngineSigner;
    use super::*;

    #[test]
    fn test_seed_verify() {
        let secret = sha256("secret_key2");
        let signer = EngineSigner::create_engine_signer_with_secret(secret);
        let pub_key = *KeyPair::from_private(secret.into()).expect("Valid private key").public();

        let prev_seed: VRFSeed = H256::random().into();
        let mut ec_vrf = ECVRF::from_suite(CipherSuite::SECP256K1_SHA256_SVDW).unwrap();

        let height = 1;
        let view = 1;

        let new_seed_proof = signer.vrf_prove(&prev_seed.generate_next_msg(height, view), &mut ec_vrf).unwrap();
        let new_seed = ec_vrf.proof_to_hash(&new_seed_proof).unwrap();
        let seed_info = SeedInfo::new(0, new_seed, new_seed_proof);
        assert_eq!(seed_info.verify(height, view, &prev_seed, &pub_key, &mut ec_vrf).unwrap(), true);
    }

    #[test]
    fn test_rlp_encode_and_decode() {
        let secret = sha256("secret_key2");
        let signer = EngineSigner::create_engine_signer_with_secret(secret);
        let prev_seed: VRFSeed = H256::random().into();
        let mut ec_vrf = ECVRF::from_suite(CipherSuite::SECP256K1_SHA256_SVDW).unwrap();

        let new_seed_proof = signer.vrf_prove(&prev_seed.generate_next_msg(1, 1), &mut ec_vrf).unwrap();
        let new_seed = ec_vrf.proof_to_hash(&new_seed_proof).unwrap();
        let seed_info = SeedInfo::new(0, new_seed, new_seed_proof);
        rlp_encode_and_decode_test!(seed_info);
    }
}
