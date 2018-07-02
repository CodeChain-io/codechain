* Name := “block-sync”
* Version := 0
* Encrypt := never

# Messages

```
Message :=
  <-> (message_id . special_message)
| <-  (message_id . request_id . request_content)
| ->  (message_id . response_id . response_content)
```

* Every message has `message_id`, which is message type identifier. Identifier of each message can be found in message description.
* Every request and response message has request/response id. Response for certain message MUST have same id as request.

## Special messages

### Status

```
Status(total_score, best_hash, genesis_hash)
```

Send current chain status to peer.

* Identifier: 0x01
* Restriction: None

## Request messages

### GetHeaders

```
GetHeaders(start_number, max_count)
```

Request at most `max_count` headers, starting from `start_number`.

* Identifier: 0x02
* Restriction: None


### GetBodies

```
GetBodies(hash_0, …)
```

Request corresponding bodies for each hash.

* Identifier: 0x04
* Restriction:
  * MUST include at least one item


### GetStateHead

```
GetStateHead(block_hash)
```

Request corresponding state head for block of `block_hash`.

* Identifier: 0x06
* Restriction: Block number of requested block MUST be multiple of 214.


### GetStateChunk

```
GetStateChunk(block_hash, tree_root)
```

Request entire subtree starting from `tree_root`.

* Identifier: 0x08
* Restriction:
  * Block number of requested block MUST be multiple of 214.
  * `tree_root` MUST be included in requested block’s state trie.
  * Depth of `tree_root` inside state trie MUST be equal to 2. (Depth of state root is 0)


## Response messages

### Headers

```
Headers(header_0, …)
```

Response to `GetHeaders` message. This response MAY contain less number of content than requested if sender has no corresponding items.

* Identifier: 0x03
* Restriction:
  * Headers SHOULD be sorted by block number in ascending order.
  * Headers included in message MUST be continuous. I.e. All parent of headers except MUST exist in message except first one.
  * Lowest block number in the list MUST be equal to `start_number` in request.


### Bodies

```
Bodies(body_0, …)
```

Response to `GetBodies` message.

* Identifier: 0x05
* Restriction:
  * Number and order of bodies included in this message MUST be equal to request information.
  * If sender doesn’t have body for requested hash, corresponding body value MUST be [], not omitted.
  * If received body is zero-length array, it means either body value is [], or sender doesn’t have body for requested hash


### StateHead

```
StateHead(compressed((key_0, value_0), …) | [])
```

Response to `GetStateHead` message. Key and value included in this messages are raw value stored in state trie. Snappy algorithm is used for compression of content.

* Identifier: 0x07
* Restriction:
  * State root of requested block MUST be included
  * For all nodes with depth of less than 2 included in this message, all of its child MUST also be included.
  * Content MUST be empty array if sender didn’t have requested data


### StateChunk
```
StateChunk(compressed((key_0, value_0), …) | [])
```

Response to `GetStateChunk` message. Details of message is same as `StateHead` message.

* Identifier: 0x09
* Restriction:
  * Node corresponding to tree_root in request MUST be included
  * Every nodes included in message MUST have all of its child in same message.
  * Content MUST be empty array if sender didn’t have requested data
