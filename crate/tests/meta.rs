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
