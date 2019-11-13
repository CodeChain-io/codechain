# Tendermint Consensus

The Tendermint consensus algorithm is one of the consensus options offered in CodeChain. Tendermint is explained in this [link](https://tendermint.com/docs/spec/consensus/consensus.html). This document describes how Tendermint is implemented in CodeChain.

## Terms

* **height**: A unique number assigned to each block. The Tendermint consensus algorithm agrees one block per height. Block number _h_ is agreed at height _h_.
* **view**: Same as ‘Round’ in the Tendermint algorithm. For every new height, the view value is initialized to zero. A new block is agreed on every view, and if the agreement is successful, it goes on to the next height. If the agreement fails, it goes on to the next view.
* **seal**: Information needed for consensus, which is stored in the header.

## State

The Tendermint consensus algorithm runs as a state machine. Tendermint states consist of Propose, Prevote, Precommit, and Commit. In the code, `Step` and `TendermintState` types are used to represent these states. The `Step` type is for informing the state to the outside, and `TendermintState` is to discern various internal operations, and thus, it is divided more intricately compared to `Step`.


```rust
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Step {
   Propose,
   Prevote,
   Precommit,
   Commit,
}

#[derive(Clone)]
pub enum TendermintState {
   Propose,
   ProposeWaitBlockGeneration {
       parent_hash: H256,
   },
   ProposeWaitImported {
       block: Box<SealedBlock>,
   },
   ProposeWaitEmptyBlockTimer {
       block: Box<SealedBlock>,
   },
   Prevote,
   Precommit,
   Commit {
       view: View,
       block_hash: H256,
   },
   CommitTimedout {
       view: View,
       block_hash: H256,
   },
}
```

### Propose

The Propose step is a step for generating a proposal block by itself or receiving a block from another node. If a node is a proposer, they would ask client* to create a block and go into the `ProposeWaitBlockGeneration` state. If an empty block is created, the timer is set to half of the Propose timeout and goes into the `ProposeWaitEmptyBlockTimer` state to prevent too many unnecessary empty blocks from being created. Once the timer is activated, the state goes on to the Prevote state. When a non-empty block is created, the state immediately goes to Prevote.

If a node is not a proposer, they should wait for a proposal created by another node. Once the proposal block is received, the author of the received proposal is examined. Then the client is asked to verify and save, and the status becomes `ProposeWaitImported`. Once imported (after verification and the block has been saved to the disk), it goes on to the Prevote step. If a block is not received before the Propose timeout, it goes to the Prevote state.

* Client: An interface that provides functionalities outside the consensus. It has functions such as block creation, received block verification and storage, and reading the previous block’s data.


#### Timeout

If the node does not receive a proposal during the timeout, the state goes on to the Prevote state.

### Prevote

If a proposal was imported before the Prevote step, it creates and propagates a Prevote message for that proposal. If not, a message for Nil is created and broadcasted throughout the network. If more than 2/3 of Prevotes voted for the same block or Nil, the node will lock and move on to Precommit.

#### Role of Timeouts

After the Prevote step begins, if more than 2/3 of the Prevotes have not been collected during the Prevote timeout period, the node notifies its status to other nodes and resets the Prevote timeout. If more than 2/3 of Prevotes have been collected, the state move on to Precommit.

### Precommit

If more than 2/3 of Prevotes for the current view’s block are collected before the Precommit step, a Precommit message for that block is generated and distributed. If not, a Precommit for Nil is created and broadcasted. When more than 2/3 of Precommits are collected for a block, the state goes on to the Commit state. If more than 2/3 of Precommits are collected for Nil, it goes on to the next view’s Proposal state.

#### Role of Timeouts

If more than 2/3 of the Precommits are collected after timeout, the state goes on to the Proposal of the next view. If not, the other nodes are requested for their Precommit votes and the timeout is reset.

### Commit

Once at the Commit step, 2/3 or more Precommits should have gathered for the block. At the Commit step, the client is asked to replace the best block with the block that has the most Precommits. Once the Proposal block is imported, all Precommits are collected, the best block is replaced, and the state moves to the next height.

#### Role of Timeouts

If the Proposal is imported, and the best block changed within the timeout period, the state moves on to the next height. If not, the state machine waits until the Proposal is imported and the best block changes before moving to the next height.

## Gossip

Tendermint's Propose, Prevote, and Precommit are delivered to all nodes via the Gossip algorithm. Gossip has Pull Gossip and Push Gossip. Pull Gossip requests peers for unknown information. Push Gossip enables the broadcasting of information to peers that do not have that information. CodeChain uses Pull Gossip.

### Status Propagation

```rust
StepState {
   vote_step: VoteStep,
   proposal: Option<BlockHash>,
   lock_view: Option<View>,
   known_votes: BitSet,
},
```

For Pull Gossip, it is important to communicate one’s state to the peers. When the height or view changes, or when a new proposal or vote is received, a `StepState` message is sent to random peers nearby. The `vote_step` field contains information about the height, view, and step. The `proposal` contains proposal information of the current view. `lock_view` contains the view that is contained in the lock at the current height. `known_votes` is a bitset of Prevote votes in the Prevote step, and a bitset of Precommit votes in the Precommit or Commit step.

### Proposal Propagation

The node that created a proposal sends a proposal block to all peers because all peers do not know about the proposal that was created. A peer that receives the proposal informs other peers using the `StepState` message that it has received a proposal. When a peer knows that another peer has received a proposal through a `StepState` message, it requests for that proposal from the peer that has that proposal.

### Prevote/Precommit Propagation

The node that generated the vote spreads the vote to random nearby nodes. The node receiving the vote sets the `known_votes` field of the `StepState` and informs the peers of its state. The peer that receives the `StepState` asks for a vote that it does not have.

### Commit Propagation

If the connected peer has a height that is 1 or 2 higher, then a `Commit` message that can skip the current height is requested. The `Commit` message consists of the block and Precommits, and once the `Commit` message is received, it goes on to the next height.

### Role of lock_view

If a node is locked, it shares its view of the lock via a `StepState` message. If the value of the `lock_view` received via a `StepState` message from another peer is higher than its own `lock_view`, it requests prevote messages of that view.

If some nodes in the network are locked and others are not due to network issues, a locked node must re-propose the lock's proposal to achieve consensus. Sharing each other's locks with lock_view increases the probability that the next view's proposer will be locked, likely resulting in faster consensus.


## Events

### Block sync

#### new_blocks

Called when a block is imported. There is the `imported` argument and the `enacted` argument. Depending on the situation, the values that go into the two arguments change.

1. When multiple blocks are imported via block sync
The last block is the best proposal block and the second to last block is the best block. All the blocks go into the `imported` argument. All the blocks except the last one go into the `enacted` argument. Then the height should be changed to that of the last block(the last block in the `imported` argument) and the Precommits from the last block should be added to `votes`.

2. If a proposal block received from the Tendermint consensus is imported.
0 blocks go into `enacted` and 1 block goes into `imported`. Then add the Precommits of the block to `votes`. If the block is a proposal of the current view and is currently at the proposal state, move to the Prevote state.

3. If the best block changes in the commit state due to a request for the best block to be replaced.
`imported` is empty and `enacted` only contains one block. If the block is imported and all of the precommits have been collected, it goes on to the next height. Otherwise, if the block is imported and the commit timeout has already passed, go to the next height. If both do not apply, wait for the commit timeout.


### Block generation

#### generate_seal

Called at the beginning of block creation to fill in the header's seal field. The seal consists of four fields in total. The first field is a view of when the previous block was agreed. The second field is a view of when this block was created. The third field is the Precommits for the previous block, and the fourth field is the bitset value for who signed the Precommits.

#### proposal_generated

Called when the requested block is created. This function is called before the block is imported. When this event is called, a node generates a signature for the proposal.

### Timer

#### on_timeout

There are three types of timers registered.

1. Empty Proposal Timeout
When the Proposer creates an empty block, it will wait 1/2 * Propose timeout before broadcasting the block to other nodes.

2. Broadcast timeout
Called once every second. If there are votes received in the last 1 second, it informs the neighboring random peers of the current status.

3. Step timeout
Register a timer at the beginning of the Propose, Prevote, Precommit, or Commit steps. The operation differs depending on which step it is called.


### Network

#### on_proposal_message

Called when a proposal message is received.

#### on_request_proposal_message

Called when a proposal request message is received.

#### on_step_state_message

This function is called when a node receives its peer's information. If the other party's height is greater by 1 or 2, the node requests a commit message. If the other party is the same height and has a proposal that the node does not have, the node asks for a proposal message. If the other party has the same height and same step as the node and the other party has a vote that the node does not have, the node asks for those votes.

#### on_request_commit_message

Called when a commit request message is received.

#### on_commit_message

Called when a commit message is received.

#### handle_message

Called when a Prevote or Precommit is received.
