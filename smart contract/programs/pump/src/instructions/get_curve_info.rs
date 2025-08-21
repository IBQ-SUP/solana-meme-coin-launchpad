use crate::states::{BondingCurve, Config};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct GetCurveInfo<'info> {
    #[account(
        seeds = [Config::SEED_PREFIX.as_bytes()],
        bump,
    )]
    global_config: Box<Account<'info, Config>>,
    
    #[account(
        seeds = [BondingCurve::SEED_PREFIX.as_bytes(), &token_mint.key().to_bytes()],
        bump
    )]
    bonding_curve: Box<Account<'info, BondingCurve>>,
    
    /// CHECK: This is just for getting the mint key
    token_mint: UncheckedAccount<'info>,
}

impl<'info> GetCurveInfo<'info> {
    pub fn process(&self) -> Result<()> {
        let bonding_curve = &self.bonding_curve;
        let global_config = &self.global_config;
        
        //  log curve information for easy access
        msg!("Curve Info for token: {}", self.token_mint.key());
        msg!("Virtual Token Reserves: {}", bonding_curve.virtual_token_reserves);
        msg!("Virtual SOL Reserves: {}", bonding_curve.virtual_sol_reserves);
        msg!("Real Token Reserves: {}", bonding_curve.real_token_reserves);
        msg!("Real SOL Reserves: {}", bonding_curve.real_sol_reserves);
        msg!("Total Token Supply: {}", bonding_curve.token_total_supply);
        msg!("Is Completed: {}", bonding_curve.is_completed);
        msg!("Curve Limit: {}", global_config.curve_limit);
        msg!("Buy Fee: {}%", global_config.buy_fee_percent);
        msg!("Sell Fee: {}%", global_config.sell_fee_percent);
        
        //  calculate and log current price
        if let Ok(price) = bonding_curve.get_current_price() {
            msg!("Current Token Price: {} lamports", price);
        }
        
        Ok(())
    }
}
