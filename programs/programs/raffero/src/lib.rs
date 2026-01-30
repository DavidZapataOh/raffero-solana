pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke,
    system_instruction,
};

use solana_keccak_hasher::hash;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("BBmLcKphet33PchsFR4NHSmAgwBxtSrTcBsPLG6Sxm1P");

const CLAIM_PUBS: usize = 6;
const FINALIZE_PUBS: usize = 9;

#[program]
pub mod raffero {
    use super::*;

    pub fn create_raffle(
        ctx: Context<CreateRaffle>,
        raffle_id: u64,
        ticket_price: u64,
        max_participants: u32,
        tree_depth: u32,
        end_slot: u64,
        prize_lamports: u64,
        claim_verifier: Pubkey,
        finalize_verifier: Pubkey,
    ) -> Result<()> {
        let r = &mut ctx.accounts.raffle;

        r.authority = ctx.accounts.creator.key();
        r.raffle_id = raffle_id;
        r.ticket_price = ticket_price;
        r.max_participants = max_participants;
        r.tree_depth = tree_depth;
        r.end_slot = end_slot;

        r.participants = 0;
        r.status = RaffleStatus::Active as u8;

        r.pending_root = [0u8; 32];
        r.final_root = [0u8; 32];
        r.alias_root = [0u8; 32];
        r.winner_index = 0;
        r.finalized = false;

        r.claim_verifier = claim_verifier;
        r.finalize_verifier = finalize_verifier;

        // vault (program-owned)
        ctx.accounts.vault.bump = ctx.bumps.vault;

        // Prize: creator -> vault (system transfer funciona aunque el vault sea program-owned)
        if prize_lamports > 0 {
            let ix = system_instruction::transfer(
                &ctx.accounts.creator.key(),
                &ctx.accounts.vault.key(),
                prize_lamports,
            );
            invoke(
                &ix,
                &[
                    ctx.accounts.creator.to_account_info(),
                    ctx.accounts.vault.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
            )?;
        }

        Ok(())
    }

    pub fn submit_ticket(ctx: Context<SubmitTicket>, entry_hash: [u8; 32]) -> Result<()> {
        let r = &mut ctx.accounts.raffle;
        require!(r.status == RaffleStatus::Active as u8, RafferoError::RaffleNotActive);

        let slot = Clock::get()?.slot;
        require!(slot < r.end_slot, RafferoError::RaffleEnded);

        require!(r.participants < r.max_participants, RafferoError::RaffleFull);
        require!(entry_hash != [0u8; 32], RafferoError::InvalidLeaf);

        // cobra ticket al buyer -> vault
        let ix = system_instruction::transfer(
            &ctx.accounts.buyer.key(),
            &ctx.accounts.vault.key(),
            r.ticket_price,
        );
        invoke(
            &ix,
            &[
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.vault.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        // marker anti-duplicate (si ya existía, init falla)
        ctx.accounts.entry_marker.entry_hash = entry_hash;

        r.participants += 1;
        Ok(())
    }

    pub fn finalize_raffle<'info>(
        ctx: Context<'_, '_, 'info, 'info, FinalizeRaffle<'info>>,
        proof: Vec<u8>,
        public_witness: Vec<u8>,
    ) -> Result<()> {
        let r = &mut ctx.accounts.raffle;
        require!(r.status == RaffleStatus::Active as u8, RafferoError::RaffleNotActive);

        let slot = Clock::get()?.slot;
        require!(slot >= r.end_slot, RafferoError::RaffleNotEnded);
        require!(!r.finalized, RafferoError::AlreadyFinalized);

        let pubs = parse_public_witness(&public_witness, FINALIZE_PUBS)?;
        let pending_root = pubs[0];
        let final_root = pubs[1];
        let alias_root = pubs[2];

        let raffle_id = field_be_to_u64(pubs[3]);
        require!(raffle_id == r.raffle_id, RafferoError::InvalidRaffleId);

        let n = field_be_to_u64(pubs[4]) as u32;
        require!(n == r.participants, RafferoError::InvalidParticipants);

        let depth = field_be_to_u64(pubs[5]) as u32;
        require!(depth == r.tree_depth, RafferoError::InvalidTreeDepth);

        require_keys_eq!(ctx.accounts.finalize_verifier.key(), r.finalize_verifier, RafferoError::BadVerifier);

        // CPI al verifier (metas desde remaining_accounts por si el verifier requiere cuentas extra)
        let metas = metas_from_infos(ctx.remaining_accounts);
        let mut data = Vec::with_capacity(proof.len() + public_witness.len());
        data.extend_from_slice(&proof);
        data.extend_from_slice(&public_witness);

        let ix = Instruction {
            program_id: ctx.accounts.finalize_verifier.key(),
            accounts: metas,
            data,
        };

        // OJO: pasamos el program account del verifier + las cuentas requeridas
        let mut infos = Vec::with_capacity(1 + ctx.remaining_accounts.len());
        infos.push(ctx.accounts.finalize_verifier.to_account_info());
        infos.extend_from_slice(ctx.remaining_accounts);
        invoke(&ix, &infos)?;

        r.pending_root = pending_root;
        r.final_root = final_root;
        r.alias_root = alias_root;
        r.finalized = true;

        Ok(())
    }

    pub fn draw_winner(ctx: Context<DrawWinner>) -> Result<()> {
        let r = &mut ctx.accounts.raffle;
        require!(r.finalized, RafferoError::RaffleNotFinalized);
        require!(r.status == RaffleStatus::Active as u8, RafferoError::RaffleNotActive);

        let slot = Clock::get()?.slot;
        require!(slot >= r.end_slot, RafferoError::RaffleNotEnded);
        require!(r.participants > 0, RafferoError::NoParticipants);

        // pseudo-random (demo): hash(slot || final_root) mod n
        let slot_bytes = slot.to_be_bytes();
        let mut data = Vec::with_capacity(40);
        data.extend_from_slice(&slot_bytes);
        data.extend_from_slice(&r.final_root);
        let seed = hash(&data);
        let bytes = seed.to_bytes();
        let x = u64::from_be_bytes(bytes[0..8].try_into().unwrap());
        r.winner_index = (x % (r.participants as u64)) as u32;

        r.status = RaffleStatus::Closed as u8;
        Ok(())
    }

    pub fn claim_prize<'info>(
        ctx: Context<'_, '_, 'info, 'info, ClaimPrize<'info>>,
        proof: Vec<u8>,
        public_witness: Vec<u8>,
        nullifier_hash: [u8; 32],
        relayer_fee: u64,
    ) -> Result<()> {
        let r = &mut ctx.accounts.raffle;
        require!(r.status == RaffleStatus::Closed as u8, RafferoError::RaffleNotClosed);

        let pubs = parse_public_witness(&public_witness, CLAIM_PUBS)?;
        let root = pubs[0];
        let pw_null = pubs[1];

        let raffle_id = field_be_to_u64(pubs[3]);
        let winner_index = field_be_to_u64(pubs[4]) as u32;
        let depth = field_be_to_u64(pubs[5]) as u32;

        require!(raffle_id == r.raffle_id, RafferoError::InvalidRaffleId);
        require!(root == r.final_root, RafferoError::InvalidRoot);
        require!(winner_index == r.winner_index, RafferoError::NotWinner);
        require!(depth == r.tree_depth, RafferoError::InvalidTreeDepth);

        require!(nullifier_hash == pw_null, RafferoError::NullifierMismatch);

        require_keys_eq!(ctx.accounts.claim_verifier.key(), r.claim_verifier, RafferoError::BadVerifier);

        // CPI verifier (metas desde remaining_accounts)
        let metas = metas_from_infos(ctx.remaining_accounts);
        let mut data = Vec::with_capacity(proof.len() + public_witness.len());
        data.extend_from_slice(&proof);
        data.extend_from_slice(&public_witness);

        let ix = Instruction {
            program_id: ctx.accounts.claim_verifier.key(),
            accounts: metas,
            data,
        };

        let mut infos = Vec::with_capacity(1 + ctx.remaining_accounts.len());
        infos.push(ctx.accounts.claim_verifier.to_account_info());
        infos.extend_from_slice(ctx.remaining_accounts);
        invoke(&ix, &infos)?;

        // marker anti-double-claim (init ya garantizó que no existía)
        ctx.accounts.nullifier_marker.used = true;

        // payout desde vault program-owned (moviendo lamports directo)
        let vault_balance = **ctx.accounts.vault.to_account_info().lamports.borrow();
        require!(vault_balance > 0, RafferoError::EmptyPrize);
        require!(relayer_fee <= vault_balance, RafferoError::BadRelayerFee);

        let to_recipient = vault_balance - relayer_fee;

        // vault -> relayer
        if relayer_fee > 0 {
            transfer_lamports(
                &ctx.accounts.vault.to_account_info(),
                &ctx.accounts.relayer.to_account_info(),
                relayer_fee,
            )?;
        }

        // vault -> recipient
        transfer_lamports(
            &ctx.accounts.vault.to_account_info(),
            &ctx.accounts.recipient.to_account_info(),
            to_recipient,
        )?;

        r.status = RaffleStatus::Claimed as u8;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CreateRaffle<'info> {
    #[account(init, payer = creator, space = 8 + Raffle::SPACE)]
    pub raffle: Account<'info, Raffle>,

    #[account(
        init,
        payer = creator,
        seeds = [b"vault", raffle.key().as_ref()],
        bump,
        space = 8 + Vault::SPACE
    )]
    pub vault: Account<'info, Vault>,

