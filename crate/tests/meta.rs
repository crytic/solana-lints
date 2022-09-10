use std::{
    fs::{read_dir, read_to_string},
    path::Path,
};
use toml::Value;

#[test]
fn all_lints_use_the_same_toolchain() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("lints");

    let mut channel = None;

    for entry in read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        let curr = toolchain_channel(&path);
        if let Some(channel) = channel.as_deref() {
            assert_eq!(channel, curr);
        } else {
            channel = Some(curr);
        }
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
