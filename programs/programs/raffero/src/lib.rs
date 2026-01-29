use anchor_lang::prelude::*;

declare_id!("E1UPv6KVgDWzmphNNvyKt9bRkPZ7qS7BbS7A8UnwpPFk");

#[program]
pub mod raffero {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
