// Copyright 2018 Kodebox, Inc.
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

#![macro_use]

use ctypes::ShardId;

pub const NETWORK_ID: &str = "tc";
pub const SHARD_ID: ShardId = 0;

macro_rules! asset_mint {
    ($output:expr, $metadata:expr) => {
        $crate::ctypes::transaction::Transaction::AssetMint {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            shard_id: $crate::impls::test_helper::SHARD_ID,
            metadata: $metadata,
            output: $output,
            approver: None,
            administrator: None,
        }
    };
    ($output:expr, $metadata:expr, approver: $approver:expr) => {
        $crate::ctypes::transaction::Transaction::AssetMint {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            shard_id: $crate::impls::test_helper::SHARD_ID,
            metadata: $metadata,
            output: $output,
            approver: Some($approver),
            administrator: None,
        }
    };
    ($output:expr, $metadata:expr, administrator: $admin:expr) => {
        $crate::ctypes::transaction::Transaction::AssetMint {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            shard_id: $crate::impls::test_helper::SHARD_ID,
            metadata: $metadata,
            output: $output,
            approver: None,
            administrator: Some($admin),
        }
    };
}

macro_rules! asset_mint_output {
    ($lock_script_hash:expr, amount: $amount:expr) => {
        asset_mint_output!($lock_script_hash, Default::default(), $amount)
    };
    ($lock_script_hash:expr, parameters: $params:expr) => {
        $crate::ctypes::transaction::AssetMintOutput {
            lock_script_hash: $lock_script_hash,
            parameters: $params,
            amount: None,
        }
    };
    ($lock_script_hash:expr, $params:expr, $amount:expr) => {
        $crate::ctypes::transaction::AssetMintOutput {
            lock_script_hash: $lock_script_hash,
            parameters: $params,
            amount: Some($amount),
        }
    };
}

macro_rules! asset_out_point {
    ($hash:expr, $index:expr, $asset_type:expr, $amount:expr) => {
        $crate::ctypes::transaction::AssetOutPoint {
            transaction_hash: $hash,
            index: $index,
            asset_type: $asset_type,
            amount: $amount,
        }
    };
}

macro_rules! asset_transfer_input {
    ($prev_out:expr) => {
        asset_transfer_input!($prev_out, Vec::new())
    };
    ($prev_out:expr, $lock_script:expr) => {
        $crate::ctypes::transaction::AssetTransferInput {
            prev_out: $prev_out,
            timelock: None,
            lock_script: $lock_script,
            unlock_script: Vec::new(),
        }
    };
}

macro_rules! asset_transfer_inputs {
    [$($x:tt),*] => {
        vec![$(asset_transfer_input! $x),*]
    };
    [$($x:tt,)*] => {
        asset_transfer_inputs![$($x),*]
    };
}

macro_rules! asset_transfer_output {
    ($lock_script_hash:expr, $asset_type:expr, $amount:expr) => {
        $crate::ctypes::transaction::AssetTransferOutput {
            lock_script_hash: $lock_script_hash,
            parameters: Vec::new(),
            asset_type: $asset_type,
            amount: $amount,
        }
    };
    ($lock_script_hash:expr, $parameters:expr, $asset_type:expr, $amount:expr) => {
        $crate::ctypes::transaction::AssetTransferOutput {
            lock_script_hash: $lock_script_hash,
            parameters: $parameters,
            asset_type: $asset_type,
            amount: $amount,
        }
    };
}

macro_rules! asset_transfer_outputs {
    [$($x:tt),*] => {
        vec![$(asset_transfer_output! $x),*]
    };
    [$($x:tt,)*] => {
        asset_transfer_outputs![$($x),*]
    };
}

macro_rules! asset_transfer {
    (inputs: $inputs:expr, $outputs:expr) => {
        $crate::ctypes::transaction::Transaction::AssetTransfer {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            burns: Vec::new(),
            inputs: $inputs,
            outputs: $outputs,
            orders: Vec::new(),
        }
    };
    (inputs: $inputs:expr, $outputs:expr, $orders:expr) => {
        $crate::ctypes::transaction::Transaction::AssetTransfer {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            burns: Vec::new(),
            inputs: $inputs,
            outputs: $outputs,
            orders: $orders,
        }
    };
    (burns: $burns:expr) => {
        $crate::ctypes::transaction::Transaction::AssetTransfer {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            burns: $burns,
            inputs: Vec::new(),
            outputs: Vec::new(),
            orders: Vec::new(),
        }
    };
}

