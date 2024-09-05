use anchor_lang::prelude::*;

declare_id!("4Nh4rUvYQkqYvwUM6v5whBa976wqMn5J8Gbb2xc1zsv3");

#[program]
pub mod nx_tx {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
