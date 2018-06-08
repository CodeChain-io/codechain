CodeChain Core [![Build Status](https://travis-ci.com/kodebox-io/codechain.svg?token=M5mUpGsZqiCqxcx6XsLP&branch=master)](https://travis-ci.com/kodebox-io/codechain) [![Gitter chat](https://badges.gitter.im/CodeChain-io/codechain.png)](https://gitter.im/CodeChain-io/codechain)
==============

CodeChain is a programmable open source blockchain technology optimal for developing and customizing multi-asset management systems.

# Build

Download CodeChain code

```
git clone git@github.com:CodeChain-io/codechain.git
cd codechain
```

Build in release mode

```
cargo build --release
```

This will produce an executable in the `./target/release` directory.

# Run

To run CodeChain, just run

```
./target/release/codechain
```

# Testing

Developers are strongly encouraged to write unit tests for new code, and to submit new unit tests for old code. Unit tests can be compiled and run with: `cargo test --all`.

# User Manual

Under `docs` folder, run following command.
```
make html
```
User manual will be generated at `docs/_build/html`.

# License
CodeChain is licensed under the AGPL License - see the LICENSE file for details
