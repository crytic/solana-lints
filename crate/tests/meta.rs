#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_data_structures;

use rustc_data_structures::fx::FxHashMap;
use std::{
    ffi::OsStr,
    fs::{read_dir, read_to_string},
    path::Path,
    process::Command,
};
use toml::Value;

#[test]
fn one_toolchain_is_used_throughout() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

    let channel = toolchain_channel(&root);

    let dir = root.join("lints");

    for entry in read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        assert_eq!(channel, toolchain_channel(&path));
    }
}

fn toolchain_channel(path: &Path) -> String {
    let rust_toolchain = path.join("rust-toolchain");
    let file = read_to_string(&rust_toolchain).unwrap();
    let document = toml::from_str::<Value>(&file).unwrap();
    document
        .as_table()
        .and_then(|table| table.get("toolchain"))
        .and_then(Value::as_table)
        .and_then(|table| table.get("channel"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap()
}

#[test]
fn only_lints_have_lockfiles() {
    let output = Command::new("git").arg("ls-files").output().unwrap();

    let stdout = std::str::from_utf8(&output.stdout).unwrap();

    for line in stdout.lines() {
        let path = Path::new(line);
        if path.file_name() == Some(OsStr::new("Cargo.lock")) {
            assert_eq!(path.parent(), Some(Path::new("lints")));
        }
    }
}

#[test]
fn ui_tests_use_distinct_package_names() {
    let mut name_path_map = FxHashMap::default();
    for entry in read_dir("../lints")
        .unwrap()
        .flat_map(|entry| read_dir(entry.unwrap().path().join("ui")).unwrap())
    {
        let entry = entry.unwrap();
        let path_curr = entry.path().canonicalize().unwrap();

        let toml = read_to_string(path_curr.join("Cargo.toml"))
            .unwrap()
            .parse::<toml::Value>()
            .unwrap();

        let package = toml
            .as_table()
            .and_then(|table| table.get("package"))
            .and_then(Value::as_table)
            .unwrap();

        let name = package
            .get("name")
            .and_then(Value::as_str)
            .unwrap()
            .replace('-', "_");

        if let Some(path_prev) = name_path_map.insert(name.clone(), path_curr.clone()) {
            panic!(
                "duplicate package name: {name:?}
 first package: {path_prev:?}
second package: {path_curr:?}"
            );
        }
    }
}
