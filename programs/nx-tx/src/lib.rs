use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, MintTo, Token,  Transfer, TokenAccount};

// This is your program's public key and it will update
// automatically when you build the project.
declare_id!("2Dnk29pNbq4PpfELXr7okQ7JeMieRQRjY3vpU7NRXPHa");



#[program]
pub mod reserva_property {
    use super::*;

    pub fn create_reservation(
        ctx: Context<CreateReservation>,
        amount: u64,
        check_in_date: i64,
    ) -> Result<()> {
        let reservation = &mut ctx.accounts.reservation;
        reservation.user = ctx.accounts.user.key();
        reservation.amount = amount;
        reservation.check_in_date = check_in_date;
        reservation.is_paid = false;

        // Colocar los fondos en staking
        admin::stake_tokens(
            ctx.accounts.user_staking_account.to_account_info(),
            ctx.accounts.admin_staking_account.to_account_info(),
            ctx.accounts.user.to_account_info(),
            amount,
            ctx.accounts.token_program.to_account_info(),
        )?;

        Ok(())
    }

    pub fn check_in(ctx: Context<CheckIn>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        // Permitir check-in en las siguientes 48 horas
        require!(
            now >= ctx.accounts.reservation.check_in_date
                && now <= ctx.accounts.reservation.check_in_date + 172800, // 48 horas
            ErrorCode::InvalidCheckInDate
        );

        let reservation = &mut ctx.accounts.reservation;
        require!(!reservation.is_paid, ErrorCode::AlreadyPaid);

        // Transferir fondos de la cuenta de staking del administrador a la cuenta del propietario
        admin::transfer_from_staking(
            ctx.accounts.admin_staking_account.to_account_info(),
            ctx.accounts.owner_account.to_account_info(),
            ctx.accounts.admin_authority.to_account_info(),
            reservation.amount,
            ctx.accounts.token_program.to_account_info(),
        )?;

        // Actualizar el estado de la reserva
        reservation.is_paid = true;
        Ok(())
    }
}


#[derive(Accounts)]
pub struct CreateReservation<'info> {
    #[account(init, payer = user, space = 8 + 32 + 8 + 8 + 1)]
    pub reservation: Account<'info, Reservation>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_staking_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub admin_staking_account: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}


#[derive(Accounts)]
pub struct CheckIn<'info> {
    #[account(mut, has_one = user)]
    pub reservation: Account<'info, Reservation>,
    pub user: Signer<'info>,
    #[account(mut)]
    pub admin_staking_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub owner_account: Account<'info, TokenAccount>,
    pub admin_authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[account]
pub struct Reservation {
    pub user: Pubkey,
    pub amount: u64,
    pub check_in_date: i64,
    pub is_paid: bool,
}

#[error_code]
pub enum ErrorCode {
    #[msg("La fecha de check-in no es válida.")]
    InvalidCheckInDate,
    #[msg("La reserva ya ha sido pagada.")]
    AlreadyPaid,
}




pub mod admin {
    use anchor_lang::prelude::*;
    use anchor_spl::token::{self, TokenAccount, Transfer, Token};
    use super::*;

    // Function to lock project tokens and mint derivatives
    pub fn lock_project_tokens(
        ctx: Context<TokenLock>,
        timestamp: String,
        vault_bump: u8,
        amount: u64
    ) -> Result<()> {
        // Validate timestamp
        let now_ts = Clock::get()?.unix_timestamp as u64;
        let date_ts = normalize_timestamp(&timestamp)?;
        
        // Uncomment for production to prevent locking tokens in the past
        // require!(date_ts > now_ts, CustomError::CannotLockInPast);

        // Transfer project tokens to vault
        transfer_to_vault(&ctx, amount)?;

        // Mint derivative tokens
        mint_derivative_tokens(&ctx, &timestamp, amount)?;

        // Emit Token Lock Event
        emit!(TokenLockEvent {
            basetoken: ctx.accounts.basemint.key(),
            amount,
            derivativetoken: ctx.accounts.mint.key(),
            receiver: ctx.accounts.der_ata.owner,
            timestamp: date_ts.to_string(),
            label: "tokenlock".to_string()
        });

        Ok(())
    }

