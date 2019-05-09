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
| **MIN_NUM_OF_VALIDATORS**        | 4             |
| **MIN_CCS_RATE_TO_BE_VALIDATOR** | 0.01          |
| **MIN_DEPOSIT**                  | TBD CCC       |


## FSM of Account States
```
                                   +--------+
                        /--------->| Banned |<---+-------\
                        |          +--------+    |       |
                       (6)                      (6)      |
                        |                        |       |
+----------+ -(1)--> +-----------+ -(3)--> +-----------+ |
| Eligible |         | Candidate |         | Validator | |
+----------+ <--(2)- +-----------+ <--(4)- +-----------+ |
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
6. Double Vote detected
7. Send *SELF_NOMINATE* after **CUSTODY_PERIOD**
8. Send *SELF_NOMINATE* before **CUSTODY_PERIOD**
9. No *SELF_NOMINATE* during **RELEASE_PERIOD**

## Term
The term is a period when one elected validator set works, and lasts for almost an hour
The block that has a different generation hour from the parent block's is the last block of a term.
CodeChain elects a new validator set after all rewards of the block is given.

## Nomination
Any account that is not banned can nominate itself.
The account becomes a candidate when the sum of the deposit is more than **MIN_DEPOSIT**.
The nomination expires after **NOMINATION_EXPIRATION**; the account that wants to remain a candidate must nominate itself before the previous nomination expires.
The deposit reverts to the account when it becomes an eligible account.

### Minimum Deposit
TBD

## Delegation
The stakeholders have the right to choose validators as much as their shares.
This is called delegation, and the stakeholders who have delegated called delegators.
The delegation is valid only when the delegatee is not an eligible or not banned.
The delegated stakes are returned when the account becomes an eligible account or a banned account.

## Election
The election is a process that elects validators of a term according to the following rule:

1. Pick **MAX_NUM_OF_VALIDATORS** candidates in order of having received many delegations.
2. Select **MIN_NUM_OF_VALIDATORS** accounts; they become validators.
3. Among the rest of them, drop the accounts having received less than MIN_CCS_RATE_TO_BE_VALIDATOR; the remains become validators.

## Voting Power
Each elected validators has different voting power.
The voting power is based on the delegation that the validator received at the election.
The block is valid only if the sum of voting power is more than 2/3 of the total delegations that the elected validators received.

## Validator Reward
The block proposer gets the express fee of the blocks at the end of a term.
Validators can get the reward after **WITHDRAW_DELAY** terms; however, the proposers cannot get all the reward if they are not loyal to their duty.
The reward is decreased according to the rate of the blocks the validator misses to sign.
TBD: The rate of decreasing.

## Punishment for Validators
### Downtime
The validator who doesn't produce blocks is jailed for a while.
The jailed account cannot be a candidate during **CUSTODY_PERIOD**.
*SELF_NOMINATE* transactions of the account are rejected; however, this is not a punishment.
It is to give validators time to fix the nodes that they manage.
The jailed account can nominate itself again after **CUSTODY_PERIOD**.

### Disloyal Validators
CodeChain gives a penalty to validators who doesn't participate in signing the blocks proposed by other nodes.
See [Validator Reward](#Validator-Reward) for more information.

### Double Vote
CodeChain bans the account who double voted.
The deposit and the reward the criminal earns is slashed and is given to the informant reporting the double vote.

## Transactions
### SELF_NOMIATION
* quantity
* metadata(TBD)

This transaction registers the sender to the candidate when the sum of the deposit is larger than **MIN_DEPOSIT**.
The nomination is valid in **NOMINATE_EXPIRATION**.

The account cannot withdraw the deposit manually, and is returned automatically when the account becomes an eligible account.

### WITHDRAW
* quantity

This transaction withdraws the reward that the node earns as a validator.
But the validator cannot withdraw the reward before **WITHDRAW_DELAY** passes.

The transaction that tries to withdraw more than what the account has will fail.

### DELEGATE
* delegatee
* quantity

It's a transaction used by the stakeholders to select the validators.
The stakeholders can delegate as much stakes as they have.
The stakeholders can delegate any candidates, including validators and jailed accounts.
The delegations return automatically when the delegatee becomes eligible or banned.

*DELEGATE* transactions to banned or eligible accounts fail.


### REVOKE
* delegatee
* quantity

It's a transaction used by the stakeholders to revoke the delegation.
The stakeholders can revoke delegations at any time without delay.
The revoke occurs immediately, but the validator cannot be ousted before its term is over.

The transaction fails when the delegator revokes more than it delegates.

### REPORT_DOUBLE_VOTE
* header1
* sig1
* header2
* sig2

This is a transaction that reports malicious validator.
The **REPORT_DOUBLE_VOTE** should be reported during **WITHDRAW_DELAY**.
The transaction that reports a double vote have occurred before **WITHDRAW_DELAY** fails.

The criminal loses all his deposit and rewards and is banned immediately; it is the only case where a validator set is changed during the term.

The informant receives all deposit and rewards(TBD) as prize money immediately.

The criminal becomes a banned account.
The account cannot become a candidate anymore.
In other words, the *DELEGATE* transaction to the banned account and the *SELF_NOMINATE* transaction from the banned account fail.
