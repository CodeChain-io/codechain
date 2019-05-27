Anyone who holds staking tokens called 'CCS' can share fees that are paid for transactions.
It is implemented as a custom action in CodeChain and enabled for the Tendermint consensus engine.

# States

## State key

```
STAKING_CUSTOM_ACTION_ID = 2;

makeKey(...fragments) = rlp([
    "ActionData",
    STAKING_CUSTOM_ACTION_ID,
    [...fragments]
])
```

## List of CCS holders

 * State Key : `makeKey("StakeholderAddresses")`

 * Value: `rlp(list of account)`

    An `account` is an [Account Id](./CodeChain-Address.md) which is a blake160 hash of a public key.
    The list consists of accounts that have non-zero balance of CCS, which is the sum of both undelegated and delegated CCS.
    An RLP-encoded list of those accounts is saved in the state.
    The list should be sorted in ascending order, and every `account` should be unique.

## Undelegated CCS of an account

  * State Key: `makeKey("Account", account)`

    An `account` is an `AccountId`

  * Value: `rlp(quantity)`

    `quantity` is a non-zero `u64` amount of undelegated CCS that an account is holding.

## Stake delegations of an account

  * State Key: `makeKey("Delegation", delegator)`

    A `delegator` is an `AccountID`.

  * Value: `rlp(list of [delegatee, quantity])`

    A `delegatee` is an `AccountId`, and the `quantity` is a non-zero `u64` amount of CCS that a `delegator` delegated to a `delegatee`.
    The RLP-encoded non-empty list should be sorted by a `delegatee` in ascending order, and every `delegatee` should be unique.

## Pending Revocations

  * State Key: `makeKey("Revocations")`

  * Value: `rlp(list of [delegator, delegatee, endTime, quantity])`

    A `delegator` is an `AccountId` who has delegated CCS to a `delegatee`, which is also an `AccountId`.
    `endTime` is a Unix time in UTC that specifies when the revocation will finally end. `quantity` is the `u64` amount of CCC that is going to be revoked.
    The RLP-encoded non-empty list should be sorted by `endTime` in ascending order.
    When multiple revocations have the same `endTime`, then the revocation created earlier (a block number is smaller, a transaction index is smaller) must have a smaller index.

# Staking Actions

You can send a RLP-encoded staking action as a payload to [`Action::Custom`](./Transaction.md) by specifying the `handler_id` as a `STAKING_CUSTOM_ACTION_ID`

```
Action::Custom {
  handler_id: STAKING_CUSTOM_ACTION_ID,
  bytes: rlp(action)
}
```

## TransferCCS
### Action

  * Format: `[ 1, receiver, quantity ]`

    - A `receiver` is an `AccountId`.
    - `quantity` is a `u64` amount of CCS to transfer to a `receiver`.

    A `receiver` will be inserted to the list of CCS holders, and its amount of undelegated CCS will be increased by `quantity`.
    The transaction sender's amount of undelegated CCS will be decreased by `quantity`.
    A sender cannot transfer more than the amount of undelegated CCS it has.

### Example
```
state = {
  stakeholders: [ "0xAB..CD" ],
  balance: {
    "0xAB..CD": 1000,
  }
}

> "0xAB..CD" sends staking action [ 1, "0x01..23", 300 ]

state = {
  stakeholders: [ "0x01..23", "0xAB..CD" ],
  balance: {
    "0x01..23": 300,
    "0xAB..CD": 700,
  }
}

> "0xAB..CD" sends staking action [ 1, "0x01..23", 100 ]

state = {
  stakeholders: [ "0x01..23", "0xAB..CD" ],
  balance: {
    "0x01..23": 400,
    "0xAB..CD": 600,
  }
}
```
## DelegateCCS

### Action
  * Format: `[ 2, delegatee, quantity ]`

    - A `delegatee` is an `AccountId`.
    - `quantity` is a `u64` amount of CCS to delegate to `delegatee`.

    A `delegatee` must be one of the current validators.
    The amount of undelegated CCS of the sender will be decreased by `quantity`. However, instead of increasing the amount of undelegated CCS of the `delegatee`, [`delegatee`, `quantity`] will be inserted to the transaction sender's delegation list.
    A sender cannot delegate more than the amount of undelegated CCS it has.

### Example

