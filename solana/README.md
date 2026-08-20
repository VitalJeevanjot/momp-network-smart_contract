# Solana MOMP

This Anchor program stores one PDA per email hash and one PDA/vault per payment.
Unlike an account containing an unbounded map, the model scales naturally and a
sender can close each finished payment account to recover its rent.

Core PDA seeds:

* configuration: `["config"]`
* email registration: `["email", email_hash]`
* payment metadata: `["payment", sender, payment_id]`
* escrow vault: `["vault", payment]`

The operator submits OTP commitments. Verification hashes the email, OTP bytes,
and recipient public key with SHA-256. `send_money` transfers immediately for a
verified record, otherwise it funds the payment's vault PDA. The verified owner
can later `claim_payment`; after expiry the original sender can `claim_refund`.

Build with `anchor build` or `cargo check --manifest-path programs/momp-solana/Cargo.toml`.
