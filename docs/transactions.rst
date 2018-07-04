.. _transactions:

##################
Transactions
##################

Gateway
=================
Gateways are responsible for gathering transactions and grouping them into parcels, which are then added
to the blockchain. Gateways must have platform accounts that contain :ref:`codechain-coin`, since gateways
are responsible for paying the transaction fees.

Parcel
==================
Parcels are a collection of transactions that are added to the blockchain. CodeChain was developed with
multi-asset management in mind, coupled with the ability for the service provider to pay transaction
fees for users. Transactions are collected at the gateway, which group the transactions into parcels.
These gateways would be the service providers, and can pay the transaction fees for the parcels going through
the respective gateways. If users want to add their transactions directly onto the blockchain without the
need to go through a gateway, then they must pay their own transaction fees and create their own parcels.

A parcel would look something like this:
::

    pub struct Parcel {
        pub nonce: U256,
        pub fee: U256,
        pub network_id: u64,
        pub action: Action,
    }

    pub enum Action {
        ChangeShardState {
            transactions: Vec<Transaction>,
        },
        Payment {
            receiver: Address,
            value: U256,
        },
        SetRegularKey {
            key: Public,
        },
    }

The fee of the parcel would determine its priority, meaning, how quickly it gets processed. In addition, there is
also a minimum fee that can be set. The nonce property exists for the purpose of preventing replay attacks.

Validator
==================
(to be completed)