macro_rules! order {
    (from: ($from:expr, $from_amount:expr), to: ($to:expr, $to_amount:expr), fee: ($fee:expr, $fee_amount:expr), [$($output:expr),*], $expiration:expr, $lock_script_hash:expr) => {
        $crate::ctypes::transaction::Order {
            asset_type_from: $from,
            asset_type_to: $to,
            asset_type_fee: $fee,
            asset_amount_from: $from_amount,
            asset_amount_to: $to_amount,
            asset_amount_fee: $fee_amount,
            origin_outputs: vec![$($output,)*],
            expiration: $expiration,
            lock_script_hash: $lock_script_hash,
            parameters: Vec::new(),
        }
    }
}

macro_rules! order_on_transfer {
    ($order:expr, $spent_amount:expr, input_indices: [$($input:expr),*], output_indices: [$($output:expr),*]) => {
        $crate::ctypes::transaction::OrderOnTransfer {
            order: $order,
            spent_amount: $spent_amount,
            input_indices: vec![$($input,)*],
            output_indices: vec![$($output,)*],
        }
    }
}

macro_rules! asset_compose {
    ($metadata:expr, $inputs:expr, $outputs:expr) => {
        $crate::ctypes::transaction::Transaction::AssetCompose {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            shard_id: $crate::impls::test_helper::SHARD_ID,
            metadata: $metadata,
            approver: None,
            administrator: None,
            inputs: $inputs,
            output: $outputs,
        }
    };
}

macro_rules! asset_decompose {
    ($input:expr, $outputs:expr) => {
        $crate::ctypes::transaction::Transaction::AssetDecompose {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            input: $input,
            outputs: $outputs,
        }
    };
}

macro_rules! asset_wrap_ccc_output {
    ($lock_script_hash:expr, $amount:expr) => {
        $crate::ctypes::transaction::AssetWrapCCCOutput {
            lock_script_hash: $lock_script_hash,
            parameters: Vec::new(),
            amount: $amount,
        }
    };
}

macro_rules! asset_wrap_ccc {
    ($parcel_hash:expr, $output:expr) => {
        $crate::ctypes::transaction::InnerTransaction::AssetWrapCCC {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            shard_id: $crate::impls::test_helper::SHARD_ID,
            parcel_hash: $parcel_hash,
            output: $output,
        }
    };
}

macro_rules! asset_unwrap_ccc {
    ($burn:expr) => {
        $crate::ctypes::transaction::Transaction::AssetUnwrapCCC {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            burn: $burn,
        }
    };
}

macro_rules! payment {
    ($receiver:expr, $amount:expr) => {
        $crate::ctypes::parcel::Action::Payment {
            receiver: $receiver,
            amount: $amount,
        }
    };
}

macro_rules! set_regular_key {
    ($key:expr) => {
        $crate::ctypes::parcel::Action::SetRegularKey {
            key: $key,
        }
    };
}

macro_rules! wrap_ccc {
    ($lock_script_hash:expr, $amount:expr) => {
        $crate::ctypes::parcel::Action::WrapCCC {
            shard_id: $crate::impls::test_helper::SHARD_ID,
            lock_script_hash: $lock_script_hash,
            parameters: Vec::new(),
            amount: $amount,
        }
    };
}

macro_rules! set_shard_owners {
    (shard_id: $shard_id:expr, $owners:expr) => {
        $crate::ctypes::parcel::Action::SetShardOwners {
            shard_id: $shard_id,
            owners: $owners,
        }
    };
    ($owners:expr) => {
        $crate::ctypes::parcel::Action::SetShardOwners {
            shard_id: $crate::impls::test_helper::SHARD_ID,
            owners: $owners,
        }
    };
}

macro_rules! set_shard_users {
    ($users:expr) => {
        $crate::ctypes::parcel::Action::SetShardUsers {
            shard_id: $crate::impls::test_helper::SHARD_ID,
            users: $users,
        }
    };
}

macro_rules! parcel {
    (fee: $fee:expr, $action:expr) => {
        parcel!(seq: 0, fee: $fee, $action)
    };
    (seq: $seq:expr, fee: $fee:expr, $action:expr) => {
        $crate::ctypes::parcel::Parcel {
            seq: $seq,
            fee: $fee,
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            action: $action,
        }
    };
}