    // Function to burn derivatives and unlock project tokens
    pub fn unlock_project_tokens(
        ctx: Context<TokenUnlock>,
        timestamp: String,
        vault_bump: u8,
        amount: u64
    ) -> Result<()> {
        // Validate timestamp and check if unlock time has passed
        let now_ts = Clock::get()?.unix_timestamp as u64;
        let date_ts = normalize_timestamp(&timestamp)?;
        require!(now_ts > date_ts, CustomError::UnlockTimeNotReached);

        // Burn derivative tokens
        burn_derivative_tokens(&ctx, &timestamp, amount)?;

        // Transfer project tokens from vault to user
        transfer_from_vault(&ctx, &timestamp, vault_bump, amount)?;

        // Emit Token Unlock Event
        emit!(TokenUnlockEvent {
            basetoken: ctx.accounts.basemint.key(),
            amount,
            derivativetoken: ctx.accounts.mint.key(),
            receiver: ctx.accounts.base_ata.owner,
            timestamp: date_ts.to_string(),
            label: "tokenunlock".to_string()
        });

        Ok(())
    }


    
    pub fn stake_tokens(
        user_staking_account: AccountInfo<'info>,
        admin_staking_account: AccountInfo<'info>,
        user: AccountInfo<'info>,
        amount: u64,
        token_program: AccountInfo<'info>,
    ) -> Result<()> {
        // Transferir tokens del usuario a la cuenta de staking del administrador
        let cpi_accounts = Transfer {
            from: user_staking_account.clone(),
            to: admin_staking_account.clone(),
            authority: user.clone(),
        };
        let cpi_program = token_program.clone();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;

        // Colocar los tokens en staking
        // Implementación de la lógica de staking aquí...

        Ok(())
    }

    pub fn transfer_from_staking(
        from: AccountInfo<'info>,
        to: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        amount: u64,
        token_program: AccountInfo<'info>,
    ) -> Result<()> {
        let cpi_accounts = Transfer {
            from: from.clone(),
            to: to.clone(),
            authority: authority.clone(),
        };
        let cpi_program = token_program.clone();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;
        Ok(())
    }




// Helper functions

fn normalize_timestamp(timestamp: &str) -> Result<u64> {
    let date_ts = timestamp.parse::<u64>().map_err(|_| CustomError::InvalidTimestamp)?;
    Ok((date_ts / 86400) * 86400)
}

fn transfer_to_vault<'info>(ctx: &Context<TokenLock<'info>>, amount: u64) -> Result<()> {
    let cpi_accounts = Transfer {
        from: ctx.accounts.base_ata.to_account_info(),
        to: ctx.accounts.vest_account.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
    );
    token::transfer(cpi_ctx, amount)
}

fn mint_derivative_tokens<'info>(
    ctx: &Context<TokenLock<'info>>,
    timestamp: &str,
    amount: u64
) -> Result<()> {
    let data_account = &ctx.accounts.data_account;
    let seeds = &[
        b"mint-data".as_ref(),
        timestamp.as_ref(),
        ctx.accounts.basemint.key().as_ref(),
        &[data_account.bump],
    ];
    let signer = &[&seeds[..]];

    let cpi_accounts = MintTo {
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.der_ata.to_account_info(),
        authority: data_account.to_account_info(),
    };
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        signer,
    );
    token::mint_to(cpi_ctx, amount)
}




fn burn_derivative_tokens<'info>(
    ctx: &Context<TokenUnlock<'info>>,
    timestamp: &str,
    amount: u64
) -> Result<()> {
    let data_account = &ctx.accounts.data_account;
    let seeds = &[
        b"mint-data".as_ref(),
        timestamp.as_ref(),
        ctx.accounts.basemint.key().as_ref(),
        &[data_account.bump],
    ];
    let signer = &[&seeds[..]];

    let cpi_accounts = Burn {
        mint: ctx.accounts.mint.to_account_info(),
        from: ctx.accounts.der_ata.to_account_info(),
        authority: data_account.to_account_info(),
    };
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        signer,
    );
    token::burn(cpi_ctx, amount)
}

