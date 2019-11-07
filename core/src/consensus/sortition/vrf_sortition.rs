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

use std::sync::Arc;

use ccrypto::sha256;
use ckey::{Private, Public};
use parking_lot::RwLock;
use primitives::H256;
use vrf::openssl::{Error as VrfError, ECVRF};
use vrf::VRF;

use super::draw::draw;
pub type Priority = H256;

const SECP256K1_TAG_PUBKEY_UNCOMPRESSED: u8 = 0x04;

pub struct VRFSortition {
    pub total_power: u64,
    pub expectation: f64,
    pub vrf_inst: Arc<RwLock<ECVRF>>,
}

#[derive(Eq, PartialEq, Debug, RlpEncodable, RlpDecodable)]
pub struct PriorityInfo {
    priority: Priority,
    sub_user_idx: u64,
    vrf_proof: Vec<u8>,
    vrf_hash: Vec<u8>,
}

impl VRFSortition {
    pub fn create_highest_priority_info(
        &self,
        seed: H256,
        priv_key: Private,
        voting_power: u64,
    ) -> Result<Option<PriorityInfo>, VrfError> {
        let mut vrf_inst = self.vrf_inst.write();
        let vrf_proof = vrf_inst.prove(&*priv_key, &*seed)?;
        let vrf_hash = vrf_inst.proof_to_hash(&vrf_proof)?;
        let j = draw(voting_power, self.total_power, self.expectation, &vrf_hash);

        Ok((0..j)
            .map(|sub_user_idx| {
                let sub_user_idx_vec = sub_user_idx.to_be_bytes();
                let concatenated = [&vrf_hash[..], &sub_user_idx_vec[..]].concat();

                let priority = sha256(&concatenated);
                (priority, sub_user_idx)
            })
            .max()
            .map(|(highest_priority, highest_sub_user_idx)| PriorityInfo {
                priority: highest_priority,
                sub_user_idx: highest_sub_user_idx,
                vrf_proof,
                vrf_hash,
            }))
    }
}

impl PriorityInfo {
    pub fn priority(&self) -> Priority {
        self.priority
    }

    pub fn sub_user_idx(&self) -> u64 {
        self.sub_user_idx
    }

    pub fn verify_vrf_hash(
        &self,
        signer_public: &Public,
        seed: &[u8],
        vrf_inst: Arc<RwLock<ECVRF>>,
    ) -> Result<bool, VrfError> {
        // CodeChain is using uncompressed form of public keys without prefix, so the prefix is required
        // for public keys to be used in openssl.

        let standard_form_pubkey = [&[SECP256K1_TAG_PUBKEY_UNCOMPRESSED], &signer_public[..]].concat();
        let verified_hash = vrf_inst.write().verify(&standard_form_pubkey, &self.vrf_proof, seed)?;
        Ok(verified_hash == self.vrf_hash)
    }

    pub fn verify_sub_user_idx(&self, voting_power: u64, total_power: u64, expectation: f64) -> bool {
        let j = draw(voting_power, total_power, expectation, &self.vrf_hash);
        self.sub_user_idx < j
    }

    pub fn verify_priority(&self) -> bool {
        let sub_user_idx_vec = self.sub_user_idx.to_be_bytes();
        let concatenated = [&self.vrf_hash[..], &sub_user_idx_vec[..]].concat();

        let expected_priority = sha256(&concatenated);
        expected_priority == self.priority
    }

    #[cfg(test)]
    pub fn create_from_members(priority: Priority, sub_user_idx: u64, vrf_proof: Vec<u8>, vrf_hash: Vec<u8>) -> Self {
        Self {
            priority,
            sub_user_idx,
            vrf_proof,
            vrf_hash,
        }
    }
}

#[cfg(test)]
mod vrf_tests {
    extern crate hex;

    use ccrypto::sha256;
    use ckey::KeyPair;
    use rlp::rlp_encode_and_decode_test;
    use vrf::openssl::CipherSuite;