macro_rules! set_top_level_state {
    ($state: expr, []) => {
    };
    ($state:expr, [(regular_key: $signer:expr => $key:expr) $(,$x:tt)*]) => {
        assert_eq!(Ok(()), $state.set_regular_key(&$signer, &$key));

        set_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(account: $addr:expr => balance: $amount:expr) $(,$x:tt)*]) => {
        assert_eq!(Ok(()), $state.set_balance(&$addr, $amount));

        set_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(shard: $shard_id:expr => owners: [$($owner:expr),*]) $(,$x:tt)*]) => {
        set_top_level_state!($state, [(shard: $shard_id => owners: [$($owner),*], users: Vec::new()) $(,$x)*]);
    };
    ($state:expr, [(shard: $shard_id:expr => owners: [$($owner:expr),*], users: [$($user:expr),*]) $(,$x:tt)*]) => {
        set_top_level_state!($state, [(shard: $shard_id => owners: [$($owner),*], users: vec![$($user),*]) $(,$x)*]);
    };
    ($state:expr, [(shard: $shard_id:expr => owners: [$($owner:expr),*], users: $users:expr) $(,$x:tt)*]) => {
        set_top_level_state!($state, [(shard: $shard_id => owners: vec![$($owner),*], users: $users) $(,$x)*]);
    };
    ($state:expr, [(shard: $shard_id:expr => owners: $owners:expr, users: $users:expr) $(,$x:tt)*]) => {
        assert_eq!(Ok(()), $state.create_shard_level_state($shard_id, $owners, $users));

        set_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(metadata: shards: $number_of_shards:expr) $(,$x:tt)*]) => {
        assert_eq!(Ok(()), $state.set_number_of_shards($number_of_shards));

        set_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($shard:expr, $addr:expr) => { amount: $amount:expr, metadata: $metadata:expr, approver: $approver:expr }) $(,$x:tt)*]) => {
        assert_eq!(Ok((true)), $state.create_asset_scheme($shard, &$addr, $metadata, $amount, $approver, None, Vec::new()));

        set_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(asset: ($shard:expr, $tx_hash:expr, $index:expr) => { asset_type: $asset_type: expr, amount: $amount:expr, lock_script_hash: $lock_script_hash:expr }) $(,$x:tt)*]) => {
        assert_eq!(Ok((true)), $state.create_asset($shard, $tx_hash, $index, $asset_type, $lock_script_hash, Vec::new(), $amount, None));

        set_top_level_state!($state, [$($x),*]);
    };
}

