Stratum is a light-weight mining protocol.

# CLI options for Stratum

 * `--no-stratum`
   > Do not run stratum.
 * `--stratum-port <PORT>`
   > Listen for stratum connections on PORT. [default: 8008]

# List of methods

 * [mining.subscribe](#mining.subscribe)
 * [mining.authorize](#mining.authorize)
 * [mining.notify](#mining.notify)
 * [mining.submit](#mining.submit)

# Specification

## mining.subscribe

Used for subscribing mining jobs.

Params: No parameters

Return Type: No parameters

Request Example
```
{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "mining.subscribe",
    "params": []
}
```

Response Example
```
{
    "jsonrpc": "2.0",
    "id": 1,
    "result": [],
    "error": null
}
```

## mining.authorize

Used for authorizing miners.

Params:
 1. name: `string`
 2. password: `string`

Return Type: `bool`

Request Example
```
{
    "jsonrpc": "2.0",
    "id":2,
    "method": "mining.authorize",
    "params": ["miner1", "password"]
}
```

Response Example
```
{
    "jsonrpc": "2.0",
    "id": 2,
    "result": true,
    "error": null
}
```

## mining.notify

Used for sending notifications regarding mining jobs.

Params: `Work`

Notification Example
```
{
    "jsonrpc": "2.0",
    "id": 3,
    "method": "mining.notify",
    "params": {
        "0x56642f04d519ae3262c7ba6facf1c5b11450ebaeb7955337cfbc45420d573077",
        "100"
    },
}
```

## mining.submit

Used for submitting a proof-of-work solution.

Params:
 1. powHash: `string`
 2. seal: `string[]`

Return Type: `null`

Request Example
```
{
    "jsonrpc": "2.0",
    "id": 4,
    "method": "mining.submit",
    "params": ["0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef", ["0x56642f04d519ae3262c7ba6facf1c5b11450ebaeb7955337cfbc45420d573077"]],
}
```

Response Example
```
{
    "jsonrpc": "2.0",
    "id": 4,
    "result": null,
    "error": null
}
```

## Exception Handling
Stratum defines simple exception handling. Example of a rejected share looks like:
```
{
    "jsonrpc": "2.0",
    "id": 5,
    "error": {"code":21, "message":"Invalid Pow hash"}
}
```

Where the error field is defined as (error_code, human_readable_message).
Proposed error codes for mining services are:
* 20 - Internal Error
* 21 - Invalid Pow hash (=stale)
* 22 - Invalid the nonce
* 23 - Unauthorized worker