    #[account(mut)]
    pub creator: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(entry_hash: [u8; 32])]
pub struct SubmitTicket<'info> {
    #[account(mut)]
    pub raffle: Account<'info, Raffle>,

    #[account(mut, seeds = [b"vault", raffle.key().as_ref()], bump = vault.bump)]
    pub vault: Account<'info, Vault>,

    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        init,
        payer = buyer,
        space = 8 + EntryMarker::SPACE,
        seeds = [b"entry", raffle.key().as_ref(), &entry_hash],
        bump
    )]
    pub entry_marker: Account<'info, EntryMarker>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FinalizeRaffle<'info> {
    #[account(mut)]
    pub raffle: Account<'info, Raffle>,
    /// CHECK: verifier program (Sunspot)
    pub finalize_verifier: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct DrawWinner<'info> {
    #[account(mut)]
    pub raffle: Account<'info, Raffle>,
}

#[derive(Accounts)]
#[instruction(nullifier_hash: [u8; 32])]
pub struct ClaimPrize<'info> {
    #[account(mut)]
    pub raffle: Account<'info, Raffle>,

    #[account(mut, seeds = [b"vault", raffle.key().as_ref()], bump = vault.bump)]
    pub vault: Account<'info, Vault>,

    #[account(mut)]
    pub recipient: SystemAccount<'info>,

    #[account(mut)]
    pub relayer: Signer<'info>,

    #[account(
        init,
        payer = relayer,
        space = 8 + NullifierMarker::SPACE,
        seeds = [b"nullifier", raffle.key().as_ref(), &nullifier_hash],
        bump
    )]
    pub nullifier_marker: Account<'info, NullifierMarker>,

    /// CHECK: verifier program (Sunspot)
    pub claim_verifier: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[account]
