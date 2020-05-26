Check TPS
=========

## Run ping test

```
cd test
cargo build --release
yarn mocha -r ts-node/register --timeout 5000 src/tendermint.test/ping.ts
```

## Run throughput test in local

```
cd test
cargo build --release
NODE_ENV=production yarn ts-node src/tendermint.test/throuput.ts
```
