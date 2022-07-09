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

    for entry in read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        #[allow(clippy::explicit_write)]
        writeln!(stderr(), "{:?}", path.canonicalize().unwrap()).unwrap();

        std::process::Command::new("cargo")
            .current_dir(path)
            .env_remove("RUSTUP_TOOLCHAIN")
            .args(["test"])
            .assert()
            .success();
    }
}
