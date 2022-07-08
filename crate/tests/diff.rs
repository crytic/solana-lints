use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{fs::read_to_string, path::Path};

struct Diff {
    sealevel_attacks_programs_dir: &'static str,
    solana_lint: &'static str,
}

const DIFFS: &[Diff] = &[
    Diff {
        sealevel_attacks_programs_dir: "0-signer-authorization",
        solana_lint: "missing_signer_check",
    },
    Diff {
        sealevel_attacks_programs_dir: "2-owner-checks",
        solana_lint: "missing_owner_check",
    },
    Diff {
        sealevel_attacks_programs_dir: "3-type-cosplay",
        solana_lint: "type_cosplay",
    },
    Diff {
        sealevel_attacks_programs_dir: "9-closing-accounts",
        solana_lint: "insecure_account_close",
    },
];

#[test]
fn diff() {
    let tempdir = tempfile::tempdir_in(".").unwrap();

    std::process::Command::new("git")
        .args([
            "clone",
            "https://github.com/project-serum/sealevel-attacks",
            &tempdir.path().to_string_lossy(),
        ])
        .assert()
        .success();

    for diff in DIFFS {
        let contents = read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("diffs")
                .join(diff.solana_lint.to_owned() + ".diff"),
        )
        .unwrap();

        std::process::Command::new("diff")
            .args([
                "-r",
                ".",
                &Path::new("..")
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("lints")
                    .join(diff.solana_lint)
                    .join("ui")
                    .to_string_lossy(),
            ])
            .current_dir(
                tempdir
                    .path()
                    .join("programs")
                    .join(diff.sealevel_attacks_programs_dir),
            )
            .assert()
            .stdout(predicate::eq(contents.as_str()));
    }
}
