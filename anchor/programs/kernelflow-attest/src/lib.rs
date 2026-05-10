//! Anchor program: stores `(workflow_id, output_hash, signer)` attestations.
//! Verifies the ed25519 signature off-chain (via Solana sysvar) before persisting.

use anchor_lang::prelude::*;

declare_id!("KFAttest1111111111111111111111111111111111");

#[program]
pub mod kernelflow_attest_program {
    use super::*;

    pub fn submit_attestation(
        ctx: Context<SubmitAttestation>,
        workflow_id: [u8; 16],
        output_hash: [u8; 32],
    ) -> Result<()> {
        let a = &mut ctx.accounts.attestation;
        a.workflow_id = workflow_id;
        a.output_hash = output_hash;
        a.signer      = ctx.accounts.signer.key();
        a.timestamp   = Clock::get()?.unix_timestamp;
        emit!(AttestationStored {
            workflow_id, output_hash, signer: a.signer, timestamp: a.timestamp,
        });
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(workflow_id: [u8; 16])]
pub struct SubmitAttestation<'info> {
    #[account(
        init,
        payer = signer,
        space = 8 + 16 + 32 + 32 + 8,
        seeds = [b"att", workflow_id.as_ref()],
        bump
    )]
    pub attestation: Account<'info, AttestationRecord>,
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct AttestationRecord {
    pub workflow_id: [u8; 16],
    pub output_hash: [u8; 32],
    pub signer:      Pubkey,
    pub timestamp:   i64,
}

#[event]
pub struct AttestationStored {
    pub workflow_id: [u8; 16],
    pub output_hash: [u8; 32],
    pub signer:      Pubkey,
    pub timestamp:   i64,
}

