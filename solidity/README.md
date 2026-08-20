# Solidity MOMP

`Momp.sol` is a self-contained EVM port. Amounts are denominated in wei and
refund windows in seconds. The owner is expected to be the SMTP/email-verifying
service: it supplies an OTP commitment when registering an email. The user later
reveals the OTP to prove possession.

An OTP commitment is:

```text
sha256(abi.encodePacked(emailHash, otp, recipientAddress))
```

Payment IDs are deterministic `bytes32` values and all transfers use pull-safe,
checks-effects-interactions accounting. Compile and test with `forge test`.