```
state = {
  stakeholders: [ "0x23..45", "0xAB..CD" ],
  balance: {
    "0x23..45": 500,
    "0xAB..CD": 1000,
  }
}

> "0xAB..CD" sends staking action [ 2, "0x23..45", 100 ]

state = {
  stakeholders: [ "0x23..45", "0xAB..CD" ],
  balance: {
    "0x23..45": 500,
    "0xAB..CD": 900,
  },
  delegation: {
    "0xAB..CD": [ ["0x23..45", 100] ]
  }
}

> "0xAB..CD" sends staking action [ 2, "0x23..45", 10 ]

state = {
  stakeholders: [ "0x23..45", "0xAB..CD" ],
  balance: {
    "0x23..45": 500,
    "0xAB..CD": 890,
  },
  delegation: {
    "0xAB..CD": [ ["0x23..45", 110] ]
  }
}

> "0xAB..CD" sends staking action [ 2, "0x01..23", 200 ]

state = {
  stakeholders: [ "0x23..45", "0xAB..CD" ],
  balance: {
    "0x23..45": 500,
    "0xAB..CD": 690,
  },
  delegation: {
    "0xAB..CD": [ ["0x01..23", 200], ["0x23..45", 110] ]
  }
}

> "0x23..45" sends staking action [ 2, "0x01..23", 500 ]

state = {
  stakeholders: [ "0x23..45", "0xAB..CD" ],
  balance: {
    "0xAB..CD": 690,
  },
  delegation: {
    "0x23..45": [ ["0x01..23", 500] ],
    "0xAB..CD": [ ["0x01..23", 200], ["0x23..45", 110] ]
  }
}

```

## RequestRevoke

### Action

  * Format: `[ 3, delegatee, quantity ]`

    - A `delegatee` is an `AccountId`.
    - `quantity` is a non-zero `u64` amount of CCS to revoke from `delegatee`.

    This action will queue a pending revocation rather than revoke immediately.
    A pending revocation is `[delegator, delegatee, endTime, quantity]`, where `endTime` is set as the `timestamp of a block + REVOKE_PENDING_DURATION`.
    A `delegator` cannot `RequestRevoke` more than the amount of delegated CCS to a `delegatee` minus the sum of the pending revocations between them.

### Example

`REVOKE_PENDING_DURATION` is 500 in this example.

```
state = {
  stakeholders: [ "0x23..45", "0xAB..CD" ],
  balance: {
    "0xAB..CD": 690,
  },
  delegation: {
    "0x23..45": [ ["0x01..23", 500] ],
    "0xAB..CD": [ ["0x01..23", 200], ["0x23..45", 110] ]
  }
}

> "0xAB..CD" sends staking action [3, "0x01..23", 100] at block timestamp 123400

state = {
  stakeholders: [ "0x23..45", "0xAB..CD" ],
  balance: {
    "0xAB..CD": 690,
  },
  delegation: {
    "0x23..45": [ ["0x01..23", 500] ],
    "0xAB..CD": [ ["0x01..23", 200], ["0x23..45", 110] ]
  },
  revocations: [ ["0xAB..CD", "0x01..23", 123900, 100] ]
}

> "0xAB..CD" sends staking action [3, "0x01..23", 50"] at block timestamp 123500

state = {
  stakeholders: [ "0x23..45", "0xAB..CD" ],
  balance: {
    "0xAB..CD": 690,
  },
  delegation: {
    "0x23..45": [ ["0x01..23", 500] ],
    "0xAB..CD": [ ["0x01..23", 200], ["0x23..45", 110] ]
  },
  revocations: [
    ["0xAB..CD", "0x01..23", 123900, 100],
    ["0xAB..CD", "0x01..23", 124000, 50],
  ]
}

> "0x23..45" sends staking action [3, "0x01..23", 500"] at block timestamp 123600

state = {
  stakeholders: [ "0x23..45", "0xAB..CD" ],
  balance: {
    "0xAB..CD": 690,
  },
  delegation: {
    "0x23..45": [ ["0x01..23", 500] ],
    "0xAB..CD": [ ["0x01..23", 200], ["0x23..45", 110] ]
  },
  revocations: [
    ["0xAB..CD", "0x01..23", 123900, 100],
    ["0xAB..CD", "0x01..23", 124000, 50],
    ["0x23..45", "0x01..23", 124100, 500],
  ]
}

> after a block with timestamp 123900

state = {
  stakeholders: [ "0x23..45", "0xAB..CD" ],
  balance: {
    "0xAB..CD": 790,
  },
  delegation: {
    "0x23..45": [ ["0x01..23", 500] ],
    "0xAB..CD": [ ["0x01..23", 100], ["0x23..45", 110] ]
  },
  revocations: [
    ["0xAB..CD", "0x01..23", 124000, 50],
    ["0x23..45", "0x01..23", 124100, 500],
  ]
}

> after a block with timestamp 124000

state = {
  stakeholders: [ "0x23..45", "0xAB..CD" ],
  balance: {
    "0xAB..CD": 840,
  },
  delegation: {
    "0x23..45": [ ["0x01..23", 500] ],
    "0xAB..CD": [ ["0x01..23", 50], ["0x23..45", 110] ]
  },
  revocations: [
    ["0x23..45", "0x01..23", 124100, 500],
  ]
}

> after a block with timestamp 124100

state = {
  stakeholders: [ "0x23..45", "0xAB..CD" ],
  balance: {
    "0x23..45": 500,
    "0xAB..CD": 840,
  },
  delegation: {
    "0xAB..CD": [ ["0x01..23", 50], ["0x23..45", 110] ]
  }
}
```

