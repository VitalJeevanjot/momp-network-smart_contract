# MOMP multichain contracts

Ports of the original Aeternity/Sophia MOMP email-payment contract to:

* `solidity/` — an EVM implementation (Solidity 0.8.24).
* `solana/` — an Anchor implementation for Solana.

Both ports preserve the core lifecycle: pay an email registration fee, let the
operator register or queue an address, verify the OTP commitment, pay a verified
email immediately or escrow a payment for an unverified email, withdraw escrow,
and claim an expired refund. Email addresses are represented by a 32-byte hash;
never put a plaintext email or OTP on-chain.

The SMTP oracle call in the legacy contract is deliberately represented by an
opaque `queryData` event payload on EVM and a generated payment public key on
Solana. Off-chain relayers can observe these without either port trusting an
external oracle contract for custody accounting.

## Solidity

```sh
cd solidity
forge test
```

## Solana

```sh
cd solana
anchor test
```

See each directory's README for deployment and API details. These contracts
have not been audited and should not be used with production funds before an
independent security review.
