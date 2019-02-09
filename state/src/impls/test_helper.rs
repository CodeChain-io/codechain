// Copyright 2018-2019 Kodebox, Inc.
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

macro_rules! mint_asset {
    ($output:expr, $metadata:expr) => {
        $crate::ctypes::transaction::Action::MintAsset {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            shard_id: $crate::impls::test_helper::SHARD_ID,
            metadata: $metadata,
            output: $output,
            approver: None,
            administrator: None,
            allowed_script_hashes: vec![],
            approvals: vec![],
        }
    };
    ($output:expr, $metadata:expr, approver: $approver:expr) => {
        $crate::ctypes::transaction::Action::MintAsset {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            shard_id: $crate::impls::test_helper::SHARD_ID,
            metadata: $metadata,
            output: $output,
            approver: Some($approver),
            administrator: None,
            allowed_script_hashes: vec![],
            approvals: vec![],
        }
    };
    ($output:expr, $metadata:expr, administrator: $admin:expr) => {
        $crate::ctypes::transaction::Action::MintAsset {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            shard_id: $crate::impls::test_helper::SHARD_ID,
            metadata: $metadata,
            output: $output,
            approver: None,
            administrator: Some($admin),
            allowed_script_hashes: vec![],
            approvals: vec![],
        }
    };
}

macro_rules! asset_mint {
    ($output:expr, $metadata:expr) => {
        $crate::ctypes::transaction::ShardTransaction::MintAsset {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            shard_id: $crate::impls::test_helper::SHARD_ID,
            metadata: $metadata,
            output: $output,
            approver: None,
            administrator: None,
            allowed_script_hashes: vec![],
        }
    };
    ($output:expr, $metadata:expr, approver: $approver:expr) => {
        $crate::ctypes::transaction::ShardTransaction::MintAsset {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            shard_id: $crate::impls::test_helper::SHARD_ID,
            metadata: $metadata,
            output: $output,
            approver: Some($approver),
            administrator: None,
            allowed_script_hashes: vec![],
        }
    };
    ($output:expr, $metadata:expr, administrator: $admin:expr) => {
        $crate::ctypes::transaction::ShardTransaction::MintAsset {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            shard_id: $crate::impls::test_helper::SHARD_ID,
            metadata: $metadata,
            output: $output,
            approver: None,
            administrator: Some($admin),
            allowed_script_hashes: vec![],
        }
    };
    ($output:expr, $metadata:expr, allowed_script_hashes: $allowed:expr) => {
        $crate::ctypes::transaction::ShardTransaction::MintAsset {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            shard_id: $crate::impls::test_helper::SHARD_ID,
            metadata: $metadata,
            output: $output,
            approver: None,
            administrator: None,
            allowed_script_hashes: $allowed,
        }
    };
}

macro_rules! asset_mint_output {
    ($lock_script_hash:expr, supply: $supply:expr) => {
        asset_mint_output!($lock_script_hash, Default::default(), $supply)
    };
    ($lock_script_hash:expr, parameters: $params:expr) => {
        $crate::ctypes::transaction::AssetMintOutput {
            lock_script_hash: $lock_script_hash,
            parameters: $params,
            supply: None,
        }
    };
    ($lock_script_hash:expr, $params:expr, $supply:expr) => {
        $crate::ctypes::transaction::AssetMintOutput {
            lock_script_hash: $lock_script_hash,
            parameters: $params,
            supply: Some($supply),
        }
    };
}

