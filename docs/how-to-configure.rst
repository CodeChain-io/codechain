.. _how-to-configure

########################
How To Configure
########################
CodeChain can be configured with either CLI options or a config file. When it comes to which options take precedence, it goes from CLI, user's own config.toml file, and config.dev.toml in that order.

CLI options can be listed by running the command ``$codechain --help``. By using the CLI options, or custom config files, the user can overwrite config.dev.toml's configurations.

Config File
===========
The default preset ``config.dev.toml`` file can be located in ``codechain/config/presets/config.dev.toml``.

Config files can be customized by the user and its location can be designated by using the CLI command ``--config``. Custom config files created by the user must have the proper custom path.

Default config.dev.toml
=======================
The following represents the default configuration values of ``config.dev.toml``.
::

    [codechain]
    quiet = false
    db_path = "db"
    keys_path = "keys"
    chain = "solo"

    [mining]

    [network]
    disable = false
    port = 3485
    max_peers = 30
    min_peers = 10
    bootstrap_addresses = []
    sync = true
    transaction_relay = true
    discovery = true
    discovery_type = "unstructured"
    discovery_refresh = 60000
    discovery_bucket_size = 10

    [rpc]
    disable = false
    interface = "127.0.0.1"
    port = 8080

    [ipc]
    disable = false
    path = "/tmp/jsonrpc.ipc"

    [snapshot]
    disable = false
    path = "snapshot"

CodeChain is set to use the Solo consensus algorithm by default. Tendermint is not suitable for solo testing purposes, since it requires a minimum of 4 users to function properly.

In order to test CodeChain alone, you may want to change chain to Solo. To do this, use ``--chain solo``.

CLI Options for CodeChain client
================================
    ``--config=[PATH]``
        Specify the certain config file path that you want to use to configure CodeChain to your needs.

    ``--port=[PORT]``
        Listen for connections on PORT. (default: 3485)

    ``--bootstrap-addresses=[BOOTSTRAP_ADDRESSES]``
        Bootstrap addresses to connect.

    ``--no-network``
        Do not open network socket.

    ``--min-peers=[NUM]``
        Set the minimum number of connections the user would like. (default: 10)

    ``--max-peers=[NUM]``
        Set the maximum number of connections the user would like. (default: 30)

    ``--instance-id=[ID]``
        Specify instance id for logging. Used when running multiple instances of CodeChain.

    ``--quiet``
        Do not show any synchronization information in the console.

    ``--chain=[CHAIN]``
        Set the blockchain type out of solo, simple_poa, tendermint or a path to chain scheme file. (default: solo)

    ``--db-path=[PATH]``
        Specify the database directory path.

    ``--keys-path=[PATH]``
        Specify the path for JSON key files to be found.

    ``--snapshot-path=[PATH]``
        Specify the snapshot directory path.

    ``--no-sync``
        Do not run block sync extension.

    ``--no-tx-relay``
        Do not relay transactions.

    ``--jsonrpc-interface=[INTERFACE]``
        Specify the interface address for rpc connections

    ``--jsonrpc-port=[PORT]``
        Listen for rpc connections on PORT. (default: 8080)

    ``--no-ipc``
        Do not run JSON-RPC over IPC service.

    ``--ipc-path=[PATH]``
        Specify custom path for JSON-RPC over IPC service

    ``--no-jsonrpc``
        Do not run jsonrpc.

    ``--author=[ADDRESS]``
        Specify the block's author (aka "coinbase") address for sending block rewards from
        sealed blocks.

    ``--engine-signer=[ADDRESS]``
        Specify the address which should be used to sign consensus messages and
        issue blocks.

    ``--mem-pool-fee-bump-shift=[INTEGER]``
        A value which is used to check whether a new transaciton can replace a transaction in the memory pool with the same signer and seq.
        If the fee of the new transaction is `new_fee` and the fee of the transaction in the memory pool is `old_fee`, then `new_fee > old_fee + old_fee >> mem_pool_fee_bump_shift` should be satisfied to replace.
        Local transactions ignore this option.

    ``--mem-pool-mem-limit=[MB]``
        Maximum amount of memory that can be used by the mem pool. Setting this parameter to 0 disables limiting.

    ``--mem-pool-size=[LIMIT]``
        Maximum amount of transactions in the queue (waiting to be included in next block).

    ``--notify-work=[URLS]``
        URLs to which work package notifications are pushed.

    ``--force-sealing``
        Force the node to author new blocks as if it were always sealing/mining.

    ``--reseal-min-period=[MS]``
        Specify the minimum time between reseals from incoming transactions. MS is time measured in milliseconds.

    ``--reseal-max-period=[MS]``
        Specify the maximum time since last block to enable force-sealing. MS is time measured in milliseconds.

    ``--work-queue-size=[ITEMS]``
        Specify the number of historical work packages which are kept cached lest a solution is found for them later. High values take more memory but result in fewer unusable solutions.

    ``--no-discovery``
        Do not use discovery. No automated peer finding.

    ``--discovery="kademlia" | "unstructured"``
        Decide which p2p discovery extension to use. Options are `kademlia <https://github.com/CodeChain-io/codechain/blob/master/spec/Node-Discovery-Protocol.md#kademlia>`_ and unstructured.
        In a testing environment, an unstructured p2p network is desirable because it is
        more than sufficient when there are a few nodes(< 100).
        (default: unstructured)

    ``--discovery-bucket-size=[NUM]``
        Bucket size for discovery. Choose how many addresses to exchange at a time
        during discovery.

    ``--discovery-refresh=[ms]``
        Refresh timeout of discovery (ms). It may conflict with:`` --no-discovery``.

    ``--no-snapshot``
        Disable snapshots