    use super::*;
    #[test]
    fn test_create_highest_priority_info() {
        let priv_key = sha256("secret_key");
        let seed = sha256("seed");
        let ec_vrf = ECVRF::from_suite(CipherSuite::SECP256K1_SHA256_SVDW).unwrap();
        let ec_vrf = Arc::new(RwLock::new(ec_vrf));
        let sortition_scheme = VRFSortition {
            total_power: 100,
            expectation: 50.0,
            vrf_inst: ec_vrf,
        };
        // maximized when sha256(vrf_result || byte expression of 1u64), the testing oracle is generated from python sha256.
        let expected_priority =
            H256::from_slice(&hex::decode("ddc2ca3bd180e1af8fdec721ea863f79ad33279da2148dd58953b44420a0abca").unwrap());
        let expected_sub_user_idx = 1;
        let actual_priority_info =
            sortition_scheme.create_highest_priority_info(seed, priv_key.into(), 10).unwrap().unwrap();
        assert_eq!(expected_priority, actual_priority_info.priority());
        assert_eq!(expected_sub_user_idx, actual_priority_info.sub_user_idx());
    }

    #[test]
    fn test_create_highest_priority_info2() {
        let priv_key = sha256("secret_key");
        let seed = sha256("seed");
        let ec_vrf = ECVRF::from_suite(CipherSuite::SECP256K1_SHA256_SVDW).unwrap();
        let ec_vrf = Arc::new(RwLock::new(ec_vrf));
        let sortition_scheme = VRFSortition {
            total_power: 100,
            expectation: 1.2,
            vrf_inst: ec_vrf,
        };
        let actual_priority_info = sortition_scheme.create_highest_priority_info(seed, priv_key.into(), 10).unwrap();
        assert!(actual_priority_info.is_none());
    }

    #[test]
    fn test_verify_vrf_hash() {
        let priv_key = sha256("secret_key2");
        let pub_key = *KeyPair::from_private(priv_key.into()).expect("Valid private key").public();
        let wrong_priv_key = sha256("wrong_secret_key");
        let wrong_pub_key = *KeyPair::from_private(wrong_priv_key.into()).expect("Valid private key").public();

        // sha256("seed2")
        let seed = sha256("seed2");
        let ec_vrf = ECVRF::from_suite(CipherSuite::SECP256K1_SHA256_SVDW).unwrap();
        let ec_vrf = Arc::new(RwLock::new(ec_vrf));
        let sortition_scheme = VRFSortition {
            total_power: 100,
            expectation: 60.7,
            vrf_inst: ec_vrf,
        };
        let voting_power = 100;
        let priority_info =
            sortition_scheme.create_highest_priority_info(seed, priv_key.into(), voting_power).unwrap().unwrap();
        assert!(priority_info.verify_vrf_hash(&pub_key, &seed, Arc::clone(&sortition_scheme.vrf_inst)).unwrap());
        match priority_info.verify_vrf_hash(&wrong_pub_key, &seed, Arc::clone(&sortition_scheme.vrf_inst)) {
            Err(VrfError::InvalidProof) => (),
            _ => panic!(),
        }
    }

    #[test]
    fn test_verify_sub_user_idx() {
        let priv_key = sha256("secret_key3");
        let seed = sha256("seed3");
        let ec_vrf = ECVRF::from_suite(CipherSuite::SECP256K1_SHA256_SVDW).unwrap();
        let ec_vrf = Arc::new(RwLock::new(ec_vrf));
        let sortition_scheme = VRFSortition {
            total_power: 100,
            expectation: 60.7,
            vrf_inst: ec_vrf,
        };
        let voting_power = 100;
        let priority_info =
            sortition_scheme.create_highest_priority_info(seed, priv_key.into(), voting_power).unwrap().unwrap();
        assert!(priority_info.verify_sub_user_idx(
            voting_power,
            sortition_scheme.total_power,
            sortition_scheme.expectation
        ));
    }


    #[test]
    fn test_priority() {
        let priv_key = sha256("secret_key4");
        let seed = sha256("seed4");
        let ec_vrf = ECVRF::from_suite(CipherSuite::SECP256K1_SHA256_SVDW).unwrap();
        let ec_vrf = Arc::new(RwLock::new(ec_vrf));
        let sortition_scheme = VRFSortition {
            total_power: 100,
            expectation: 41.85,
            vrf_inst: ec_vrf,
        };
        let voting_power = 50;
        let priority_info =
            sortition_scheme.create_highest_priority_info(seed, priv_key.into(), voting_power).unwrap().unwrap();
        assert!(priority_info.verify_priority());
    }

    #[test]
    fn test_encode_and_decode_priority_info() {
        let priority_info = PriorityInfo {
            priority: H256::random(),
            sub_user_idx: 1,
            vrf_hash: vec![0x10, 0x11, 0x30, 0x31],
            vrf_proof: vec![0x41, 0x22, 0x11, 0x12, 0x22, 0x78],
        };
        rlp_encode_and_decode_test!(priority_info);
    }
}
