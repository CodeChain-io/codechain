* Name := “transaction-propagation”
* Version := 0
* Encrypt := never

# Messages

## Transactions (<->)

```
Transactions(tx_0, …)
```

This message MUST contain one or more items. To avoid spamming, sender SHOULD NOT include transaction that is expected to be known by receiver.
