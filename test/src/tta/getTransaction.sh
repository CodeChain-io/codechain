#!/usr/bin/env bash

if [ -z "$1" ]
then
    echo "No argument error. Please use getTransaction <txHash>"
    exit 1;
fi

echo "Tx hash is" $1

curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "chain_getTransaction", "params": ["'$1'"], "id": null}' \
    localhost:2487
