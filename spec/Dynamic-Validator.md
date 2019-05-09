# Dynamic Validator

## Constants
| name                             | value         |
|----------------------------------|---------------|
| **TERM**                         | 1 hour        |
| **NOMINATION_EXPIRATION**        | 24 **TERM**s  |
| **CUSTODY_PERIOD**               | 24 **TERM**s  |
| **RELEASE_PERIOD**               | 240 **TERM**s |
| **WITHDRAW_DELAY**               | 1 **TERM**    |
| **MAX_NUM_OF_VALIDATORS**        | 30            |
| **MIN_NUM_OF_VALIDATORS**        | TBD           |
| **MIN_CCS_RATE_TO_BE_VALIDATOR** | 0.01          |
| **MIN_DEPOSIT**                  | TBD CCC       |


## FSM of Account States
```
                                         +--------+  
                              /--------->| Banned |<---+-------\
                              |          +--------+    |       |
                             (6)                      (6)      |
                              |                        |       |
+----------------+ -(1)--> +-----------+ -(3)--> +-----------+ |
| Normal Account |         | Candidate |         | Validator | |
+----------------+ <--(2)- +-----------+ <--(4)- +-----------+ |
            ^                 ^                        |       |
            |                 |      +--------+ <--(5)-/       |
            |                 \-(7)- | Jailed | -(6)-----------/
            \-------------------(9)- +--------+
                                       ^     |
                                       |     |
                                       \-(8)-/
```
1. Send *SELF_NOMINATE*
2. No *SELF_NOMINATE* while **NOMINATE_EXPIRATION** terms
3. Elected
4. End of term and the validator worked
5. End of term and the validator didn't work
6. Double Vote dected
7. Send *SELF_NOMINATE* after **CUSTODY_PERIOD**
8. Send *SELF_NOMINATE* before **CUSTODY_PERIOD**
9. No *SELF_NOMINATE* during **RELEASE_PERIOD**

## Term
Term is a period that one elected validator set works.
The term is almost one hour.
The block that's generation hour is different from the parent block's is the last block of a term.
CodeChain elects new validator set after all reward of the block is given.

## Nomination
Any account, that are not banned, can nominate itself.
The account becomes a candidate when the sum of the deposit is more than **MIN_DEPOSIT**.
The nomination expires after **NOMINATION_EXPIRATION**; the account who wants to remain a candidate must nominate itself before the previous nomination expires.
The deposit revert to the account when it becomes a normal account.

## Delegation
The stakeholders have the right to choose validators as must as their shares.
It's called a delegation.
And the stakeholders who has been delegated called delegators.
The delegation is valid only when the delegatee is not a normal account or not banned.
The delegated stakes are returned when the account becomes a normal account or banned.

## Election
The process that elects validators of a term is called election.
The election selects validators as the following rule.
<<<<<<<
1. **MAX_NUM_OF_VALIDATORS** accounts in order of having received many delegations.
2. Pick **MIN_NUM_OF_VALIDATORS** accounts; they become validators.
3. In the rest of them, drops the accounts having received less than **MIN_CCS_RATE_TO_BE_VALIDATOR**.
4. The remains become validators.
=======
The initial *MIN_DELEGATION* is **MIN_CCS_RATE_TO_BE_VALIDATOR**.
1. Pick accounts in order of having received many delegations.
2. Drops the accounts having received more than *MIN_DELEGATION*
3. If the number of accounts are more than **MIN_NUM_OF_VALIDATORS** selects top **MAX_NUM_OF_VALIDATORS** accounts.
4. Otherwise, repeat from step 1 after changing *MIN_DELEGATION* to half.
>>>>>>>

## Transactions
### SELF_NOMIATION
* quantity
* metadata(TBD)

### WITHDRAW
* quantity

### DELEGATION
* delegatee
* quantity

### REVOCATION
* delegatee
* quantity

### DOUBLE_VOTE
* header1
* seig1
* header2
* sig2
