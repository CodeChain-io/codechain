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

    struct Transaction {
        seq: u64,
        fee: u64,
        network_id: NetworkId,
        action: Action,
    }

    enum Action {
        MintAsset { ..., },
        TransferAsset { ..., },
        ChangeAssetScheme { ..., },
        ComposeAsset { ..., },
        DecomposeAsset { ..., },
        Pay { ..., },
        SetRegularKey { ..., },
        CreateShard,
        SetShardOwners { ..., },
        SetShardUsers { ..., },
        WrapCCC { ..., },
        UnwrapCCC { ..., },
        Store { ..., },
        Remove { ..., },
        Custom { ..., },
    }

The fee of the transaction would determine its priority, meaning, how quickly it gets processed. In addition, there is
also a minimum fee that can be set. The seq property exists for the purpose of preventing replay attacks.

The following is a brief explanation for different actions you can use through a transaction:

Mint Asset
==============================
`MintAsset` issues a new asset. When issuing a new asset, the asset has fields that can be designated, such as metadata, approver, and registrar. There are two types of assets that can be issued:

- A permissioned asset is an asset that has an approver. These kind of assets need permission from the specifically assigned approver in order to be transferred to other addresses.
- A regulated asset is an asset that has an registrar. The registrar can change the asset scheme and is allowed to transfer the asset arbitrarily.

Transfer Asset
==============================
`TransferAsset` transfers assets from one address to another. `TransferAsset` can also be used to make orders on the DEX.

Change Asset Scheme
==============================
When minting assets as described above, you create an asset scheme. This scheme defines properties of a specific asset, such as the metadata, and through `ChangeAssetScheme`, the registrar can change an asset's scheme. However, it is important to note that only the registrar has access to `ChangeAssetScheme`.

Compose Asset
==============================
`ComposeAsset` combines multiple assets into a single new package. This new package is called a composed asset, and composed assets can be used as a regular asset. Note that composed assets can be decomposed as well.

Decompose Asset
==============================
`DecomposeAsset` decomposes any composed asset. The original contents that were used as inputs for `ComposeAsset` will be returned as output of `DecomposeAsset`.

Pay
==============================
`Pay` allows a user to make a payment of a certain value of CCC to another user.

Set Regular Key
==============================
Regular keys are responsible for taking the place of the master key. Regular keys provide a safe way to verify one's identity and sign transactions, while keeping the original master key safe in cold storage. Regular keys are safe because they can be easily replaced if they are stolen, while allowing you to maintain the original public address of the master key. `SetRegularKey` defines the regular key that will be used by the payer. If one already exists, this transaction will overwrite the existing one with the new regular key.

Wrap CCC
==============================
WCCC is a wrapped version of CCC, transforming CCC into an asset. `WrapCCC` converts CCC into WCCC.

Unwrap CCC
==============================
`UnwrapCCC` converts WCCC back into CCC.

Store
==============================
`Store` is a special type of transaction that allows the addition of text onto the blockchain. This added text can also be certified by someone through that person's signature.

Remove
==============================
`Remove` removes the content added by the `Store` transaction.

Custom
==============================
`Custom` is a special transaction that may have been added or needed when using a custom consensus engine.
