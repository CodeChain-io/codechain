Anyone who holds staking tokens called 'CCS' can share fees that are paid for transactions. It is implemented as a custom action in CodeChain and enabled for the Tendermint consensus engine.

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

    `account` is an [Account Id](./CodeChain-Address.md) which is a blake160 hash of a public key. The list consists of accounts that hold CCS of non-zero balances and accounts of who have delegated a positive sum of CCS to another account. An RLP-encoded list of those accounts is saved in the state. The list should be sorted in ascending order, and every `account` should be unique.

## CCS balance of an account

  * State Key: `makeKey("Account", account)`

    `account` is an `AccountId`

  * Value: `rlp(balance)`

    `balance` is a non-zero `u64` amount of CCS that an account is holding. The amount doesn't include CCS that are delegated to someone.

## Stake delegation of an account

  * State Key: `makeKey("Delegation", delegator)`

    `delegator` is an `AccountID`.

  * Value: `rlp(list of [delegatee, quantity])`

    A `delegatee` is an `AccountId`, and a `quantity` is a non-zero `u64` amount of CCS that a `delegator` delegated to a `delegatee`. The RLP-encoded non-empty list should be sorted by a `delegatee` in ascending order, and every `delegatee` should be unique.

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

    - `receiver` is an `AccountId`.
    - `quantity` is a `u64` amount of CCS to transfer to a `receiver`.

    A `receiver` will be inserted to "the list of CCS holders", and its "CCS balance" will be increased by `quantity`. The transaction sender's balance will be decreased by `quantity`. A sender cannot transfer more than what they have.

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

    - a `delegatee` is an `AccountId`.
    - `quantity` is a `u64` amount of CCS to delegate to `delegatee`.

    A `delegatee` must be one of the current validators. [`delegatee`, `quantity`] will be inserted to the transaction sender's stake delegation list. The balance of the sender will be decreased by `quantity`. However, the balance of the `delegatee` will not be increased. A sender cannot delegate more than what they have.

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

# Fee distribution

You pay fees to make a transaction, which is greater than the specified minimum fee for the transaction type. Transaction fees that are within a block are collected, and CCS holders and the block author share this.

CCS Holders share the sum of the minimum fees for the transaction. They get fees in proportion to the sum of the balance of CCS plus the total amount of CCS that are delegated to someone.

Fees distributed to CCS holders will be rounded down to an integer. The block author gets the rest, which is the sum of the remaining amount due to rounding and the amount that exceeds the minimum fee.

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
  }
}
```

In a such state, an account "0x23..45" has a weight of 500 (balance 450 + delegation 50), and "0xAB..CD" has a weight of 1000 (balance 690 + delegation 200 + delegation 110). If a block has total fees of 110, and if the sum of the minimum fees for it is 35, each account will share the following amount of fees.

```
share of "0x23..45" = Math.floor(35 * ( 500 / (500 + 1000))) = 11
share of "0xAB..CD" = Math.floor(35 * (1000 / (500 + 1000))) = 23
remaing share (of author) = 110 - (11 + 23) = 76
```