# Fee distribution

You pay fees to make a transaction. Fees should be greater than the specified minimum fee for the transaction type.
Transaction fees that are within a block are collected, and CCS holders and the block author share this.

CCS Holders share the sum of the minimum fees for the transaction.
They get fees in proportion to the CCS balance they have, which is the sum of the amount of undelegated CCS plus the total amount of CCS that are delegated to someone.

Fees distributed to CCS holders will be rounded down to an integer.
The block author gets the rest, which is the sum of the remaining amount due to rounding and the amount that exceeds the minimum fee.

## Example

```
state = {
  stakeholders: [ "0x23..45", "0xAB..CD" ],
  balance: {
    "0x23..45": 450,
    "0xAB..CD": 690,
  },
  delegation: {
    "0x23..45": [ ["0x01..23", 50] ],
    "0xAB..CD": [ ["0x01..23", 200], ["0x23..45", 110] ]
  },
  revocations: [
    ["0xAB..CD", "0x01..23", 124000, 50],
  }
}
```

In a such state, an account "0x23..45" has a balance of 500 (undelegated 450 + delegated 50), and "0xAB..CD" has a balance of 1000 (undelegated 690 + delegated 200 + delegated 110, pending revocation will not affect it).
If a block has total fees of 110, and the sum of the minimum fees for it is 35, each account will share the following amount of fees:

```
share of "0x23..45" = Math.floor(35 * ( 500 / (500 + 1000))) = 11
share of "0xAB..CD" = Math.floor(35 * (1000 / (500 + 1000))) = 23
remaing share (of author) = 110 - (11 + 23) = 76
```

## ChangeParameters
This transaction will change the common parameters when more than half of the stakeholders agree.
It does not change other fields of the scheme file because there are fields related to the genesis block.

It also does not provide a voting feature.
The vote initiator should collect the signatures through the off-chain.

This transaction increases the `seq` of `Metadata` and changes the `params` of `Metadata`.
The changed parameters are applied from the next block that the changing transaction is included.

The new parameters are used from the next block.

### Action
`[ 0xFF, metadata_seq, new_parameters, ...signatures ]`

#### metadata_seq
The transaction fails if the metadata_seq is different from the `seq` of `Metadata` and is introduced to prevent replay attacks.

#### new_parameters
```
new_parameters := [ new_parameter(, new_parameter)* ]
new_parameter := [ key, value ]

key := usize
value := usize | u64 | boolean | string
```
It is the list of the fields that the transaction changes.
The stakeholder MUST NOT sign the transaction when the type of value is not a type that the key expected.

The parameters that are not in the new_parameters are kept as the previous value.

#### signatures
`signatures` are the ECDSA signatures of stakeholders.
The stakeholders should send the signature of `blake256(rlp_encode([ 0xFF, metadata_seq, new_parameters ]))` to the vote initiator if they agree to the change.
The transaction is valid only if more than half of the stakeholders agree.

# Revocation

`RequestRevoke` will queue a pending revocation instead of revoking a delegation immediately.
The revocation will be delayed by a certain amount of time (`REVOKE_PENDING_DURATION`) to prevent abuse.
It will be processed when a block whose timestamp is greater than or equal to the time when `endTime` is created.
The amount of undelegated CCS of the `delegator` will be increased by `quantity` upon revocation, and the amount of delegated CCS that the `delegator` has delegated to the `delegatee` will be decreased by `quantity`, and the pending revocation will be removed from the revocation queue.
Queues and delegations that become empty should be removed from the state.

## `REVOKE_PENDING_DURATION`

It is 3 weeks in mainnet (1814400 seconds)

## Example

See Example of RequestRevoke in Staking Actions
