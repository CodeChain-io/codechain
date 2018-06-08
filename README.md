CodeChain Core [![Build Status](https://travis-ci.com/kodebox-io/codechain.svg?token=M5mUpGsZqiCqxcx6XsLP&branch=master)](https://travis-ci.com/kodebox-io/codechain) [![Gitter chat](https://badges.gitter.im/CodeChain-io/codechain.png)](https://gitter.im/CodeChain-io/codechain)
==============

CodeChain Core is software designed to operate and connect to highly scalable permissioned blockchain networks conforming to the CodeChain Protocol.

# Run

Use [cargo-watch](https://github.com/passcod/cargo-watch) to monitor for any changes in the source tree and restart the server.

```
cargo watch -x run
```

# Testing

Developers are strongly encouraged to write unit tests for new code, and to submit new unit tests for old code. Unit tests can be compiled and run with: `cargo test`.

# User Manual

Under `docs` folder, run following command.
```
make html
```
User manual will be generated at `docs/_build/html`.

# License
CodeChain is licensed under the AGPL License - see the LICENSE file for details
