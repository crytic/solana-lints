// smoelius: Default const names to:
//   crate '_' last_segment
// all in upper snake case.

pub const ANCHOR_LANG_ACCOUNT: [&str; 4] = ["anchor_lang", "accounts", "account", "Account"];
pub const ANCHOR_LANG_PROGRAM: [&str; 4] = ["anchor_lang", "accounts", "program", "Program"];
pub const ANCHOR_LANG_SYSTEM_ACCOUNT: [&str; 4] =
    ["anchor_lang", "accounts", "system_account", "SystemAccount"];
pub const ANCHOR_LANG_ACCOUNT_DESERIALIZE: [&str; 2] = ["anchor_lang", "AccountDeserialize"];
pub const ANCHOR_LANG_CONTEXT: [&str; 3] = ["anchor_lang", "context", "Context"];
pub const ANCHOR_LANG_DISCRIMINATOR: [&str; 2] = ["anchor_lang", "Discriminator"];
pub const ANCHOR_LANG_SIGNER: [&str; 4] = ["anchor_lang", "accounts", "signer", "Signer"];
pub const ANCHOR_LANG_TO_ACCOUNT_INFO: [&str; 3] =
    ["anchor_lang", "ToAccountInfo", "to_account_info"];
pub const ANCHOR_LANG_TRY_DESERIALIZE: [&str; 3] =
    ["anchor_lang", "AccountDeserialize", "try_deserialize"];

pub const BORSH_TRY_FROM_SLICE: [&str; 4] = ["borsh", "de", "BorshDeserialize", "try_from_slice"];

pub const CORE_BRANCH: [&str; 5] = ["core", "ops", "try_trait", "Try", "branch"];

pub const SOLANA_PROGRAM_ACCOUNT_INFO: [&str; 3] =
    ["solana_program", "account_info", "AccountInfo"];
pub const SOLANA_PROGRAM_INVOKE: [&str; 3] = ["solana_program", "program", "invoke"];
pub const SOLANA_PROGRAM_CREATE_PROGRAM_ADDRESS: [&str; 4] = [
    "solana_program",
    "pubkey",
    "Pubkey",
    "create_program_address",
];

pub const SPL_TOKEN_INSTRUCTION: [&str; 2] = ["spl_token", "instruction"];
