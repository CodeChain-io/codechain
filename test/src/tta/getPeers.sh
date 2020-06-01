#!/usr/bin/env bash

curl \
    -H 'Content-Type: application/json' \
    -d '{"jsonrpc": "2.0", "method": "net_getEstablishedPeers", "params": [], "id": null}' \
    localhost:2487