fn transfer_from_vault<'info>(
    ctx: &Context<TokenUnlock<'info>>,
    timestamp: &str,
    vault_bump: u8,
    amount: u64
) -> Result<()> {
    let seeds = &[
        b"mint-vault".as_ref(),
        timestamp.as_ref(),
        ctx.accounts.basemint.key().as_ref(),
        &[vault_bump],
    ];
    let signer = &[&seeds[..]];

    let cpi_accounts = Transfer {
        from: ctx.accounts.vest_account.to_account_info(),
        to: ctx.accounts.base_ata.to_account_info(),
        authority: ctx.accounts.vest_account.to_account_info(),
    };
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        signer,
    );
    token::transfer(cpi_ctx, amount)
}




}

#[derive(Accounts)]
#[instruction(_timestamp : String, _vault_bump : u8)]
pub struct TokenLock<'info> {

    // project token
    #[account(mut)]
    pub basemint: Account<'info, Mint>,

    // ATA of project token owned by caller
    #[account(mut, constraint = base_ata.mint ==  basemint.key(), constraint = base_ata.owner == user.key())]
    pub base_ata: Account<'info, TokenAccount>,

    // Data account
    #[account(
        mut,
        seeds = [b"mint-data".as_ref(),_timestamp.as_ref(),basemint.key().as_ref()], bump=data_account.bump
    )]
    pub data_account: Box<Account<'info, MintdAccount>>,

    // Mint vault which holds the basemint tokens
    #[account(
        mut,
        seeds = [b"mint-vault".as_ref(),_timestamp.as_ref(),basemint.key().as_ref()],
        bump=_vault_bump
    )]
    pub vest_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub user: Signer<'info>,

    // Derivative 
    #[account(
        mut,
        seeds = [b"mint-token".as_ref(),_timestamp.as_ref(),basemint.key().as_ref()],
        bump = data_account.tokenbump,
        constraint = mint.key() == data_account.mintkey
    )]
    pub mint: Account<'info, Mint>,

    // Derivative ATA key is mint key
    #[account(mut, constraint = der_ata.mint ==  mint.key())]
    pub der_ata: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>
}

#[derive(Accounts)]
#[instruction(_timestamp : String, _vault_bump : u8)]
pub struct TokenUnlock<'info> {
    
    // project token
    #[account(mut)]
    pub basemint: Account<'info, Mint>,

    // ATA of project token owned by caller
    #[account(mut, constraint = base_ata.mint ==  basemint.key(), constraint = base_ata.owner == user.key())]
    pub base_ata: Account<'info, TokenAccount>,

    // Data account
    #[account(
        mut,
        seeds = [b"mint-data".as_ref(),_timestamp.as_ref(),basemint.key().as_ref()], bump=data_account.bump
    )]
    pub data_account: Box<Account<'info, MintdAccount>>,

    // Mint vault which holds the basemint tokens
    #[account(
        mut,
        seeds = [b"mint-vault".as_ref(),_timestamp.as_ref(),basemint.key().as_ref()],
        bump=_vault_bump
    )]
    pub vest_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub user: Signer<'info>,

    // Derivative 
    #[account(
        mut,
        seeds = [b"mint-token".as_ref(),_timestamp.as_ref(),basemint.key().as_ref()],
        bump = data_account.tokenbump,
        constraint = mint.key() == data_account.mintkey
    )]
    pub mint: Account<'info, Mint>,

    // Derivative ATA owned by caller
    #[account(mut, constraint = der_ata.mint ==  mint.key(), constraint = der_ata.owner == user.key())]
    pub der_ata: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>
}

// Event definitions
#[event]
pub struct TokenLockEvent {
    pub basetoken: Pubkey,
    pub amount: u64,
    pub derivativetoken: Pubkey,
    pub receiver: Pubkey,
    pub timestamp: String,
    #[index]
    pub label: String,
}

#[event]
pub struct TokenUnlockEvent {
    pub basetoken: Pubkey,
    pub amount: u64,
    pub derivativetoken: Pubkey,
    pub receiver: Pubkey,
    pub timestamp: String,
    #[index]
    pub label: String,
}

// Custom error definitions
#[error_code]
pub enum CustomError {
    #[msg("Invalid timestamp provided")]
    InvalidTimestamp,
    #[msg("Cannot lock tokens in the past")]
    CannotLockInPast,
    #[msg("Unlock time has not been reached yet")]
    UnlockTimeNotReached,
    // ... (otros errores existentes)
}
