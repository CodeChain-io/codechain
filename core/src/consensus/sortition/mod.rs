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

#![allow(dead_code)]

mod binom_cdf;
mod draw;
pub mod vrf_sortition;

use std::sync::Arc;

use ckey::Public;
use primitives::H256;
use vrf::openssl::Error as VrfError;

use self::vrf_sortition::{PriorityInfo, VRFSortition};

#[derive(Debug, PartialEq, RlpEncodable, RlpDecodable)]
pub struct PriorityMessage {
    pub seed: H256,
    pub info: PriorityInfo,
}

impl PriorityMessage {
    pub fn verify(
        &self,
        signer_public: &Public,
        voting_power: u64,
        sortition_scheme: &VRFSortition,
    ) -> Result<bool, VrfError> {
        // fast verification first
        Ok(self.info.verify_sub_user_idx(voting_power, sortition_scheme.total_power, sortition_scheme.expectation)
            && self.info.verify_priority()
            && self.info.verify_vrf_hash(signer_public, &self.seed, Arc::clone(&sortition_scheme.vrf_inst))?)
    }
}

#[cfg(test)]
mod priority_message_tests {
    use ccrypto::sha256;
    use ckey::{KeyPair, Private};
    use parking_lot::RwLock;
    use rlp::rlp_encode_and_decode_test;
    use vrf::openssl::{CipherSuite, ECVRF};

    use super::*;
    #[test]
    fn check_priority_message_verification() {
        let priv_key: Private = sha256("secret_key").into();
        let pub_key = *KeyPair::from_private(priv_key).expect("Valid private key").public();

        let wrong_priv_key: Private = sha256("wrong_secret_key2").into();
        let wrong_pub_key = *KeyPair::from_private(wrong_priv_key).expect("Valid private key").public();

        let seed = sha256("seed");
        let ec_vrf = ECVRF::from_suite(CipherSuite::SECP256K1_SHA256_SVDW).unwrap();
        let ec_vrf = Arc::new(RwLock::new(ec_vrf));
        let sortition_scheme = VRFSortition {
            total_power: 100,
            expectation: 71.85,
            vrf_inst: ec_vrf,
        };
        let voting_power = 50;
        let priority_info =
            sortition_scheme.create_highest_priority_info(seed, priv_key, voting_power).unwrap().unwrap();

        let priority_message = PriorityMessage {
            seed,
            info: priority_info,
        };
        assert!(priority_message.verify(&pub_key, voting_power, &sortition_scheme).unwrap());
        assert!(priority_message.verify(&wrong_pub_key, voting_power, &sortition_scheme).is_err());
    }

    #[test]
    fn test_encode_and_decode_priority_message() {
        let priv_key: Private = sha256("secret_key").into();

        let seed = sha256("seed");
        let ec_vrf = ECVRF::from_suite(CipherSuite::SECP256K1_SHA256_SVDW).unwrap();
        let ec_vrf = Arc::new(RwLock::new(ec_vrf));
        let sortition_scheme = VRFSortition {
            total_power: 100,
            expectation: 71.85,
            vrf_inst: ec_vrf,
        };
        let voting_power = 50;
        let priority_info =
            sortition_scheme.create_highest_priority_info(seed, priv_key, voting_power).unwrap().unwrap();

        let priority_message = PriorityMessage {
            seed,
            info: priority_info,
        };
        rlp_encode_and_decode_test!(priority_message);
    }
}
