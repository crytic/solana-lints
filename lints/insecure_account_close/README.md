# insecure_account_close

**What it does:** Checks for attempts to close an account by setting its lamports to 0 but
not also clearing its data. See:
https://docs.solana.com/developing/programming-model/transactions#multiple-instructions-in-a-single-transaction
