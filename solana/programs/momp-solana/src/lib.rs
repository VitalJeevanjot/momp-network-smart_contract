use anchor_lang::prelude::*;
use anchor_lang::solana_program::{hash::hashv, program::invoke, system_instruction};

declare_id!("MoMP111111111111111111111111111111111111111");

#[program]
pub mod momp_solana {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.owner = ctx.accounts.owner.key();
        config.registration_fee = 10_000_000;
        config.base_fee = 50_000_000;
        config.refund_period = 15 * 24 * 60 * 60;
        config.fees_collected = 0;
        config.bump = ctx.bumps.config;
        Ok(())
    }

    pub fn set_fees(ctx: Context<OnlyOwner>, registration_fee: u64, base_fee: u64, refund_period: i64) -> Result<()> {
        require!(refund_period >= 0, MompError::InvalidValue);
        let config = &mut ctx.accounts.config;
        config.registration_fee = registration_fee;
        config.base_fee = base_fee;
        config.refund_period = refund_period;
        Ok(())
    }

    pub fn withdraw_fees(ctx: Context<WithdrawFees>, amount: u64) -> Result<()> {
        require!(amount <= ctx.accounts.config.fees_collected, MompError::InvalidValue);
        ctx.accounts.config.fees_collected -= amount;
        **ctx.accounts.config.to_account_info().try_borrow_mut_lamports()? -= amount;
        **ctx.accounts.recipient.try_borrow_mut_lamports()? += amount;
        Ok(())
    }

    pub fn pay_registration_fee(ctx: Context<PayRegistrationFee>, email_hash: [u8; 32]) -> Result<()> {
        transfer(&ctx.accounts.payer, &ctx.accounts.config.to_account_info(), ctx.accounts.config.registration_fee, &ctx.accounts.system_program)?;
        ctx.accounts.config.fees_collected = ctx.accounts.config.fees_collected.checked_add(ctx.accounts.config.registration_fee).ok_or(MompError::Overflow)?;
        let email = &mut ctx.accounts.email;
        email.email_hash = email_hash;
        email.fee_paid = true;
        email.bump = ctx.bumps.email;
        Ok(())
    }

    pub fn add_email(ctx: Context<AddEmail>, _email_hash: [u8; 32], account: Pubkey, otp_commitment: [u8; 32]) -> Result<()> {
        let email = &mut ctx.accounts.email;
        require!(email.fee_paid && account != Pubkey::default() && otp_commitment != [0; 32], MompError::InvalidState);
        if email.account == Pubkey::default() {
            email.account = account;
            email.otp_commitment = otp_commitment;
        } else {
            email.pending_account = account;
            email.pending_otp_commitment = otp_commitment;
        }
        Ok(())
    }

    pub fn verify_otp(ctx: Context<VerifyOtp>, _email_hash: [u8; 32], otp: Vec<u8>) -> Result<()> {
        require!(otp.len() <= 128, MompError::InvalidValue);
        let email = &mut ctx.accounts.email;
        let editing = email.pending_account != Pubkey::default();
        let candidate = if editing { email.pending_account } else { email.account };
        let expected = if editing { email.pending_otp_commitment } else { email.otp_commitment };
        require!(!editing || ctx.accounts.signer.key() == email.account, MompError::Unauthorized);
        let digest = hashv(&[&email.email_hash, &otp, candidate.as_ref()]).to_bytes();
        require!(digest == expected, MompError::OtpMismatch);
        email.account = candidate;
        email.otp_commitment = expected;
        email.pending_account = Pubkey::default();
        email.pending_otp_commitment = [0; 32];
        email.verified = true;
        email.fee_paid = false;
        emit!(EmailVerified { email_hash: email.email_hash, account: candidate });
        Ok(())
    }

    pub fn send_money(ctx: Context<SendMoney>, _email_hash: [u8; 32], payment_id: [u8; 32], amount: u64, query_data: Vec<u8>) -> Result<()> {
        require!(query_data.len() <= 512 && amount > ctx.accounts.config.base_fee, MompError::InvalidValue);
        let net = amount.checked_sub(ctx.accounts.config.base_fee).ok_or(MompError::Overflow)?;
        transfer(&ctx.accounts.sender, &ctx.accounts.config.to_account_info(), ctx.accounts.config.base_fee, &ctx.accounts.system_program)?;
        ctx.accounts.config.fees_collected = ctx.accounts.config.fees_collected.checked_add(ctx.accounts.config.base_fee).ok_or(MompError::Overflow)?;
        let payment = &mut ctx.accounts.payment;
        payment.id = payment_id;
        payment.sender = ctx.accounts.sender.key();
        payment.email_hash = ctx.accounts.email.email_hash;
        payment.amount = net;
        payment.expires_at = Clock::get()?.unix_timestamp.checked_add(ctx.accounts.config.refund_period).ok_or(MompError::Overflow)?;
        payment.bump = ctx.bumps.payment;
        payment.vault_bump = ctx.bumps.vault;
        if ctx.accounts.email.verified {
            require_keys_eq!(ctx.accounts.recipient.key(), ctx.accounts.email.account, MompError::WrongRecipient);
            transfer(&ctx.accounts.sender, &ctx.accounts.recipient, net, &ctx.accounts.system_program)?;
            payment.claimed = true;
        } else {
            transfer(&ctx.accounts.sender, &ctx.accounts.vault.to_account_info(), net, &ctx.accounts.system_program)?;
        }
        emit!(PaymentCreated { payment_id, sender: payment.sender, email_hash: payment.email_hash, amount: net, delivered: payment.claimed, query_data });
        Ok(())
    }

    pub fn claim_payment(ctx: Context<SettlePayment>) -> Result<()> {
        require!(ctx.accounts.email.verified, MompError::InvalidState);
        require_keys_eq!(ctx.accounts.recipient.key(), ctx.accounts.email.account, MompError::WrongRecipient);
        settle(&mut ctx.accounts.payment, &ctx.accounts.vault, &ctx.accounts.recipient, false)
    }

    pub fn claim_refund(ctx: Context<RefundPayment>) -> Result<()> {
        require!(Clock::get()?.unix_timestamp >= ctx.accounts.payment.expires_at, MompError::NotExpired);
        settle(&mut ctx.accounts.payment, &ctx.accounts.vault, &ctx.accounts.sender.to_account_info(), true)
    }
}