pub struct Raffle {
    pub authority: Pubkey,
    pub raffle_id: u64,

    pub ticket_price: u64,
    pub max_participants: u32,
    pub participants: u32,

    pub tree_depth: u32,
    pub end_slot: u64,

    pub pending_root: [u8; 32],
    pub final_root: [u8; 32],
    pub alias_root: [u8; 32],

    pub winner_index: u32,
    pub status: u8,
    pub finalized: bool,

    pub claim_verifier: Pubkey,
    pub finalize_verifier: Pubkey,
}

impl Raffle {
    pub const SPACE: usize =
        32 + 8 +
        8 + 4 + 4 +
        4 + 8 +
        32 + 32 + 32 +
        4 + 1 + 1 +
        32 + 32;
}

#[account]
pub struct Vault {
    pub bump: u8,
}
impl Vault { pub const SPACE: usize = 1; }

#[account]
pub struct EntryMarker {
    pub entry_hash: [u8; 32],
}
impl EntryMarker { pub const SPACE: usize = 32; }

#[account]
pub struct NullifierMarker {
    pub used: bool,
}
impl NullifierMarker { pub const SPACE: usize = 1; }

#[repr(u8)]
pub enum RaffleStatus {
    Active = 0,
    Closed = 1,
    Claimed = 2,
}

