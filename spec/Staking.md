Anyone who holds staking tokens called 'CCS' can share fees that are paid for transactions.
It is implemented as a custom action in CodeChain and enabled for the Tendermint consensus engine.

# States

## State key

```
STAKING_CUSTOM_ACTION_ID = 2;

makeKey(...fragments) = blake256(rlp([
    "ActionData",
    STAKING_CUSTOM_ACTION_ID,
    [...fragments]
]))
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

## List of Candidates

  * State Key: `makeKey("Candidates")`
  * Value: `rlp(list of [pubkey, deposit, nominations_ends_at, metadata])`

    The `pubkey` is a public key of a self-nominated candidate.
    The `deposit` is a `u64` amount of CCS deposited by the candidate and `nomination_ends_at` is a `u64` term id when the entry will expire.
    The `metadata` is a `bytes` that can store a short amount of data that expresses or advertises themselves.
    The order of it is constantly changed as the candidates send a self-nominate transaction and the term finishes.
    See the 'Candidate prioritizing' section of [dynamic validator](./Dynamic-Validator.md#Candidate-prioritizing).

## List of jailed accounts

  * State Key: `makeKey("Jailed")`
  * Value: `rlp(list of [account, deposit, custody_until, released_at])`

    The `account` is an `AccountId`, and the `deposit` is a `u64` amount of CCS deposited before the candidate is jailed.
    A jailed candidate can self-nominate and be removed from the list after the term id is greater or equal than a `u64` value of `custody_until`, and it is automatically removed when the term id is greater or equal than a `u64` value of `released_at`.
    The RLP-encoded non-empty list should be sorted by `account` in ascending order, and every `account` should be unique.

## List of banned accounts

  * State Key: `makeKey("Banned")`
  * Value: `rlp(list of account)`

    The `account` is an `AccountId`.

## Current validator set

  * State Key: `makeKey("Validators")`
  * Value: `rlp(list of [weight, delegation, deposit, pubkey])`

    See 'How to update validators' section in [dynamic validator](./Dynamic-Validator.md#How-to-update-validators).

## Intermediate rewards

  * State Key: `makeKey("IntermediateRewards")`
  * Value: `rlp([list of [account,quantity], list of [account,quantity]])`

    The `address` is an `AccountId`, and `quantity` is a `u64` value of CCC. The value is an RLP encoded list of two lists.
    The first list is the rewards of the previous term, and the second list is the rewards of the current term.
    Each list is sorted by `account` in ascending order, and every `account` in a list should be unique.

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

    A `delegatee` must be one of the current candidates.
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

## Revoke

### Action

  * Format: `[ 3, delegatee, quantity ]`

    - A `delegatee` is an `AccountId`.
    - `quantity` is a non-zero `u64` amount of CCS to revoke from `delegatee`.

    This action will revoke delegated CCS from a `delegatee` immediately.
    A `delegator` cannot `Revoke` more than the amount of delegated CCS to a `delegatee`.

### Example

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

> "0xAB..CD" sends staking action [3, "0x01..23", 100]

state = {
  stakeholders: [ "0x23..45", "0xAB..CD" ],
  balance: {
    "0xAB..CD": 790,
  },
  delegation: {
    "0x23..45": [ ["0x01..23", 500] ],
    "0xAB..CD": [ ["0x01..23", 200], ["0x23..45", 10] ]
  },
}

> "0xAB..CD" sends staking action [3, "0x01..23", 50"]

state = {
  stakeholders: [ "0x23..45", "0xAB..CD" ],
  balance: {
    "0xAB..CD": 840,
  },
  delegation: {
    "0x23..45": [ ["0x01..23", 500] ],
    "0xAB..CD": [ ["0x01..23", 150], ["0x23..45", 10] ]
  },
}

> "0x23..45" sends staking action [3, "0x01..23", 500"]

state = {
  stakeholders: [ "0x23..45", "0xAB..CD" ],
  balance: {
    "0xAB..CD": 1340,
  },
  delegation: {
    "0xAB..CD": [ ["0x01..23", 200], ["0x23..45", 110] ]
  }
}
```


## RedelegateCCS

### Action

  * Format: `[6, prev_delegatee, next_delegatee, quantity]`
    - A `prev_delegatee` is an `AccountId`.
    - A `next_delegatee` is an `AccountId`.
    - `quantity` is a `u64` amount of CCS to redelegate to `next_delegatee` from `prev_delegatee`.

   Executing this action is the same as executing the revoke action and the delegate action. The `quantity` should be less than the delegated quantity of `prev_delegatee` from the sender.

### Example

```
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

> "0xAB..CD" sends staking action [ 6, "0x23..45", "0x67..89", 10 ]

state = {
  stakeholders: [ "0x23..45", "0xAB..CD" ],
  balance: {
    "0x23..45": 500,
    "0xAB..CD": 900,
  },
  delegation: {
    "0xAB..CD": [ ["0x23..45", 90], ["0x67..89", 10] ]
  }
}
```

## SelfNominate

### Action

  * Format: `[ 4, deposit, metadata ]`

  See SELF_NOMINATE section in [Dynamic Validator](./Dynamic-Validator.md#SELF_NOMINATE)

## ReportDoubleVote

### Action

  * Format: `[ 5, metadata_seq, params, ...signatrues ]`

  See REPORT_DOUBLE_VOTE section in [Dynamic Validator](./Dynamic-Validator.md#REPORT_DOUBLE_VOTE)

## ChangeParameters
This transaction will change the common parameters when more than half of the stakeholders agree.
It does not change other fields of the scheme file because there are fields related to the genesis block.

It also does not provide a voting feature.
The vote initiator should collect the signatures through the off-chain.

This transaction increases the `seq` of `Metadata` and changes the `params` of `Metadata`.
The changed parameters are applied from the next block that the changing transaction is included in.

The new parameters are used from the next block.

### Action
`[ 0xFF, metadata_seq, new_parameters, ...signatures ]`

#### metadata_seq
The transaction fails if the metadata_seq is different from the `seq` of `Metadata` and is introduced to prevent replay attacks.

#### new_parameters
```
new_parameters := [ (value,)* ]

value := usize | u64 | boolean | string
```
It is the list of the values that the transaction changes.
The stakeholder MUST NOT sign the transaction when the type of value is not a type that the key expected.

#### signatures
`signatures` are the ECDSA signatures of stakeholders.
The stakeholders should send the signature of `blake256(rlp_encode([ 0xFF, metadata_seq, new_parameters ]))` to the vote initiator if they agree to the change.
The transaction is valid only if more than half of the stakeholders agree.

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