fn transfer<'info>(from: &Signer<'info>, to: &AccountInfo<'info>, amount: u64, system: &Program<'info, System>) -> Result<()> {
    invoke(&system_instruction::transfer(&from.key(), &to.key(), amount), &[from.to_account_info(), to.clone(), system.to_account_info()])?;
    Ok(())
}

fn settle(payment: &mut Account<Payment>, vault: &Account<Vault>, recipient: &AccountInfo, refunded: bool) -> Result<()> {
    require!(!payment.claimed && !payment.refunded, MompError::AlreadySettled);
    require!(vault.to_account_info().lamports() >= payment.amount, MompError::InvalidState);
    **vault.to_account_info().try_borrow_mut_lamports()? -= payment.amount;
    **recipient.try_borrow_mut_lamports()? += payment.amount;
    payment.claimed = !refunded;
    payment.refunded = refunded;
    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = owner, space = 8 + Config::INIT_SPACE, seeds = [b"config"], bump)] pub config: Account<'info, Config>,
    #[account(mut)] pub owner: Signer<'info>, pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct OnlyOwner<'info> {
    #[account(mut, seeds = [b"config"], bump = config.bump, has_one = owner)] pub config: Account<'info, Config>, pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    #[account(mut, seeds = [b"config"], bump = config.bump, has_one = owner)] pub config: Account<'info, Config>,
    pub owner: Signer<'info>,
    /// CHECK: owner chooses the fee recipient.
    #[account(mut)] pub recipient: AccountInfo<'info>,
}

#[derive(Accounts)]
#[instruction(email_hash: [u8; 32])]
pub struct PayRegistrationFee<'info> {
    #[account(mut, seeds = [b"config"], bump = config.bump)] pub config: Account<'info, Config>,
    #[account(init_if_needed, payer = payer, space = 8 + EmailRecord::INIT_SPACE, seeds = [b"email", email_hash.as_ref()], bump)] pub email: Account<'info, EmailRecord>,
    #[account(mut)] pub payer: Signer<'info>, pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(email_hash: [u8; 32])]
