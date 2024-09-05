use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, MintTo, Token, TokenAccount};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod capx_sol_liq {
    use super::*;

    // Function to initialize a new project for the first time
    pub fn init_project(ctx : Context<InitProject>, _name : String, _desc : String) -> Result<()> {
        
        // Fetch the project account to store project details
        let project_account = &mut ctx.accounts.project_account;

        // If the caller do not own any token they cannot initialize it
        require!(ctx.accounts.base_ata.amount > 0,CustomError::DoesNotOwnTokens);
        
        // Hard check on IPFS string
        require!(_desc.chars().count() == 46, CustomError::IPFSLengthMismatch);

        // project account being updated with the details
        project_account.tokenkey = ctx.accounts.basemint.to_account_info().key();
        project_account.projectname = _name;
        project_account.projectdesc = _desc;
        project_account.creator = ctx.accounts.user.to_account_info().key();
        project_account.decimal = ctx.accounts.basemint.decimals;
        project_account.bump = *ctx.bumps.get("project_account").unwrap();

        // emitting the details using event
        emit!(TokenRegEvent {
            tokenowner : project_account.creator,
            tokenmint : project_account.tokenkey,
            name : project_account.projectname.to_string(),
            desc : project_account.projectdesc.to_string(),
            tokendecimal : ctx.accounts.basemint.decimals,
            label: "tokendata".to_string()
        });

        Ok(())
    }

    // Function to initialize a derivative of an initialized project
    pub fn initialize_derivative(ctx: Context<InitializeDerivative>, _timestamp : String) -> Result<()> {
        
        // Fetching data and creating data account which will have mint authority
        let data_account = &mut ctx.accounts.data_account;
        let project_account = &mut ctx.accounts.project_account;

        // Checking if the token is registered with us or not
        require!(project_account.tokenkey == ctx.accounts.basemint.to_account_info().key(), CustomError::TokenNotReg);

        // Fetching mint address
        let mint = ctx.accounts.mint.clone();

        // Time stamp string converted to u64
        let mut date_ts : u64 = (_timestamp.parse::<u64>()).expect("Mismatch Panic");

        // Normalised time stamp
        date_ts = (date_ts/86400)*86400;

        // Convering timestamp to string
        let tbound = date_ts.to_string();

        // Checking if valid timestamp is provided or not
        require!(_timestamp==tbound,CustomError::TimestampMismatch);

        // Adding data to data account
        data_account.mintkey = mint.key();
        data_account.bump = *ctx.bumps.get("data_account").unwrap();
        data_account.tokenbump = *ctx.bumps.get("mint").unwrap();



        //propietario tiene que ver el saldo de sus properties , staking 

        //retirarlo a un exchange , retirar por amount 


        // Emitting event
        emit!(DerivativeRegEvent {
            basetoken : ctx.accounts.basemint.to_account_info().key(),
            timestamp : _timestamp,
            derivativetoken : data_account.mintkey,
            derivativeinitializer : ctx.accounts.user.to_account_info().key(),
            label: "derivativeNew".to_string()
        });

        Ok(())
    }

    // Function to mint derivatives of an initialized project
    pub fn lock_project_tokens(ctx: Context<TokenLock>, _timestamp : String, _vault_bump : u8,_amount: u64) -> Result<()> {

        let now_ts = Clock::get().unwrap().unix_timestamp as u64;  
        let mut date_ts : u64 = (_timestamp.parse::<u64>()).expect("Mismatch Panic");
        // Normalised time stamp
        date_ts = (date_ts/86400)*86400;
        
        // Convering timestamp to string
        let tbound = date_ts.to_string();

        // Checking if valid timestamp is provided or not
        require!(_timestamp==tbound,CustomError::TimestampMismatch);

        // To be activated in production to prevent vesting of past tokens
        // require!(date_ts > now_ts, CustomError::CannotVestInPast);

        // Fetching data account
        let data_account = &mut ctx.accounts.data_account;
        
        // Token transfer instruction
        let transfer_instruction = anchor_spl::token::Transfer {
            from: ctx.accounts.base_ata.to_account_info(),
            to: ctx.accounts.vest_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            transfer_instruction,
        );

        // Transfer Project token to program owned account
        anchor_spl::token::transfer(cpi_ctx, _amount)?;


        let _bump = data_account.bump;

        let bump_vector = _bump.to_le_bytes();
        let inner = vec![b"mint-data".as_ref(), _timestamp.as_ref(),ctx.accounts.basemint.to_account_info().key.as_ref(),bump_vector.as_ref()];
        let outer = vec![inner.as_slice()];

        let cpi_accounts = MintTo {
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.der_ata.to_account_info(),
            authority: data_account.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx2 = CpiContext::new_with_signer(cpi_program, cpi_accounts
            , outer.as_slice());
        
        // Mint Derivative to the der_ata owner passed to us
        token::mint_to(cpi_ctx2, _amount)?;

        // Emit Token Mint Event
        emit!(TokenMintEvent {
            basetoken: ctx.accounts.basemint.to_account_info().key(),
            amount: _amount,
            derivativetoken: ctx.accounts.mint.to_account_info().key(),
            receiver: ctx.accounts.der_ata.owner,
            label: "tokenmint".to_string()
        });


        Ok(())
    }

    // Function to burn derivatives of an initialized project
    pub fn unlock_project_tokens(ctx: Context<TokenUnlock>,_timestamp : String, _vault_bump : u8, _amount: u64) -> Result<()> {

        let now_ts = Clock::get().unwrap().unix_timestamp as u64;  
        let mut date_ts : u64 = (_timestamp.parse::<u64>()).expect("Mismatch Panic");
        
        // Normalised time stamp
        date_ts = (date_ts/86400)*86400;
        
        // Convering timestamp to string
        let tbound = date_ts.to_string();

        // Checking if valid timestamp is provided or not
        require!(_timestamp==tbound,CustomError::TimestampMismatch);

        
        // Condition to be activated in prod
        require!(now_ts > date_ts, CustomError::VestTimeNotEnded);

        let data_account = &mut ctx.accounts.data_account;


        let transfer_instruction = anchor_spl::token::Transfer {
            from: ctx.accounts.vest_account.to_account_info(),
            to: ctx.accounts.base_ata.to_account_info(),
            authority: ctx.accounts.vest_account.to_account_info(),
        };

        let bump_vector = _vault_bump.to_le_bytes();
        let inner = vec![b"mint-vault".as_ref(),_timestamp.as_ref(),ctx.accounts.basemint.to_account_info().key.as_ref(), bump_vector.as_ref()];
        let outer = vec![inner.as_slice()];
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            transfer_instruction,
            outer.as_slice(),
        );
        
        
        
        let _bump = data_account.bump;
        
        let bump_vector_burn = _bump.to_le_bytes();
        let inner_burn = vec![b"mint-data".as_ref(), _timestamp.as_ref(),ctx.accounts.basemint.to_account_info().key.as_ref(),bump_vector_burn.as_ref()];
        let outer_burn = vec![inner_burn.as_slice()];
        
        let cpi_accounts = Burn {
            mint: ctx.accounts.mint.to_account_info(),
            from: ctx.accounts.der_ata.to_account_info(),
            authority: data_account.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx_burn = CpiContext::new_with_signer(cpi_program, cpi_accounts
            , outer_burn.as_slice());
            
        token::burn(cpi_ctx_burn, _amount)?;
        anchor_spl::token::transfer(cpi_ctx, _amount)?;
            
        emit!(TokenBurnEvent {
            basetoken: ctx.accounts.basemint.to_account_info().key(),
            amount: _amount,
            derivativetoken: ctx.accounts.mint.to_account_info().key(),
            receiver: ctx.accounts.der_ata.owner,
            label: "tokenburn".to_string()
        });

        Ok(())
    }


}

// Custom errors
#[error_code]
pub enum CustomError {
    DoesNotOwnTokens,
    TimestampMismatch,
    VestTimeNotEnded,
    CannotVestInPast,
    IPFSLengthMismatch,
    TokenNotReg
}

#[derive(Accounts)]
pub struct InitProject<'info> {
    
    // caller of the transaction
    #[account(mut)]
    pub user: Signer<'info>,

    // project token which is being registered
    #[account(mut)]
    pub basemint: Account<'info, Mint>,

    // Associated token account of the project token owned by caller
    #[account(mut, constraint = base_ata.mint ==  basemint.key(), constraint = base_ata.owner == user.key())]
    pub base_ata: Account<'info, TokenAccount>,

    // Initializing a PDA to store data of the registered token
    #[account(    
        init,
        payer = user,
        space = 8 + 32 + (4 + 12) + (4 + 46) + 32 + 1 + 1,
        seeds = [b"project-data".as_ref(),basemint.key().as_ref()],
        bump
    )]
    pub project_account: Box<Account<'info, ProjectAccount>>,

    pub token_program: Program<'info, Token>,
    
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>

}

