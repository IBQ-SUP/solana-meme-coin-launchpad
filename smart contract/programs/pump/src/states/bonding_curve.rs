use anchor_lang::prelude::*;
use anchor_spl::token::Mint;

use crate::errors::PumpError;
use crate::consts::PRICE_PRECISION;
use crate::utils::{sol_transfer_from_user, sol_transfer_with_signer, token_transfer_user, token_transfer_with_signer};

#[account]
pub struct BondingCurve {
    //  vitual balances on the curve
    pub virtual_token_reserves: u64,
    pub virtual_sol_reserves: u64,

    //  real balances on the curve
    pub real_token_reserves: u64,
    pub real_sol_reserves: u64,

    //  token supply
    pub token_total_supply: u64,

    //  true - if the curve reached the limit
    pub is_completed: bool,
}

impl<'info> BondingCurve {
    pub const SEED_PREFIX: &'static str = "bonding-curve";
    pub const LEN: usize = 8 * 5 + 1;

    //  get signer for bonding curve PDA
    pub fn get_signer<'a>(mint: &'a Pubkey, bump: &'a u8) -> [&'a [u8]; 3] {
        [
            Self::SEED_PREFIX.as_bytes(),
            mint.as_ref(),
            std::slice::from_ref(bump),
        ]
    }

    //  update reserve balance on the curve PDA
    pub fn update_reserves(&mut self, reserve_lamport: u64, reserve_token: u64) -> Result<bool> {
        self.virtual_sol_reserves = reserve_lamport;
        self.virtual_token_reserves = reserve_token;

        Ok(false)
    }

    //  get current token price in lamports
    pub fn get_current_price(&self) -> Result<u64> {
        if self.virtual_token_reserves == 0 {
            return Ok(0);
        }
        
        let price = self.virtual_sol_reserves
            .checked_mul(PRICE_PRECISION) //  multiply by 1M for precision
            .ok_or(PumpError::OverflowOrUnderflowOccurred)?
            .checked_div(self.virtual_token_reserves)
            .ok_or(PumpError::OverflowOrUnderflowOccurred)?;
            
        Ok(price)
    }

    //  get price impact for a given amount
    pub fn get_price_impact(&self, amount_in: u64, direction: u8) -> Result<f64> {
        if self.virtual_sol_reserves == 0 || self.virtual_token_reserves == 0 {
            return Ok(0.0);
        }
        
        let impact = if direction == 0 {
            //  buy: impact on SOL reserves
            (amount_in as f64 / self.virtual_sol_reserves as f64) * 100.0
        } else {
            //  sell: impact on token reserves  
            (amount_in as f64 / self.virtual_token_reserves as f64) * 100.0
        };
        
        Ok(impact)
    }

    //  swap sol for token
    pub fn buy(
        &mut self,
        token_mint: &Account<'info, Mint>, //  token mint address
        curve_limit: u64,                  //  bonding curve limit
        user: &Signer<'info>,              //  user address

        curve_pda: &mut AccountInfo<'info>,     //  bonding curve PDA
        fee_recipient: &mut AccountInfo<'info>, //  team wallet address to get fee

        user_ata: &mut AccountInfo<'info>, //  associated toke accounts for user
        curve_ata: &mut AccountInfo<'info>, //  associated toke accounts for curve

        amount_in: u64,      //  sol amount to pay
        min_amount_out: u64, //  minimum amount out
        fee_percent: f64,    //  buy fee

        curve_bump: u8, // bump for signer

        system_program: &AccountInfo<'info>, //  system program
        token_program: &AccountInfo<'info>,  //  token program
    ) -> Result<bool> {
        //  validate input parameters
        require!(amount_in > 0, PumpError::IncorrectValue);
        require!(fee_percent >= 0.0 && fee_percent <= 100.0, PumpError::IncorrectValue);
        
        let (amount_out, fee_lamports) =
            self.calc_amount_out(amount_in, token_mint.decimals, 0, fee_percent)?;

        //  check min amount out
        require!(
            amount_out >= min_amount_out,
            PumpError::ReturnAmountTooSmall
        );

        //  check if curve has enough tokens
        require!(
            amount_out <= self.real_token_reserves,
            PumpError::IncorrectValue
        );

        //  transfer fee to team wallet
        sol_transfer_from_user(&user, fee_recipient, system_program, fee_lamports)?;
        //  transfer adjusted amount to curve
        sol_transfer_from_user(&user, curve_pda, system_program, amount_in - fee_lamports)?;
        //  transfer token from PDA to user
        token_transfer_with_signer(
            curve_ata,
            curve_pda,
            user_ata,
            token_program,
            &[&BondingCurve::get_signer(&token_mint.key(), &curve_bump)],
            amount_out,
        )?;

        //  calculate new reserves
        let new_token_reserves = self
            .virtual_token_reserves
            .checked_sub(amount_out)
            .ok_or(PumpError::OverflowOrUnderflowOccurred)?;

        let new_sol_reserves = self
            .virtual_sol_reserves
            .checked_add(amount_in - fee_lamports)
            .ok_or(PumpError::OverflowOrUnderflowOccurred)?;

        //  update real reserves
        self.real_token_reserves = self.real_token_reserves
            .checked_sub(amount_out)
            .ok_or(PumpError::OverflowOrUnderflowOccurred)?;

        msg! {"Reserves:: Token: {:?} SOL: {:?}", new_token_reserves, new_sol_reserves};

        //  update reserves on the curve
        self.update_reserves(new_sol_reserves, new_token_reserves)?;

        //  return true if the curve reached the limit
        if new_sol_reserves >= curve_limit {
            self.is_completed = true;
            return Ok(true);
        }

        //  return false, curve is not reached the limit
        Ok(false)
    }

    //  swap token for sol
    pub fn sell(
        &mut self,
        token_mint: &Account<'info, Mint>, //  token mint address
        user: &Signer<'info>,              //  user address

        curve_pda: &mut AccountInfo<'info>, //  bonding curve PDA
        fee_recipient: &mut AccountInfo<'info>, //  team wallet address to get fee

        user_ata: &mut AccountInfo<'info>, //  associated toke accounts for user
        curve_ata: &mut AccountInfo<'info>, //  associated toke accounts for curve

        amount_in: u64,      //  token amount to pay
        min_amount_out: u64, //  minimum amount out
        fee_percent: f64,    //  sell fee

        curve_bump: u8, // bump for signer
        
        system_program: &AccountInfo<'info>, //  system program
        token_program: &AccountInfo<'info>,  //  token program
    ) -> Result<()> {
        //  validate input parameters
        require!(amount_in > 0, PumpError::IncorrectValue);
        require!(fee_percent >= 0.0 && fee_percent <= 100.0, PumpError::IncorrectValue);
        
        let (amount_out, fee_lamports) =
            self.calc_amount_out(amount_in, token_mint.decimals, 1, fee_percent)?;

        //  check min amount out
        require!(
            amount_out >= min_amount_out,
            PumpError::ReturnAmountTooSmall
        );

        //  check if curve has enough SOL
        require!(
            amount_out + fee_lamports <= self.real_sol_reserves,
            PumpError::IncorrectValue
        );

        let token = token_mint.key();
        let signer_seeds: &[&[&[u8]]] = &[&BondingCurve::get_signer(&token, &curve_bump)];
        //  transfer fee to team wallet
        sol_transfer_with_signer(
            &user,
            fee_recipient,
            system_program,
            signer_seeds,
            fee_lamports,
        )?;
        //  transfer SOL to curve PDA
        sol_transfer_with_signer(
            &user,
            curve_pda,
            &system_program,
            signer_seeds,
            amount_in - fee_lamports,
        )?;
        //  transfer token from user to PDA
        token_transfer_user(
            user_ata,
            user,
            curve_ata,
            token_program,
            amount_out,
        )?;

        //  calculate new reserves
        let new_token_reserves = self
            .virtual_token_reserves
            .checked_add(amount_in)
            .ok_or(PumpError::OverflowOrUnderflowOccurred)?;

        let new_sol_reserves = self
            .virtual_sol_reserves
            .checked_sub(amount_out + fee_lamports)
            .ok_or(PumpError::OverflowOrUnderflowOccurred)?;

        //  update real reserves
        self.real_token_reserves = self.real_token_reserves
            .checked_add(amount_in)
            .ok_or(PumpError::OverflowOrUnderflowOccurred)?;
        
        self.real_sol_reserves = self.real_sol_reserves
            .checked_sub(amount_out + fee_lamports)
            .ok_or(PumpError::OverflowOrUnderflowOccurred)?;

        msg! {"Reserves:: Token: {:?} SOL: {:?}", new_token_reserves, new_sol_reserves};

        //  update reserves on the curve
        self.update_reserves(new_sol_reserves, new_token_reserves)?;

        Ok(())
    }

    //  calculate amount out and fee lamports
    fn calc_amount_out(
        &mut self,
        amount_in: u64,
        token_decimal: u8, //  decimal for token
        direction: u8,     //  0 - buy, 1 - sell
        fee_percent: f64,
    ) -> Result<(u64, u64)> {
        //  implement bonding curve formula using constant product AMM
        //  k = virtual_token_reserves * virtual_sol_reserves
        
        let k = self.virtual_token_reserves
            .checked_mul(self.virtual_sol_reserves)
            .ok_or(PumpError::OverflowOrUnderflowOccurred)?;
        
        let fee_lamports = (amount_in as f64 * fee_percent / 100.0) as u64;
        let adjusted_amount_in = amount_in.checked_sub(fee_lamports)
            .ok_or(PumpError::OverflowOrUnderflowOccurred)?;
        
        let amount_out = if direction == 0 {
            //  buy: swap SOL for tokens
            //  new_sol_reserves = virtual_sol_reserves + adjusted_amount_in
            //  new_token_reserves = k / new_sol_reserves
            //  amount_out = virtual_token_reserves - new_token_reserves
            let new_sol_reserves = self.virtual_sol_reserves
                .checked_add(adjusted_amount_in)
                .ok_or(PumpError::OverflowOrUnderflowOccurred)?;
            
            let new_token_reserves = k.checked_div(new_sol_reserves)
                .ok_or(PumpError::OverflowOrUnderflowOccurred)?;
            
            self.virtual_token_reserves
                .checked_sub(new_token_reserves)
                .ok_or(PumpError::OverflowOrUnderflowOccurred)?
        } else {
            //  sell: swap tokens for SOL
            //  new_token_reserves = virtual_token_reserves + amount_in
            //  new_sol_reserves = k / new_token_reserves
            //  amount_out = virtual_sol_reserves - new_sol_reserves
            let new_token_reserves = self.virtual_token_reserves
                .checked_add(amount_in)
                .ok_or(PumpError::OverflowOrUnderflowOccurred)?;
            
            let new_sol_reserves = k.checked_div(new_token_reserves)
                .ok_or(PumpError::OverflowOrUnderflowOccurred)?;
            
            self.virtual_sol_reserves
                .checked_sub(new_sol_reserves)
                .ok_or(PumpError::OverflowOrUnderflowOccurred)?
        };

        Ok((amount_out, fee_lamports))
    }

    //  estimate amount out for a given input (without fees)
    pub fn estimate_amount_out(&self, amount_in: u64, direction: u8) -> Result<u64> {
        if self.virtual_sol_reserves == 0 || self.virtual_token_reserves == 0 {
            return Ok(0);
        }
        
        let k = self.virtual_token_reserves
            .checked_mul(self.virtual_sol_reserves)
            .ok_or(PumpError::OverflowOrUnderflowOccurred)?;
        
        let amount_out = if direction == 0 {
            //  buy: estimate tokens received for SOL
            let new_sol_reserves = self.virtual_sol_reserves
                .checked_add(amount_in)
                .ok_or(PumpError::OverflowOrUnderflowOccurred)?;
            
            let new_token_reserves = k.checked_div(new_sol_reserves)
                .ok_or(PumpError::OverflowOrUnderflowOccurred)?;
            
            self.virtual_token_reserves
                .checked_sub(new_token_reserves)
                .ok_or(PumpError::OverflowOrUnderflowOccurred)?
        } else {
            //  sell: estimate SOL received for tokens
            let new_token_reserves = self.virtual_token_reserves
                .checked_add(amount_in)
                .ok_or(PumpError::OverflowOrUnderflowOccurred)?;
            
            let new_sol_reserves = k.checked_div(new_token_reserves)
                .ok_or(PumpError::OverflowOrUnderflowOccurred)?;
            
            self.virtual_sol_reserves
                .checked_sub(new_sol_reserves)
                .ok_or(PumpError::OverflowOrUnderflowOccurred)?
        };
        
        Ok(amount_out)
    }
}
