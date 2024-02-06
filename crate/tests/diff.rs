// smoelius: The purpose of this test is to check the differences between each lint's ui tests and
// their corresponding subdirectory of:
//   https://github.com/coral-xyz/sealevel-attacks/tree/master/programs
// Ideally, those changes should be small.
//   The changes to a lint's ui tests are stored in a file in the `diffs` subdirectory. If any
// lint's actual changes differ from the changes reflected in the lint's diff file, the test fails.

use assert_cmd::prelude::*;
use similar_asserts::SimpleDiff;
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
        sealevel_attacks_programs_dir: "5-arbitrary-cpi",
        solana_lint: "arbitrary_cpi",
    },
    Diff {
        sealevel_attacks_programs_dir: "7-bump-seed-canonicalization",
        solana_lint: "bump_seed_canonicalization",
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
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("diffs")
            .join(diff.solana_lint.to_owned() + ".diff");

        let contents = read_to_string(&path).unwrap();

        let assert = std::process::Command::new("diff")
            .args([
                "-r",
                "-x",
                "Cargo.lock",
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
            .assert();

        let stdout = std::str::from_utf8(&assert.get_output().stdout).unwrap();

        if contents != stdout {
            if enabled("BLESS") {
                std::fs::write(path, stdout).unwrap();
            } else {
                panic!(
                    "{}",
                    SimpleDiff::from_str(&contents, stdout, "left", "right")
                );
            }
        }
    }
}

#[must_use]
pub fn enabled(key: &str) -> bool {
    std::env::var(key).map_or(false, |value| value != "0")
}
