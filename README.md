# MOMP: Money Over Mail Protocol

Send Aeternity (AE) to an email address. The recipient does not need a wallet when the money is sent. They prove they own the address with a one-time code delivered by email, link a wallet, and withdraw. If nobody claims a payment in time, the sender can take a refund.

Built in September 2021 by Jeevanjot Singh Vital on top of the [Say Network oracle](https://github.com/VitalJeevanjot/oracle-service), which carries the email step on-chain.

**Watch:** [demo video](https://www.youtube.com/watch?v=2CzktZvRIdM) · **Read:** [forum introduction](https://forum.aeternity.com/t/money-over-mail-protocol-momp/10066) · [step-by-step tutorials on DEV](https://dev.to/genievot)

## How it works

1. **Register an email.** The backend generates a one-time password (OTP), emails it, and stores only a hash of the email and OTP in the contract (`add_email`).
2. **Verify ownership.** The owner submits the OTP. The contract checks it against the stored hash (`verify_otp`) and links the email to a wallet address.
3. **Send money to an email.** Anyone can pay to an email hash, verified or not. The contract holds the funds against a payment ID.
4. **Withdraw or refund.** A verified owner withdraws their balance (`clients_withdraw`). After a set time, the sender can reclaim an unclaimed payment (`claim_refund`).
5. **Move to a new wallet.** A verified owner can change the linked address with a fresh OTP (`verify_and_edit_email_with_otp`).

The contract also exposes read-only views for balances, payment IDs, senders and fees. The contract owner can set the base and registration fees.

## Repositories

| Repository | What it holds |
| --- | --- |
| [momp-network-smart_contract](https://github.com/VitalJeevanjot/momp-network-smart_contract) (this repo) | The Sophia contract, `merged_momp.aes`, with the Say oracle library inlined |
| [momp-network-backend](https://github.com/VitalJeevanjot/momp-network-backend) | Node.js service that sends OTP emails over SMTP and calls the contract |
| [momp_network_front-end](https://github.com/VitalJeevanjot/momp_network_front-end) | Quasar (Vue) web app; the current version is in `b_momp - NOW` |
| [momp-network-document](https://github.com/VitalJeevanjot/momp-network-document) | Protocol document (PDF, 24 September 2021) |

## Status

This is a 2021 prototype and is not maintained. It has not been audited, so don't use it with real funds. An experimental pull request ([#1](https://github.com/VitalJeevanjot/momp-network-smart_contract/pull/1)) sketches Solidity and Solana versions. It is unreviewed and not merged.
