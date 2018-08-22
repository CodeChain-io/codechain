.. _logging:

############
Logging
############
For logging, run the following to configure:
``$ RUST_LOG=<level> codechain``

Log Levels
=============
CodeChain currently offers five different ``<level>``. They are error, warn, info, debug, and trace.

For example, the log level will be set to debug, if you run the following:

``$ RUST_LOG="debug" codechain``

* The **error** level represents an event where something can be dangerous, but can still run. In the case in which it cannot run anymore, it must crash ASAP instead of logging.

* The **warn** level represents an event which can be potentially dangerous.

* The **info** level represents an event which is not dangerous, but can be useful information to the users.

* The **debug** level represents an event that is useful for the developers, but not for the users.

* The **trace** level is used for tracing.

Log Targets
==============

Log levels can be set differently for each log targets. For example, you can set ``tx``'s log level as ``trace`` and ``parcel``'s
log level as ``info`` with the following code:

``$ RUST_LOG="tx=trace,parcel=info" codechain``

The possible log targets are as follows:
::

    "blockchain"
    "client"
    "discovery"
    "engine"
    "external_parcel"
    "io"
    "mem_pool"
    "miner"
    "net"
    "netapi"
    "own_parcel"
    "poa"
    "shutdown"
    "snapshot"
    "solo_authoirty"
    "spec"
    "state"
    "state_db"
    "stratum"
    "sync"
    "test_script"
    "trie"
    "tx"