#[error_code]
pub enum RafferoError {
    #[msg("Raffle no está activo")]
    RaffleNotActive,
    #[msg("Raffle ya terminó")]
    RaffleEnded,
    #[msg("Raffle aún no termina")]
    RaffleNotEnded,
    #[msg("Raffle lleno")]
    RaffleFull,
    #[msg("Leaf inválida")]
    InvalidLeaf,
    #[msg("Ya finalizado")]
    AlreadyFinalized,
    #[msg("Raffle no finalizado")]
    RaffleNotFinalized,
    #[msg("Sin participantes")]
    NoParticipants,
    #[msg("Verifier incorrecto")]
    BadVerifier,
    #[msg("RaffleId inválido")]
    InvalidRaffleId,
    #[msg("n_participants inválido")]
    InvalidParticipants,
    #[msg("Tree depth inválido")]
    InvalidTreeDepth,
    #[msg("Root inválido")]
    InvalidRoot,
    #[msg("No es winner")]
    NotWinner,
    #[msg("Raffle no está cerrado")]
    RaffleNotClosed,
    #[msg("Nullifier no coincide con public witness")]
    NullifierMismatch,
    #[msg("Premio vacío")]
    EmptyPrize,
    #[msg("Relayer fee inválido")]
    BadRelayerFee,
    #[msg("Public witness inválido")]
    InvalidPublicWitness,
}

fn parse_public_witness(pw: &[u8], expected: usize) -> Result<Vec<[u8; 32]>> {
    require!(pw.len() >= 4, RafferoError::InvalidPublicWitness);

    let n = u32::from_be_bytes(pw[0..4].try_into().unwrap()) as usize;
    require!(n == expected, RafferoError::InvalidPublicWitness);

    let need = 4 + 32 * n;
    require!(pw.len() >= need, RafferoError::InvalidPublicWitness);

    let mut out = Vec::with_capacity(n);
    let mut off = 4;
    for _ in 0..n {
        let mut v = [0u8; 32];
        v.copy_from_slice(&pw[off..off + 32]);
        out.push(v);
        off += 32;
    }
    Ok(out)
}

fn field_be_to_u64(f: [u8; 32]) -> u64 {
    u64::from_be_bytes(f[24..32].try_into().unwrap())
}

fn metas_from_infos(infos: &[AccountInfo]) -> Vec<AccountMeta> {
    infos
        .iter()
        .map(|ai| {
            if ai.is_writable {
                AccountMeta::new(*ai.key, ai.is_signer)
            } else {
                AccountMeta::new_readonly(*ai.key, ai.is_signer)
            }
        })
        .collect()
}

fn transfer_lamports(from: &AccountInfo, to: &AccountInfo, amount: u64) -> Result<()> {
    let from_lamports = **from.lamports.borrow();
    require!(from_lamports >= amount, RafferoError::EmptyPrize);

    **from.try_borrow_mut_lamports()? -= amount;
    **to.try_borrow_mut_lamports()? += amount;
    Ok(())
}