pub struct AddEmail<'info> {
    #[account(seeds = [b"config"], bump = config.bump, has_one = owner)] pub config: Account<'info, Config>, pub owner: Signer<'info>,
    #[account(mut, seeds = [b"email", email_hash.as_ref()], bump = email.bump)] pub email: Account<'info, EmailRecord>,
}

#[derive(Accounts)]
#[instruction(email_hash: [u8; 32])]
pub struct VerifyOtp<'info> {
    #[account(mut, seeds = [b"email", email_hash.as_ref()], bump = email.bump)] pub email: Account<'info, EmailRecord>, pub signer: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(email_hash: [u8; 32], payment_id: [u8; 32])]
pub struct SendMoney<'info> {
    #[account(mut, seeds = [b"config"], bump = config.bump)] pub config: Account<'info, Config>,
    #[account(seeds = [b"email", email_hash.as_ref()], bump = email.bump)] pub email: Account<'info, EmailRecord>,
    #[account(init, payer = sender, space = 8 + Payment::INIT_SPACE, seeds = [b"payment", sender.key().as_ref(), payment_id.as_ref()], bump)] pub payment: Account<'info, Payment>,
    /// CHECK: PDA is a lamport-only escrow with no data.
    #[account(init, payer = sender, space = 8 + Vault::INIT_SPACE, seeds = [b"vault", payment.key().as_ref()], bump)] pub vault: Account<'info, Vault>,
    #[account(mut)] pub sender: Signer<'info>,
    /// CHECK: constrained to the verified address before transfer.
    #[account(mut)] pub recipient: AccountInfo<'info>, pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SettlePayment<'info> {
    #[account(seeds = [b"email", payment.email_hash.as_ref()], bump = email.bump)] pub email: Account<'info, EmailRecord>,
    #[account(mut)] pub payment: Account<'info, Payment>,
    #[account(mut, seeds = [b"vault", payment.key().as_ref()], bump = payment.vault_bump)] pub vault: Account<'info, Vault>,
    /// CHECK: constrained to email.account.
    #[account(mut)] pub recipient: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct RefundPayment<'info> {
    #[account(mut, has_one = sender)] pub payment: Account<'info, Payment>,
    #[account(mut, seeds = [b"vault", payment.key().as_ref()], bump = payment.vault_bump)] pub vault: Account<'info, Vault>,
    #[account(mut)] pub sender: Signer<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct Config { pub owner: Pubkey, pub registration_fee: u64, pub base_fee: u64, pub refund_period: i64, pub fees_collected: u64, pub bump: u8 }

#[account]
#[derive(InitSpace)]
pub struct EmailRecord {
    pub email_hash: [u8; 32], pub account: Pubkey, pub otp_commitment: [u8; 32], pub verified: bool, pub fee_paid: bool,
    pub pending_account: Pubkey, pub pending_otp_commitment: [u8; 32], pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Payment {
    pub id: [u8; 32], pub sender: Pubkey, pub email_hash: [u8; 32], pub amount: u64, pub expires_at: i64,
    pub claimed: bool, pub refunded: bool, pub bump: u8, pub vault_bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Vault {}

#[event]
pub struct EmailVerified { pub email_hash: [u8; 32], pub account: Pubkey }

#[event]
pub struct PaymentCreated {
    pub payment_id: [u8; 32], pub sender: Pubkey, pub email_hash: [u8; 32], pub amount: u64, pub delivered: bool,
    pub query_data: Vec<u8>,
}

#[error_code]
pub enum MompError {
    #[msg("unauthorized")] Unauthorized,
    #[msg("invalid state")] InvalidState,
    #[msg("invalid value")] InvalidValue,
    #[msg("OTP commitment does not match")] OtpMismatch,
    #[msg("wrong recipient")] WrongRecipient,
    #[msg("payment has not expired")] NotExpired,
    #[msg("payment already settled")] AlreadySettled,
    #[msg("arithmetic overflow")] Overflow,
}
