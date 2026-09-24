//! Flatten our Solidity sources for Alloy, which does not resolve Solidity imports.
//! The published .sol files remain the single source of truth for the Rust ABI.
use std::{env, fs, path::PathBuf};

fn main() {
    let mut source =
        String::from("pallet_revive::precompiles::alloy::sol! { #![sol(all_derives)]\n");
    for name in [
        "NetworkTypes",
        "IStaking",
        "IValidators",
        "ISubnets",
        "ISubnetNodes",
        "IConsensus",
        "IOverwatch",
    ] {
        let path = format!("interfaces/{name}.sol");
        println!("cargo:rerun-if-changed={path}");
        for line in fs::read_to_string(path)
            .expect("Solidity interface exists")
            .lines()
        {
            if !line.starts_with("pragma ") && !line.starts_with("import ") {
                source.push_str(line);
                source.push('\n');
            }
        }
    }
    source.push_str("}\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("abi.rs"),
        source,
    )
    .unwrap();
}
