.. _sharding:

#########################
Sharding
#########################
When it comes to blockchain technology that utilizes PoW consensus algorithms, there exists issues related to scalability. In order to scale out, CodeChain provides sharding.

To provide sharding, we divide CodeChain’s state into two. The top-level state contains data necessary to operate CodeChain. For instance, these data could be a shard’s root,
a platform account that holds CCC, or a dynamic validator set. The other state, known as the shard-level state, contains data that are related to the assets, such as the asset
scheme. Thus, sharding allows CodeChain to divide and store all data related to assets. In other words, CodeChain can be viewed as having a single top-level state with multiple
shard-level states branching out.

A node that only has a top-level state without a shard-level state, is called a top-level node. Conversely, a node that has all the shard states is called a full node. If a node
contains a top-level state with certain shard states, it is called a shard node.

However, this does not mean that sharding is always necessary. Sharding is a solution for PoW’s scalability issue. Thus, if Tendermint, or a similar consensus algorithm, is used,
then sharding is not necessary. For these scenarios, you can configure the specific chain that you are using to utilize sharding. However, even if you do not use sharding, it does
not mean that the two state levels will become one. If sharding is not used, then the beacons will behave as the top-level node, and will verify every transaction.

In the case where sharding is used, it is sufficient for the beacon to be the top-level node. In this situation, the verification of AssetTransactionGroup parcel is delegated to the
shard validator, and beacon uses the verified AssetTransactionGroup parcel to generate the block using only the top-level state.

Shard Validator
==========================
When using a shard, the AssetTransactionGroup action must be verified by the Shard Validator. The Shard Validator is randomly selected from the registered shard validator pool. 

TBC
==========================
[TO BE COMPLETED]

RPC
==========================
When using shards, AssetTransactionGroup action can only take place once validator signatures are gathered. The RPCs that exist for this purpose are `shardValidator_registerAction
<https://github.com/CodeChain-io/codechain/blob/master/spec/JSON-RPC.md#shardvalidator_registeraction>`_ and `shardValidator_getSignatures
<https://github.com/CodeChain-io/codechain/blob/master/spec/JSON-RPC.md#shardvalidator_getsignatures>`_. shardValidator_registerAction propagates the surrounding nodes
so that the shard validator can accept and sign an action. The shard validator that receives the action verifies the action and propagates its signature around it.
Through shardValidator_getSignatures, the node can get the signatures it receives.

how-to-configure.rst
==========================
::

    [shard_validator]
    disable = true


    CLI Options for CodeChain client
    ``--shard-validator=[ACCOUNT]``                        Specify the account for shard validator.
    ``--shard-validator-password-path=[PATH]``             Specify the password path of account for shard validator.