macro_rules! check_top_level_state {
    ($state: expr, []) => { };
    ($state:expr, [(account: $addr:expr => (seq: $seq:expr, balance: $balance:expr)) $(,$x:tt)*]) => {
        assert_eq!(Ok($seq), $state.seq(&$addr));
        assert_eq!(Ok($balance), $state.balance(&$addr));

        check_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(account: $addr:expr => (seq: $seq:expr, balance: $balance:expr, key: $key:expr)) $(,$x:tt)*]) => {
        assert_eq!(Ok(Some($key)), $state.regular_key(&$addr));
        check_top_level_state!($state, [(account: $addr => (seq: $seq, balance: $balance)) $(,$x)*]);
    };
    ($state:expr, [(account: $addr:expr => (seq: $seq:expr, balance: $balance:expr, key)) $(,$x:tt)*]) => {
        assert_eq!(Ok(None), $state.regular_key(&$addr));
        check_top_level_state!($state, [(account: $addr => (seq: $seq, balance: $balance)) $(,$x)*]);
    };
    ($state:expr, [(shard: $shard_id:expr => owners: [$($owner:expr),*]) $(,$x:tt)*]) => {
        check_top_level_state!($state, [(shard: $shard_id => owners: vec![$($owner,)*]) $(,$x)*]);
    };
    ($state:expr, [(shard: $shard_id:expr => owners: $owners:expr) $(,$x:tt)*]) => {
        assert_eq!(Ok(Some($owners)), $state.shard_owners($shard_id));

        check_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(shard: $shard_id:expr => owners: $owners:expr, users: $users:expr) $(,$x:tt)*]) => {
        assert_eq!(Ok(Some($users)), $state.shard_users($shard_id));

        check_top_level_state!($state, [(shard: $shard_id => owners: $owners) $(,$x)*]);
    };
    ($state:expr, [(shard: $shard_id:expr) $(,$x:tt)*]) => {
        assert_eq!(Ok(None), $state.shard_root($shard_id));

        check_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($tx_hash:expr, $shard_id:expr) => { metadata: $metadata:expr, amount: $amount:expr }) $(,$x:tt)*]) => {
        let asset_scheme_address = $crate::AssetSchemeAddress::new($tx_hash, $shard_id);
        let scheme = $state.asset_scheme($shard_id, &asset_scheme_address).unwrap().unwrap();
        assert_eq!(&$metadata, scheme.metadata());
        assert_eq!($amount, scheme.amount());

        check_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($tx_hash:expr, $shard_id:expr) => { metadata: $metadata:expr, amount: $amount:expr, approver: $approver:expr }) $(,$x:tt)*]) => {
        let asset_scheme_address = $crate::AssetSchemeAddress::new($tx_hash, $shard_id);
        let scheme = $state.asset_scheme($shard_id, &asset_scheme_address).unwrap().unwrap();
        assert_eq!(&$metadata, scheme.metadata());
        assert_eq!($amount, scheme.amount());
        assert_eq!(Some(&$approver), scheme.approver().as_ref());

        check_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($tx_hash:expr, $shard_id:expr)) $(,$x:tt)*]) => {
        let asset_scheme_address = $crate::AssetSchemeAddress::new($tx_hash, $shard_id);
        assert_eq!(Ok(None), $state.asset_scheme($shard_id, &asset_scheme_address));

        check_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(asset: ($tx_hash:expr, $index:expr, $shard_id:expr)) $(,$x:tt)*]) => {
        let asset_address = $crate::OwnedAssetAddress::new($tx_hash, $index, $shard_id);
        assert_eq!(Ok(None), $state.asset($shard_id, &asset_address));

        check_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(asset: ($tx_hash:expr, $index:expr, $shard_id:expr) => { asset_type: $asset_type:expr, amount: $amount:expr }) $(,$x:tt)*]) => {
        let asset_address = $crate::OwnedAssetAddress::new($tx_hash, $index, $shard_id);
        let asset = $state.asset($shard_id, &asset_address).unwrap().unwrap();
        assert_eq!(&$asset_type, asset.asset_type());
        assert_eq!($amount, asset.amount());

        check_top_level_state!($state, [$($x),*]);
    };
}

