CodeChain [![Build Status](https://travis-ci.org/CodeChain-io/codechain.svg?branch=master)](https://travis-ci.org/CodeChain-io/codechain) [![Gitter: CodeChain](https://img.shields.io/badge/gitter-codechain-4AB495.svg)](https://gitter.im/CodeChain-io/codechain) [![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0) [![Read the Docs](https://img.shields.io/readthedocs/codechain.svg)](https://codechain.readthedocs.io/en/latest/)
==============

CodeChain is a programmable open source blockchain technology optimal for developing and customizing multi-asset management systems.

## Build

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

## Run

To run CodeChain, just run

```
./target/release/codechain -c solo
```
You can create a block by sending a parcel through [JSON-RPC](https://github.com/CodeChain-io/codechain/blob/master/spec/JSON-RPC.md) or [JavaScript SDK](https://api.codechain.io/).

## Formatting


Make sure you run `rustfmt` before creating a PR to the repo. You need to install the nightly-2018-12-06 version of `rustfmt`.

```
rustup toolchain install nightly-2018-12-06
rustup component add rustfmt-preview --toolchain nightly-2018-12-06
```

To run `rustfmt`,

```
cargo +nightly-2018-12-06 fmt
```

## Linting

You should run `clippy` also. This is a lint tool for rust. It suggests more efficient/readable code.
You can see [the clippy document](https://rust-lang.github.io/rust-clippy/master/index.html) for more information.
You need to install the nightly-2018-12-06 version of `clippy`.

### Install
```
rustup toolchain install nightly-2018-12-06
rustup component add clippy-preview --toolchain nightly-2018-12-06
```

### Run

```
cargo +nightly-2018-12-06 clippy --all
```

## Testing

Developers are strongly encouraged to write unit tests for new code, and to submit new unit tests for old code. Unit tests can be compiled and run with: `cargo test --all`. For more details, please reference [Unit Tests](https://github.com/CodeChain-io/codechain/wiki/Unit-Tests).

## User Manual

Under `docs` folder, run following command.
```
make html
```
User manual will be generated at `docs/_build/html`.

## License
CodeChain is licensed under the AGPL License - see the [LICENSE](https://github.com/CodeChain-io/codechain/blob/master/LICENSE) file for details
