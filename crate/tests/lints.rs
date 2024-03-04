use assert_cmd::prelude::*;
use std::{
    fs::read_dir,
    io::{stderr, Write},
    path::Path,
};

#[test]
fn lints() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("lints");

    for (index, entry) in read_dir(dir).unwrap().enumerate() {
        let entry = entry.unwrap();
        let path = entry.path();

        #[allow(clippy::explicit_write)]
        writeln!(stderr(), "{:?}", path.canonicalize().unwrap()).unwrap();

        std::process::Command::new("cargo")
            .current_dir(path.clone())
            .env_remove("RUSTUP_TOOLCHAIN")
            .env("CARGO_TARGET_DIR", format!("target_{index}"))
            .args(["test"])
            .assert()
            .success();

        std::process::Command::new("cargo")
            .current_dir(path)
            .env_remove("RUSTUP_TOOLCHAIN")
            .env("CARGO_TARGET_DIR", format!("target_{index}"))
            .args(["clean"])
            .assert()
            .success();
    }
}