macro_rules! asset_out_point {
    ($tracker:expr, $index:expr, $asset_type:expr, $quantity:expr) => {
        asset_out_point!($tracker, $index, $asset_type, $crate::impls::test_helper::SHARD_ID, $quantity)
    };
    ($tracker:expr, $index:expr, $asset_type:expr, $shard_id:expr, $quantity:expr) => {
        $crate::ctypes::transaction::AssetOutPoint {
            tracker: $tracker,
            index: $index,
            asset_type: $asset_type,
            shard_id: $shard_id,
            quantity: $quantity,
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
    ($lock_script_hash:expr, $asset_type:expr, $quantity:expr) => {
        $crate::ctypes::transaction::AssetTransferOutput {
            lock_script_hash: $lock_script_hash,
            parameters: Vec::new(),
            asset_type: $asset_type,
            shard_id: $crate::impls::test_helper::SHARD_ID,
            quantity: $quantity,
        }
    };
    ($lock_script_hash:expr, $parameters:expr, $asset_type:expr, $quantity:expr) => {
        $crate::ctypes::transaction::AssetTransferOutput {
            lock_script_hash: $lock_script_hash,
            parameters: $parameters,
            asset_type: $asset_type,
            shard_id: $crate::impls::test_helper::SHARD_ID,
            quantity: $quantity,
        }
    };
    ($lock_script_hash:expr, $parameters:expr, $asset_type:expr, $shard_id:expr, $quantity:expr) => {
        $crate::ctypes::transaction::AssetTransferOutput {
            lock_script_hash: $lock_script_hash,
            parameters: $parameters,
            asset_type: $asset_type,
            shard_id: $shard_id,
            quantity: $quantity,
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

macro_rules! transfer_asset {
    (inputs: $inputs:expr, $outputs:expr) => {
        $crate::ctypes::transaction::Action::TransferAsset {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            burns: Vec::new(),
            inputs: $inputs,
            outputs: $outputs,
            orders: Vec::new(),
            metadata: "".into(),
            approvals: vec![],
            expiration: None,
        }
    };
    (inputs: $inputs:expr, $outputs:expr, approvals: $approvals:expr) => {
        $crate::ctypes::transaction::Action::TransferAsset {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            burns: Vec::new(),
            inputs: $inputs,
            outputs: $outputs,
            orders: Vec::new(),
            metadata: "".into(),
            approvals: $approvals,
            expiration: None,
        }
    };
    (inputs: $inputs:expr, $outputs:expr, $orders:expr) => {
        $crate::ctypes::transaction::Action::TransferAsset {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            burns: Vec::new(),
            inputs: $inputs,
            outputs: $outputs,
            orders: $orders,
            metadata: "".into(),
            approvals: vec![],
            expiration: None,
        }
    };
    (burns: $burns:expr) => {
        $crate::ctypes::transaction::Action::TransferAsset {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            burns: $burns,
            inputs: Vec::new(),
            outputs: Vec::new(),
            orders: Vec::new(),
            metadata: "".into(),
            approvals: vec![],
            expiration: None,
        }
    };
}

macro_rules! asset_transfer {
    (inputs: $inputs:expr, $outputs:expr) => {
        $crate::ctypes::transaction::ShardTransaction::TransferAsset {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            burns: Vec::new(),
            inputs: $inputs,
            outputs: $outputs,
            orders: Vec::new(),
        }
    };
    (inputs: $inputs:expr, $outputs:expr, $orders:expr) => {
        $crate::ctypes::transaction::ShardTransaction::TransferAsset {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            burns: Vec::new(),
            inputs: $inputs,
            outputs: $outputs,
            orders: $orders,
        }
    };
    (burns: $burns:expr) => {
        $crate::ctypes::transaction::ShardTransaction::TransferAsset {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            burns: $burns,
            inputs: Vec::new(),
            outputs: Vec::new(),
            orders: Vec::new(),
        }
    };
}

macro_rules! order {
    (from: ($from:expr, $from_quantity:expr), to: ($to:expr, $to_quantity:expr), fee: ($fee:expr, $fee_quantity:expr), [$($output:expr),*], $expiration:expr, $lock_script_hash:expr) => {
        $crate::ctypes::transaction::Order {
            asset_type_from: $from,
            asset_type_to: $to,
            asset_type_fee: $fee,
            shard_id_from: $crate::impls::test_helper::SHARD_ID,
            shard_id_to: $crate::impls::test_helper::SHARD_ID,
            shard_id_fee: $crate::impls::test_helper::SHARD_ID,
            asset_quantity_from: $from_quantity,
            asset_quantity_to: $to_quantity,
            asset_quantity_fee: $fee_quantity,
            origin_outputs: vec![$($output,)*],
            expiration: $expiration,
            lock_script_hash_from: $lock_script_hash,
            parameters_from: Vec::new(),
            lock_script_hash_fee: $lock_script_hash,
            parameters_fee: vec![vec![0x1]],
        }
    }
}

macro_rules! order_on_transfer {
    ($order:expr, $spent_quantity:expr, input_indices: [$($input:expr),*], output_indices: [$($output:expr),*]) => {
        $crate::ctypes::transaction::OrderOnTransfer {
            order: $order,
            spent_quantity: $spent_quantity,
            input_indices: vec![$($input,)*],
            output_indices: vec![$($output,)*],
        }
    }
}

macro_rules! asset_compose {
    ($metadata:expr, $inputs:expr, $outputs:expr) => {
        $crate::ctypes::transaction::ShardTransaction::ComposeAsset {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            shard_id: $crate::impls::test_helper::SHARD_ID,
            metadata: $metadata,
            approver: None,
            administrator: None,
            allowed_script_hashes: vec![],
            inputs: $inputs,
            output: $outputs,
        }
    };
}

macro_rules! asset_decompose {
    ($input:expr, $outputs:expr) => {
        $crate::ctypes::transaction::ShardTransaction::DecomposeAsset {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            input: $input,
            outputs: $outputs,
        }
    };
}

macro_rules! asset_wrap_ccc_output {
    ($lock_script_hash:expr, $quantity:expr) => {
        $crate::ctypes::transaction::AssetWrapCCCOutput {
            lock_script_hash: $lock_script_hash,
            parameters: Vec::new(),
            quantity: $quantity,
        }
    };
}

macro_rules! asset_wrap_ccc {
    ($tx_hash:expr, $output:expr) => {
        $crate::ctypes::transaction::ShardTransaction::WrapCCC {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            shard_id: $crate::impls::test_helper::SHARD_ID,
            tx_hash: $tx_hash,
            output: $output,
        }
    };
}

macro_rules! unwrap_ccc {
    ($burn:expr) => {
        $crate::ctypes::transaction::Action::UnwrapCCC {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            burn: $burn,
        }
    };
}

macro_rules! asset_unwrap_ccc {
    ($burn:expr) => {
        $crate::ctypes::transaction::ShardTransaction::UnwrapCCC {
            network_id: $crate::impls::test_helper::NETWORK_ID.into(),
            burn: $burn,
        }
    };
}

macro_rules! pay {
    ($receiver:expr, $quantity:expr) => {
        $crate::ctypes::transaction::Action::Pay {
            receiver: $receiver,
            quantity: $quantity,
        }
    };
}

macro_rules! set_regular_key {
    ($key:expr) => {
        $crate::ctypes::transaction::Action::SetRegularKey {
            key: $key,
        }
    };
}

macro_rules! wrap_ccc {
    ($lock_script_hash:expr, $quantity:expr) => {
        $crate::ctypes::transaction::Action::WrapCCC {
            shard_id: $crate::impls::test_helper::SHARD_ID,
            lock_script_hash: $lock_script_hash,
            parameters: Vec::new(),
            quantity: $quantity,
        }
    };
}

macro_rules! store {
    ($content:expr, $certifier:expr, $signature:expr) => {
        $crate::ctypes::transaction::Action::Store {
            content: $content,
            certifier: $certifier,
            signature: $signature,
        }
    };
}

macro_rules! remove {
    ($hash:expr, $signature:expr) => {
        $crate::ctypes::transaction::Action::Remove {
            hash: $hash,
            signature: $signature,
        }
    };
}

macro_rules! set_shard_owners {
    (shard_id: $shard_id:expr, $owners:expr) => {
        $crate::ctypes::transaction::Action::SetShardOwners {
            shard_id: $shard_id,
            owners: $owners,
        }
    };
    ($owners:expr) => {
        $crate::ctypes::transaction::Action::SetShardOwners {
            shard_id: $crate::impls::test_helper::SHARD_ID,
            owners: $owners,
        }
    };
}

macro_rules! set_shard_users {
    ($users:expr) => {
        $crate::ctypes::transaction::Action::SetShardUsers {
            shard_id: $crate::impls::test_helper::SHARD_ID,
            users: $users,
        }
    };
}

macro_rules! transaction {
    (fee: $fee:expr, $action:expr) => {
        transaction!(seq: 0, fee: $fee, $action)
    };
    (seq: $seq:expr, fee: $fee:expr, $action:expr) => {
        $crate::ctypes::transaction::Transaction {
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
    ($state:expr, [(account: $addr:expr => balance: $quantity:expr) $(,$x:tt)*]) => {
        assert_eq!(Ok(()), $state.set_balance(&$addr, $quantity));

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
    ($state:expr, [(scheme: ($shard_id:expr, $asset_type:expr) => { supply: $supply:expr, metadata: $metadata:expr, approver: $approver:expr }) $(,$x:tt)*]) => {
        assert_eq!(Ok((true)), $state.create_asset_scheme($shard_id, $asset_type, $metadata, $supply, $approver, None, Vec::new(), Vec::new()));

        set_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(asset: ($shard:expr, $tx_hash:expr, $index:expr) => { asset_type: $asset_type: expr, quantity: $quantity:expr, lock_script_hash: $lock_script_hash:expr }) $(,$x:tt)*]) => {
        assert_eq!(Ok((true)), $state.create_asset($shard, $tx_hash, $index, $asset_type, $lock_script_hash, Vec::new(), $quantity, None));

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
    ($state:expr, [(scheme: ($shard_id:expr, $asset_type:expr) => { metadata: $metadata:expr, supply: $supply:expr }) $(,$x:tt)*]) => {
        let scheme = $state.asset_scheme($shard_id, $asset_type).unwrap().unwrap();
        assert_eq!(&$metadata, scheme.metadata());
        assert_eq!($supply, scheme.supply());

        check_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($shard_id:expr, $asset_type:expr) => { metadata: $metadata:expr, supply: $supply:expr, approver: $approver:expr }) $(,$x:tt)*]) => {
        let scheme = $state.asset_scheme($shard_id, $asset_type).unwrap().unwrap();
        assert_eq!(&$metadata, scheme.metadata());
        assert_eq!($supply, scheme.supply());
        assert_eq!(Some(&$approver), scheme.approver().as_ref());

        check_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($shard_id:expr, $asset_type:expr)) $(,$x:tt)*]) => {
        assert_eq!(Ok(None), $state.asset_scheme($shard_id, $asset_type));

        check_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(asset: ($tx_hash:expr, $index:expr, $shard_id:expr)) $(,$x:tt)*]) => {
        assert_eq!(Ok(None), $state.asset($shard_id, $tx_hash, $index));

        check_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(asset: ($tx_hash:expr, $index:expr, $shard_id:expr) => { asset_type: $asset_type:expr, quantity: $quantity:expr }) $(,$x:tt)*]) => {
        let asset = $state.asset($shard_id, $tx_hash, $index).unwrap().unwrap();
        assert_eq!(&$asset_type, asset.asset_type());
        assert_eq!($quantity, asset.quantity());

        check_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(text: $tx_hash:expr) $(,$x:tt)*]) => {
        assert_eq!(Ok(None), $state.text($tx_hash));

        check_top_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(text: $tx_hash:expr => { content: $content:expr, certifier: $certifier:expr }) $(,$x:tt)*]) => {
        let text = $crate::Text::new($content, $certifier);
        assert_eq!(Ok(Some(text)), $state.text($tx_hash));

        check_top_level_state!($state, [$($x),*]);
    };
}

