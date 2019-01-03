.. _transaction:

#####################
Transaction
#####################

Transactions can do a variety of things that change the state of various aspects within CodeChain. Obvious features
of transactions include trading assets and making payments. However, a less obvious feature involves setting a regular
key to accounts so that transactions can be signed with the regular key instead of the private key. Finally, there is
also a feature that allows users to create shards, where assets are stored and managed.

CodeChain was developed with multi-asset management in mind, coupled with the ability for the service provider to pay transaction
fees for users. Asset transactions are collected at the gateway. These gateways would be the service providers, and can pay the
transaction fees for the transactions going through the respective gateways. If users want to add their transactions directly onto
the blockchain without the need to go through a gateway, then they must pay their own transaction fees.

A transaction would look something like this:
::

    pub struct Transaction {
        pub seq: U256,
        pub fee: U256,
        pub network_id: NetworkId,
        pub action: Action,
    }

    pub enum Action {
        MintAsset {
            network_id: NetworkId,
            shard_id: ShardId,
            metadata: String,
            approver: Option<PlatformAddress>,
            administrator: Option<PlatformAddress>,

            output: AssetMintOutput,

            approvals: Vec<Signature>,
        },
        MintAsset {
            network_id: NetworkId,
            shard_id: ShardId,
            metadata: String,
            approver: Option<PlatformAddress>,
            administrator: Option<PlatformAddress>,

            output: AssetMintOutput,

            approvals: Vec<Signature>,
        },
        // ...
        Pay {
            receiver: Address,
            amount: U256,
        },
        SetRegularKey {
            key: Public,
        },
    }

The fee of the transaction would determine its priority, meaning, how quickly it gets processed. In addition, there is
also a minimum fee that can be set. The seq property exists for the purpose of preventing replay attacks.

Mint Asset Transaction
==============================
When assets are newly minted, there are a couple of things you must understand. First, the asset's scheme must be defined, since the asset being created must have some
sort of definition. Second, there must be an owner to this newly minted asset. Thus, when creating assets in CodeChain, an address of the owner is required. A transaction
that sends freshly minted assets to a user is called the `Asset Mint Transaction <https://codechain.readthedocs.io/en/latest/asset-management.html#minting-creating-new-assets>`_.
The address used for Mint Asset Transactions should follow the `Address Format <https://codechain.readthedocs.io/en/latest/asset-management.html#address-format>`_.

Transfer Asset Transaction
==============================
Once assets have been successfully minted, these assets can now be sent to other users. For instance, let's say that the initial owner of the newly minted assets
is Alice. If Alice wants to send some assets to Bob, then a transaction must be created. This transaction of sending assets from one user to another is called
the Transfer Asset Transaction. By using Alice's signature, assets can be sent to any user, if their `Asset Address <https://codechain.readthedocs.io/en/latest/asset-management.html#asset-transfer-address-format>`_
is known.