#[derive(Accounts)]
#[instruction(_timestamp : String)]
pub struct InitializeDerivative<'info> {

    // project token whose basemint is registered
    #[account(mut)]
    pub basemint: Account<'info, Mint>,

    // Adding mint data to data account
    #[account(
        init,
        payer = user,
        space = 8 + 32 + 1 + 1, seeds = [b"mint-data".as_ref(),_timestamp.as_ref(),basemint.key().as_ref()], bump
    )]
    pub data_account: Box<Account<'info, MintdAccount>>,

    // Vault which will hold basemint tokens for this derivative
    #[account(
        init,
        payer = user,
        seeds = [b"mint-vault".as_ref(),_timestamp.as_ref(),basemint.key().as_ref()],
        bump,
        token::mint = basemint,
        token::authority = vest_account,
    )]
    pub vest_account: Box<Account<'info, TokenAccount>>,

    // Project Account to fetch project data
    #[account(
        mut,
        seeds = [b"project-data".as_ref(),basemint.key().as_ref()],
        bump=project_account.bump
    )]
    pub project_account: Box<Account<'info, ProjectAccount>>,

    #[account(mut)]
    pub user: Signer<'info>,

    // New mint token address
    #[account(
        init,
        payer = user,
        seeds = [b"mint-token".as_ref(),_timestamp.as_ref(),basemint.key().as_ref()],
        bump,
        mint::decimals = basemint.decimals,
        mint::authority = data_account
    )]
    pub mint: Account<'info, Mint>,
    
    pub token_program: Program<'info, Token>,
    
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>
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

