# 1.0.0 2019-04-01
* Initial release

# 1.1.0 - 2019-04-04
* Removed stakeholders who don't have stakes from trie ([#1439](https://github.com/CodeChain-io/codechain/pull/1439))
* Fixed the bug in AssetScheme related transactions ([#1442](https://github.com/CodeChain-io/codechain/pull/1442))

# 1.2.0 - 2019-04-19
* Fixed the bug of version flag.
* Added "commit-hash" command
* Added "net_recentNetworkUsage" RPC
* Added "chain_getMinTransactionFee" RPC
* Reduced network traffic
    * Request the header only it need
    * Send new header to random peer instead of all.
* Disabled Order and stake delegation by default
* Enhanced unit tests and e2e tests

# 1.3.0 - 2019-05-13
* Fixed the broken commitHash RPC in a docker image #1443
* Fixed the crash in Tendermint #1514
* Added base-path option #236
* Fixed the crash on exit #348
* Reduced the booting time #1513
