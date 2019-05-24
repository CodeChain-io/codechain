CodeChain [![Build Status](https://travis-ci.org/CodeChain-io/codechain.svg?branch=master)](https://travis-ci.org/CodeChain-io/codechain) [![Gitter: CodeChain](https://img.shields.io/badge/gitter-codechain-4AB495.svg)](https://gitter.im/CodeChain-io/codechain) [![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
==============

CodeChain is a programmable open source blockchain technology optimal for developing and customizing multi-asset management systems.

## Build

Download CodeChain code

```sh
git clone git@github.com:CodeChain-io/codechain.git
cd codechain
```

Build in release mode

```sh
cargo build --release
```

This will produce an executable in the `./target/release` directory.

## Setup

### Using Docker

CodeChain supports the use of Docker to provide an easy and seamless installation process by providing a single package that gives the user everything he/she
needs to get CodeChain up and running. In order to get the installation package, run the following command after installing Docker:

```sh
docker build -f docker/ubuntu/Dockerfile --tag codechain-io/codechain:branch_or_tag_name .
```    

WSL users may find difficulty in using Docker, and thus, it is highly recommended to use Ubuntu, or install Docker for Windows. When using Docker for Windows,
it is necessary to enable Hyper-V in BIOS settings.

To see the Docker images created, run the following:
```sh
docker images
```

It will result in something like this:
```sh
REPOSITORY               TAG                  IMAGE ID            CREATED              SIZE
codechain-io/codechain   branch_or_tag_name   6f8474d9bc7a        About a minute ago   1.85GB
ubuntu                   14.04                971bb384a50a        6 days ago           188MB
```
    
If you want to run the first image file, run the following command:
```sh
docker run -it codechain-io/codechain:branch_or_tag_name
```

This should result in CodeChain running.

#### Making local database and keys persistent

CodeChain depends on the local database and keys commonly stored under the directories `keys` and `db`. A Docker container is independent of host environment and other Docker images. Therefore, when running a new Docker container with an image with a new CodeChain version or even with the same image, the database and keys are not persistent. To solve the problem, one can take advantage of the Docker's volume option. With the command below,
```sh
docker run -it -v codechain-db-vol:/app/codechain/db -v codechain-keys-vol:/app/codechain/keys codechain-io/codechain:branch_or_tag_name
```
one can mount the volume `codechain-db-vol` into `/app/db` and the volume `codechain-keys-vol` into `/app/keys` in the container. This command will automatically create volumes if existing volumes with specified names do not exist. Because the default working directory specified in `Dockerfile` is `/app/codechain`, the default db and keys path are `/app/codechain/db` and `app/codechian/keys`. One can also customize the paths with CodeChain cli arguments `base-path`, `key-path` and `db-path`.

```sh
docker run -it -v codechain-db-vol:custom_base_path/db -v codechain-keys-vol:custom_base_path/keys codechain-io/codechain:branch_or_tag_name --base-path custom_base_path
```

```sh
docker run -it -v codechain-db-vol:custom_db_path -v codechain-keys-vol:custom_keys_path codechain-io/codechain:branch_or_tag_name --db-path custom_db_path --keys-path custom_keys_path
```
With the methods above, node organizers can manage their local persistent data using docker images.

### Building From Source

#### Build Dependencies
CodeChain requires Rust version 1.34.0 to build. Using [rustup](https://rustup.rs/ "rustup URL") is recommended.

- For Linux Systems:
  - Ubuntu

    > `gcc`, `g++` and `make` are required for installing packages.
    ```sh
    $ curl https://sh.rustup.rs -sSf | sh
    ```
        

- For Mac Systems:
  - MacOS 10.13.2 (17C88) tested
    > `clang` is required for installing packages.

    ```sh
    $ curl https://sh.rustup.rs -sSf | sh
    ```
        

- For Windows Systems:
  - Currently not supported for Windows. If on a Windows system, please install [WSL](https://docs.microsoft.com/en-us/windows/wsl/install-win10) to continue as Ubuntu.

Please make sure that all of the binaries above are included in your `PATH`. These conditions must be fulfilled before building CodeChain from source.


Download CodeChain's source code and go into its directory.
```sh
git clone git@github.com:CodeChain-io/codechain.git
cd codechain
```

#### Build as Release Version
```sh
cargo build --release
```

This will produce an executable in the ./target/release directory.

### Using CodeChain SDK

Before starting to use the CodeChain SDK, please install node.js by going to this [page](https://nodejs.org/en/).

Next, install the package with the following command:

`npm install codechain-sdk` or `yarn add codechain-sdk`

## Run

To run CodeChain, just run

```sh
./target/release/codechain -c solo
```
You can create a block by sending a parcel through [JSON-RPC](https://github.com/CodeChain-io/codechain/blob/master/spec/JSON-RPC.md) or [JavaScript SDK](https://api.codechain.io/).

## Formatting


Make sure you run `rustfmt` before creating a PR to the repo. You need to install the nightly-2018-12-06 version of `rustfmt`.

```sh
rustup toolchain install nightly-2019-05-17
rustup component add rustfmt --toolchain nightly-2019-05-17
```

To run `rustfmt`,

```sh
cargo +nightly-2019-05-17 fmt
```

## Linting

You should run `clippy` also. This is a lint tool for rust. It suggests more efficient/readable code.
You can see [the clippy document](https://rust-lang.github.io/rust-clippy/master/index.html) for more information.
You need to install the nightly-2019-05-17 version of `clippy`.

### Install
```sh
rustup toolchain install nightly-2019-05-17
rustup component add clippy --toolchain nightly-2019-05-17
```

### Run

```sh
cargo +nightly-2019-05-17 clippy --all --all-targets
```

## Testing

Developers are strongly encouraged to write unit tests for new code, and to submit new unit tests for old code. Unit tests can be compiled and run with: `cargo test --all`. For more details, please reference [Unit Tests](https://github.com/CodeChain-io/codechain/wiki/Unit-Tests).

## User Manual

Under `docs` folder, run following command.
```sh
make html
```
User manual will be generated at `docs/_build/html`.

## License
CodeChain is licensed under the AGPL License - see the [LICENSE](https://github.com/CodeChain-io/codechain/blob/master/LICENSE) file for details