macro_rules! check_shard_level_state {
    ($state: expr, []) => { };
    ($state:expr, [(scheme: ($addr:expr) => { amount: $amount:expr }) $(,$x:tt)*]) => {
        let scheme = $state.asset_scheme(&$addr).unwrap().expect("scheme must exist");
        assert_eq!($amount, scheme.amount());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($tx_hash:expr, $shard_id:expr) => { metadata: $metadata:expr, amount: $amount:expr }) $(,$x:tt)*]) => {
        let asset_scheme_address = $crate::AssetSchemeAddress::new($tx_hash, $shard_id);
        let scheme = $state.asset_scheme(&asset_scheme_address).unwrap().expect("scheme must exist");
        assert_eq!(&$metadata, scheme.metadata());
        assert_eq!($amount, scheme.amount());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($tx_hash:expr, $shard_id:expr) => { metadata: $metadata:expr, amount: $amount:expr, pool: $pool:expr }) $(,$x:tt)*]) => {
        let asset_scheme_address = $crate::AssetSchemeAddress::new($tx_hash, $shard_id);
        let scheme = $state.asset_scheme(&asset_scheme_address).unwrap().expect("scheme must exist");
        assert_eq!(&$metadata, scheme.metadata());
        assert_eq!($amount, scheme.amount());
        assert_eq!($pool, scheme.pool());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($tx_hash:expr, $shard_id:expr) => { metadata: $metadata:expr, amount: $amount:expr, approver: $approver:expr }) $(,$x:tt)*]) => {
        let asset_scheme_address = $crate::AssetSchemeAddress::new($tx_hash, $shard_id);
        let scheme = $state.asset_scheme(&asset_scheme_address).unwrap().expect("scheme must exist");
        assert_eq!(&$metadata, scheme.metadata());
        assert_eq!($amount, scheme.amount());
        assert_eq!(Some(&$approver), scheme.approver().as_ref());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($tx_hash:expr, $shard_id:expr) => { metadata: $metadata:expr, amount: $amount:expr, approver: $approver:expr, administrator }) $(,$x:tt)*]) => {
        let asset_scheme_address = $crate::AssetSchemeAddress::new($tx_hash, $shard_id);
        let scheme = $state.asset_scheme(&asset_scheme_address).unwrap().expect("scheme must exist");
        assert_eq!(&$metadata, scheme.metadata());
        assert_eq!($amount, scheme.amount());
        assert_eq!(Some(&$approver), scheme.approver().as_ref());
        assert_eq!(&None, scheme.administrator());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($tx_hash:expr, $shard_id:expr) => { metadata: $metadata:expr, amount: $amount:expr, approver, administrator: $administrator:expr }) $(,$x:tt)*]) => {
        let asset_scheme_address = $crate::AssetSchemeAddress::new($tx_hash, $shard_id);
        let scheme = $state.asset_scheme(&asset_scheme_address).unwrap().expect("scheme must exist");
        assert_eq!(&$metadata, scheme.metadata());
        assert_eq!($amount, scheme.amount());
        assert_eq!(&None, scheme.approver());
        assert_eq!(Some(&$administrator), scheme.administrator().as_ref());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($tx_hash:expr, $shard_id:expr) => { metadata: $metadata:expr, amount: $amount:expr, administrator: $administrator:expr }) $(,$x:tt)*]) => {
        let asset_scheme_address = $crate::AssetSchemeAddress::new($tx_hash, $shard_id);
        let scheme = $state.asset_scheme(&asset_scheme_address).unwrap().expect("scheme must exist");
        assert_eq!(&$metadata, scheme.metadata());
        assert_eq!($amount, scheme.amount());
        assert_eq!(Some(&$administrator), scheme.administrator().as_ref());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($tx_hash:expr, $shard_id:expr)) $(,$x:tt)*]) => {
        let asset_scheme_address = $crate::AssetSchemeAddress::new($tx_hash, $shard_id);
        assert_eq!(Ok(None), $state.asset_scheme(&asset_scheme_address));

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(asset: ($tx_hash:expr, $index:expr, $shard_id:expr) => { asset_type: $asset_type:expr, amount: $amount:expr }) $(,$x:tt)*]) => {
        let asset_address = $crate::OwnedAssetAddress::new($tx_hash, $index, $shard_id);
        let asset = $state.asset(&asset_address).unwrap().expect("asset must exist");
        assert_eq!(&$asset_type, asset.asset_type());
        assert_eq!($amount, asset.amount());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(asset: ($tx_hash:expr, $index:expr, $shard_id:expr) => { asset_type: $asset_type:expr, amount: $amount:expr, order: $order:expr }) $(,$x:tt)*]) => {
        let asset_address = $crate::OwnedAssetAddress::new($tx_hash, $index, $shard_id);
        let asset = $state.asset(&asset_address).unwrap().expect("asset must exist");
        assert_eq!(&$asset_type, asset.asset_type());
        assert_eq!($amount, asset.amount());
        assert_eq!(Some(&$order), asset.order_hash().as_ref());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(asset: ($tx_hash:expr, $index:expr, $shard_id:expr) => { asset_type: $asset_type:expr, amount: $amount:expr, order }) $(,$x:tt)*]) => {
        let asset_address = $crate::OwnedAssetAddress::new($tx_hash, $index, $shard_id);
        let asset = $state.asset(&asset_address).unwrap().expect("asset must exist");
        assert_eq!(&$asset_type, asset.asset_type());
        assert_eq!($amount, asset.amount());
        assert_eq!(&None, asset.order_hash());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(asset: ($tx_hash:expr, $index:expr, $shard_id:expr) => { asset_type: $asset_type:expr, amount: $amount:expr, lock_script_hash: $lock_script_hash:expr }) $(,$x:tt)*]) => {
        let asset_address = $crate::OwnedAssetAddress::new($tx_hash, $index, $shard_id);
        let asset = $state.asset(&asset_address).unwrap().expect("asset must exist");
        assert_eq!(&$asset_type, asset.asset_type());
        assert_eq!($amount, asset.amount());
        assert_eq!(&$lock_script_hash, asset.lock_script_hash());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(asset: ($tx_hash:expr, $index:expr, $shard_id:expr)) $(,$x:tt)*]) => {
        let asset_address = $crate::OwnedAssetAddress::new($tx_hash, $index, $shard_id);
        assert_eq!(Ok(None), $state.asset(&asset_address));

        check_shard_level_state!($state, [$($x),*]);
    };
}
