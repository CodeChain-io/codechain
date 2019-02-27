* Name := "discovery"
* Version := 0
* Encrypt := optional

# Messages

## Request (->)

```
Request(limit)

limit := u64
```

## Response (<-)

```
Response(Contacts)

Contacts := nil
	| Contact . Contacts
Contact := SocketAddr
```
