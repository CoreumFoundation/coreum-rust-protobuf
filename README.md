# Coreum Rust Protobuf

This repository contains a tool to generate the Rust protobuf structures used to interact with the Coreum blockchain
using gRPC and Rust.

## How it works

What the tool does is:

1. Clones the repository of the Cosmos-SDK, WASMD and Coreum.
2. Extracts the protos of the modules that we want to generate.
3. Generates the initial Rust protobuf structures using `buf build` and `buf generate` commands.
4. Applies transformation to the generated Rust protobuf structures to make them compatible with tools that will use
   our [coreum-wasm-sdk](https://github.com/CoreumFoundation/coreum-wasm-sdk).

## How to use

* Install `1.75.0` rust version `rustup default 1.75.0`.

Clone this repository and run the command `cargo run` in the root of the repository. This will generate the Rust
protobuf structures in the `OUT_DIR` folder specified in `main.rs`.

```rust
//The directory where proto files will be generated
const OUT_DIR: &str = "transformed-protos";
```

If you want to use a different branch/version of the Cosmos-SDK or Wasmd module you can just modify the constants in
`main.rs`:

```rust
// version of the Cosmos SDK that we are using
const COSMOS_SDK_VERSION: &str = "v0.50.9";
// version of the WASMD version that we are using
const WASMD_VERSION: &str = "v0.53.0";
```

If you want to generate protobuf for additional modules, you just need to add them in the INCLUDE_MODS constant in
`main.rs`.

```rust
const INCLUDE_MODS: [&str; 10] = [
    "/cosmos/auth",
    "/cosmos/authz",
    "/cosmos/bank",
    "/cosmos/base",
    "/cosmos/distribution",
    "/cosmos/gov",
    "/cosmos/feegrant",
    "/cosmos/staking",
    "/cosmos/nft",
    "/cosmos/group",
    "/coreum/asset",
    "/coreum/dex",
    "/cosmwasm/wasm",
];
```