macro_rules! check_shard_level_state {
    ($state: expr, []) => { };
    ($state:expr, [(scheme: ($shard_id:expr, $asset_type:expr) => { supply: $supply:expr }) $(,$x:tt)*]) => {
        let scheme = $state.asset_scheme($shard_id, $asset_type).unwrap().expect("scheme must exist");
        assert_eq!($supply, scheme.supply());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($shard_id:expr, $asset_type:expr) => { metadata: $metadata:expr, supply: $supply:expr }) $(,$x:tt)*]) => {
        let scheme = $state.asset_scheme($shard_id, $asset_type).unwrap().expect("scheme must exist");
        assert_eq!(&$metadata, scheme.metadata());
        assert_eq!($supply, scheme.supply());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($shard_id:expr, $asset_type:expr) => { metadata: $metadata:expr, supply: $supply:expr, allowed_script_hashes: $allowed:expr}) $(,$x:tt)*]) => {
        let scheme = $state.asset_scheme($shard_id, $asset_type).unwrap().expect("scheme must exist");
        assert_eq!(&$metadata, scheme.metadata());
        assert_eq!($supply, scheme.supply());
        assert_eq!($allowed, scheme.allowed_script_hashes());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($shard_id:expr, $asset_type:expr) => { metadata: $metadata:expr, supply: $supply:expr, pool: $pool:expr }) $(,$x:tt)*]) => {
        let scheme = $state.asset_scheme($shard_id, $asset_type).unwrap().expect("scheme must exist");
        assert_eq!(&$metadata, scheme.metadata());
        assert_eq!($supply, scheme.supply());
        assert_eq!($pool, scheme.pool());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($shard_id:expr, $asset_type:expr) => { metadata: $metadata:expr, supply: $supply:expr, approver: $approver:expr }) $(,$x:tt)*]) => {
        let scheme = $state.asset_scheme($shard_id, $asset_type).unwrap().expect("scheme must exist");
        assert_eq!(&$metadata, scheme.metadata());
        assert_eq!($supply, scheme.supply());
        assert_eq!(Some(&$approver), scheme.approver().as_ref());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($shard_id:expr, $asset_type:expr) => { metadata: $metadata:expr, supply: $supply:expr, approver: $approver:expr, administrator }) $(,$x:tt)*]) => {
        let scheme = $state.asset_scheme($shard_id, $asset_type).unwrap().expect("scheme must exist");
        assert_eq!(&$metadata, scheme.metadata());
        assert_eq!($supply, scheme.supply());
        assert_eq!(Some(&$approver), scheme.approver().as_ref());
        assert_eq!(&None, scheme.administrator());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($shard_id:expr, $asset_type:expr) => { metadata: $metadata:expr, supply: $supply:expr, approver, administrator: $administrator:expr }) $(,$x:tt)*]) => {
        let scheme = $state.asset_scheme($shard_id, $asset_type).unwrap().expect("scheme must exist");
        assert_eq!(&$metadata, scheme.metadata());
        assert_eq!($supply, scheme.supply());
        assert_eq!(&None, scheme.approver());
        assert_eq!(Some(&$administrator), scheme.administrator().as_ref());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($shard_id:expr, $asset_type:expr) => { metadata: $metadata:expr, supply: $supply:expr, administrator: $administrator:expr }) $(,$x:tt)*]) => {
        let scheme = $state.asset_scheme($shard_id, $asset_type).unwrap().expect("scheme must exist");
        assert_eq!(&$metadata, scheme.metadata());
        assert_eq!($supply, scheme.supply());
        assert_eq!(Some(&$administrator), scheme.administrator().as_ref());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(scheme: ($shard_id:expr, $asset_type:expr)) $(,$x:tt)*]) => {
        assert_eq!(Ok(None), $state.asset_scheme($shard_id, $asset_type));

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(asset: ($tx_hash:expr, $index:expr, $shard_id:expr) => { asset_type: $asset_type:expr, quantity: $quantity:expr }) $(,$x:tt)*]) => {
        let asset = $state.asset($shard_id, $tx_hash, $index).unwrap().expect("asset must exist");
        assert_eq!(&$asset_type, asset.asset_type());
        assert_eq!($quantity, asset.quantity());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(asset: ($tx_hash:expr, $index:expr, $shard_id:expr) => { asset_type: $asset_type:expr, quantity: $quantity:expr, order: $order:expr }) $(,$x:tt)*]) => {
        let asset = $state.asset($shard_id, $tx_hash, $index).unwrap().expect("asset must exist");
        assert_eq!(&$asset_type, asset.asset_type());
        assert_eq!($quantity, asset.quantity());
        assert_eq!(Some(&$order), asset.order_hash().as_ref());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(asset: ($tx_hash:expr, $index:expr, $shard_id:expr) => { asset_type: $asset_type:expr, quantity: $quantity:expr, order }) $(,$x:tt)*]) => {
        let asset = $state.asset($shard_id, $tx_hash, $index).unwrap().expect("asset must exist");
        assert_eq!(&$asset_type, asset.asset_type());
        assert_eq!($quantity, asset.quantity());
        assert_eq!(&None, asset.order_hash());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(asset: ($tx_hash:expr, $index:expr, $shard_id:expr) => { asset_type: $asset_type:expr, quantity: $quantity:expr, lock_script_hash: $lock_script_hash:expr }) $(,$x:tt)*]) => {
        let asset = $state.asset($shard_id, $tx_hash, $index).unwrap().expect("asset must exist");
        assert_eq!(&$asset_type, asset.asset_type());
        assert_eq!($quantity, asset.quantity());
        assert_eq!(&$lock_script_hash, asset.lock_script_hash());

        check_shard_level_state!($state, [$($x),*]);
    };
    ($state:expr, [(asset: ($tx_hash:expr, $index:expr, $shard_id:expr)) $(,$x:tt)*]) => {
        assert_eq!(Ok(None), $state.asset($shard_id, $tx_hash, $index));

        check_shard_level_state!($state, [$($x),*]);
    };
}
