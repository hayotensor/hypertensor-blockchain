# Build environment

The workspace's `rust-toolchain.toml` pins Rust 1.93.0, the compiler used by the
Polkadot SDK stable2606 release. Rustup selects it automatically from the repository.
The runtime builds for `wasm32v1-none`; no nightly compiler is required.

On Ubuntu/Debian, install the native build dependencies:

```sh
sudo apt install build-essential pkg-config clang libclang-dev libssl-dev protobuf-compiler
rustup show
cargo build --locked --release
```

If libclang is installed outside the system search path, set `LIBCLANG_PATH` to its
library directory. The default node uses RocksDB. For a ParityDB-only build, use
`--no-default-features --features paritydb` and run with `--database paritydb`.

Use `SKIP_WASM_BUILD=1` only for native tests or compile checks. A node built with that
variable cannot generate the built-in genesis presets because it has no embedded runtime.
See [validation](npos-validation.md) and [validator setup](validators.md).
