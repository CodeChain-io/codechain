* Name := “kademlia-discovery”
* Version := 0
* Encrypt := optional

# Messages

## FindNode (->)

```
FindNode(id, sender, target, limit)

sender := NodeId
target := NodeId
limit := u64
```

## Nodes (<-)

```
Nodes(id, Contacts)

id:= u64
Contacts := Contact
	| Contact . Contacts
Contact := NodeId . SocketAddr
```
