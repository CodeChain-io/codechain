// Copyright 2019-2020 Kodebox, Inc.
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

use std::cmp::Ordering;
use std::fmt;

use ccrypto::sha256;
use ckey::{standard_uncompressed_pubkey, Public};
use primitives::H256;
use vrf::openssl::{Error as VRFError, ECVRF};
use vrf::VRF;

use super::super::signer::EngineSigner;
use super::draw::draw;
use crate::AccountProviderError;

pub type Priority = H256;

pub struct VRFSortition {
    pub total_power: u64,
    pub expectation: f64,
    pub vrf_inst: ECVRF,
}

#[derive(Eq, PartialEq, Clone, Default, Debug, RlpEncodable, RlpDecodable)]
pub struct PriorityInfo {
    signer_idx: usize,
    priority: Priority,
    sub_user_idx: u64,
    number_of_elections: u64,
    vrf_proof: Vec<u8>,
}

impl fmt::Display for PriorityInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "index-info {}-{}-{}", self.signer_idx(), self.sub_user_idx(), self.number_of_elections())
    }
}

impl Ord for PriorityInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
    }
}

impl PartialOrd for PriorityInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl VRFSortition {
    pub fn create_highest_priority_info(
        &mut self,
        msg: &[u8],
        signer: &EngineSigner,
        signer_idx: usize,
        voting_power: u64,
    ) -> Result<Option<PriorityInfo>, AccountProviderError> {
        let vrf_proof = signer.vrf_prove(msg, &mut self.vrf_inst)?;
        let vrf_hash = self.vrf_inst.proof_to_hash(&vrf_proof)?;
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
                signer_idx,
                priority: highest_priority,
                sub_user_idx: highest_sub_user_idx,
                number_of_elections: j,
                vrf_proof,
            }))
    }
}

impl PriorityInfo {
    pub fn number_of_elections(&self) -> u64 {
        self.number_of_elections
    }

    pub fn priority(&self) -> Priority {
        self.priority
    }

    pub fn signer_idx(&self) -> usize {
        self.signer_idx
    }

    pub fn sub_user_idx(&self) -> u64 {
        self.sub_user_idx
    }

    pub fn verify(
        &self,
        msg: &[u8],
        signer_public: &Public,
        voting_power: u64,
        sortition_scheme: &mut VRFSortition,
    ) -> Result<bool, VRFError> {
        let vrf_hash = self.verify_vrf_hash(signer_public, msg, &mut sortition_scheme.vrf_inst)?;
        if self.verify_sub_user_idx(voting_power, sortition_scheme.total_power, sortition_scheme.expectation, &vrf_hash)
        {
            Ok(self.verify_priority(&vrf_hash) == self.priority)
        } else {
            Err(VRFError::InvalidProof)
        }
    }

    fn verify_vrf_hash(&self, signer_public: &Public, msg: &[u8], vrf_inst: &mut ECVRF) -> Result<Vec<u8>, VRFError> {
        let standard_form_pubkey = standard_uncompressed_pubkey(signer_public);
        vrf_inst.verify(&standard_form_pubkey, &self.vrf_proof, msg)
    }

    fn verify_sub_user_idx(&self, voting_power: u64, total_power: u64, expectation: f64, vrf_hash: &[u8]) -> bool {
        let j = draw(voting_power, total_power, expectation, vrf_hash);
        self.sub_user_idx < j
    }

    fn verify_priority(&self, vrf_hash: &[u8]) -> Priority {
        let sub_user_idx_vec = self.sub_user_idx.to_be_bytes();
        let concatenated = [&vrf_hash[..], &sub_user_idx_vec[..]].concat();

        sha256(&concatenated)
    }

    #[cfg(test)]
    pub fn new(
        signer_idx: usize,
        priority: Priority,
        sub_user_idx: u64,
        number_of_elections: u64,
        vrf_proof: Vec<u8>,
    ) -> Self {
        Self {
            signer_idx,
            priority,
            sub_user_idx,
            number_of_elections,
            vrf_proof,
        }
    }
}

#[cfg(test)]
mod vrf_tests {
    extern crate hex;

    use ccrypto::sha256;
    use rlp::rlp_encode_and_decode_test;
    use vrf::openssl::CipherSuite;

    use super::*;
    #[test]
    fn test_create_highest_priority_info() {
        let signer = EngineSigner::create_engine_signer_with_secret(sha256("secret_key"));
        let seed = sha256("seed");
        let ec_vrf = ECVRF::from_suite(CipherSuite::SECP256K1_SHA256_SVDW).unwrap();
        let mut sortition_scheme = VRFSortition {
            total_power: 100,
            expectation: 50.0,
            vrf_inst: ec_vrf,
        };
        // maximized when sha256(vrf_result || byte expression of 1u64), the testing oracle is generated from python sha256.
        let expected_priority =
            H256::from_slice(&hex::decode("ddc2ca3bd180e1af8fdec721ea863f79ad33279da2148dd58953b44420a0abca").unwrap());
        let voting_power = 10;
        let signer_idx = 1;
        let expected_sub_user_idx = 1;
        let actual_priority_info =
            sortition_scheme.create_highest_priority_info(&seed, &signer, signer_idx, voting_power).unwrap().unwrap();
        assert_eq!(expected_priority, actual_priority_info.priority());
        assert_eq!(expected_sub_user_idx, actual_priority_info.sub_user_idx());
    }

    #[test]
    fn test_create_highest_priority_info2() {
        let signer = EngineSigner::create_engine_signer_with_secret(sha256("secret_key"));
        let seed = sha256("seed");
        let ec_vrf = ECVRF::from_suite(CipherSuite::SECP256K1_SHA256_SVDW).unwrap();
        let mut sortition_scheme = VRFSortition {
            total_power: 100,
            expectation: 1.2,
            vrf_inst: ec_vrf,
        };
        let signer_idx = 1;
        let actual_priority_info =
            sortition_scheme.create_highest_priority_info(&seed, &signer, signer_idx, 10).unwrap();
        assert!(actual_priority_info.is_none());
    }

    #[test]
    fn test_encode_and_decode_priority_info() {
        let priority_info = PriorityInfo {
            signer_idx: 1,
            priority: H256::random(),
            sub_user_idx: 1,
            number_of_elections: 2,
            vrf_proof: vec![0x41, 0x22, 0x11, 0x12, 0x22, 0x78],
        };
        rlp_encode_and_decode_test!(priority_info);
    }
}
