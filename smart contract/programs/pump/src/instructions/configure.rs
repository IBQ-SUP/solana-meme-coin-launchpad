use crate::{errors::PumpError, states::Config, consts::{MAX_FEE_PERCENT, MIN_CURVE_LIMIT, MAX_CURVE_LIMIT}};
use anchor_lang::{prelude::*, system_program};

#[derive(Accounts)]
pub struct Configure<'info> {
    #[account(mut)]
    admin: Signer<'info>,

    #[account(
        init_if_needed,
        payer = admin,
        seeds = [Config::SEED_PREFIX.as_bytes()],
        space = 8 + Config::LEN,
        bump,
    )]
    global_config: Account<'info, Config>,

    #[account(address = system_program::ID)]
    system_program: Program<'info, System>,
}

impl<'info> Configure<'info> {
    pub fn process(&mut self, new_config: Config) -> Result<()> {
        require!(self.global_config.authority.eq(&Pubkey::default())
            || self.global_config.authority.eq(&self.admin.key()), PumpError::NotAuthorized);

        //  validate configuration parameters
        require!(
            new_config.buy_fee_percent >= 0.0 && new_config.buy_fee_percent <= MAX_FEE_PERCENT,
            PumpError::IncorrectValue
        );
        require!(
            new_config.sell_fee_percent >= 0.0 && new_config.sell_fee_percent <= MAX_FEE_PERCENT,
            PumpError::IncorrectValue
        );
        require!(
            new_config.migration_fee_percent >= 0.0 && new_config.migration_fee_percent <= MAX_FEE_PERCENT,
            PumpError::IncorrectValue
        );
        require!(
            new_config.curve_limit >= MIN_CURVE_LIMIT && new_config.curve_limit <= MAX_CURVE_LIMIT,
            PumpError::IncorrectValue
        );
        require!(
            new_config.initial_virtual_token_reserves > 0,
            PumpError::IncorrectValue
        );
        require!(
            new_config.initial_virtual_sol_reserves > 0,
            PumpError::IncorrectValue
        );
        require!(
            new_config.total_token_supply > 0,
            PumpError::IncorrectValue
        );

        self.global_config.set_inner(new_config);

        Ok(())
    }
}
