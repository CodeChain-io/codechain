Integration tests are for testing CodeChain application as a whole. Related files are located in this directory, and written in TypeScript.

# Installing Dependencies

[yarn](https://yarnpkg.com/lang/en/) is the package manager we use. To install dependencies, run the following command in the `test` directory:
```
yarn install
```

# Building CodeChain executable

(We're going to automate this step in the near future)

Currently, it requires you to compile CodeChain in advance to run the tests. The integration tests will directly execute the binary in `target/debug` directory or `target/release` directory.

```sh
# debug
cargo build
```

```sh
# release
cargo build --release
```

# Running Tests

To run integration tests, run following command in integration test directory.
```sh
# debug
yarn start
```

```
# release
NODE_ENV=production yarn start
```

# Writing Test

Simple integration test that sends a parcel and gets an invoice from CodeChain is implemented at `src/basic.test.ts`. It would be a good starting point for implementing new tests.

Writing an integration test involves spawning a new CodeChain process and attaching SDK to it. Helper class for automating this process is defined under `src/helper/spawn.ts`, named `CodeChain`. Some important functions are described below:

### constructor
Assigns globally unique instance id to this object. Many parameters that need to avoid conflict(such as a port number) are derived from this instance id.

### start
Spawns a new CodeChain node, and returns Promise that resolves when initialization is completed.

### clean
Kills a process and cleanup files created while running this node.