#[account]
#[derive(Default)]
pub struct ProjectAccount {
    tokenkey: Pubkey,
    projectname : String,
    projectdesc : String,
    creator : Pubkey,
    decimal : u8,
    bump : u8
}

#[account]
#[derive(Default)]
pub struct MintdAccount {
    mintkey: Pubkey,
    tokenbump : u8,
    bump : u8
}

#[event]
pub struct DerivativeRegEvent {
    pub basetoken: Pubkey,
    pub timestamp: String,
    pub derivativetoken: Pubkey,
    pub derivativeinitializer: Pubkey,
    #[index]
    pub label: String,
}

#[event]
pub struct TokenRegEvent {
    pub tokenowner: Pubkey,
    pub tokenmint: Pubkey,
    pub name: String,
    pub desc: String,
    pub tokendecimal : u8,
    #[index]
    pub label: String,
}

#[event]
pub struct TokenMintEvent {
    pub basetoken: Pubkey,
    pub amount: u64,
    pub derivativetoken: Pubkey,
    pub receiver: Pubkey,
    #[index]
    pub label: String,
}

#[event]
pub struct TokenBurnEvent {
    pub basetoken: Pubkey,
    pub amount: u64,
    pub derivativetoken: Pubkey,
    pub receiver: Pubkey,
    #[index]
    pub label: String,
}