use std::ops::{Div, Mul};

pub fn convert_to_float(value: u64, decimals: u8) -> f64 {
    (value as f64).div(f64::powf(10.0, decimals as f64))
}

pub fn convert_from_float(value: f64, decimals: u8) -> u64 {
    value.mul(f64::powf(10.0, decimals as f64)) as u64
}

pub fn calculate_price_impact(amount_in: u64, reserves: u64) -> f64 {
    if reserves == 0 {
        return 0.0;
    }
    (amount_in as f64 / reserves as f64) * 100.0
}

pub fn calculate_slippage(amount_in: u64, amount_out: u64, expected_amount: u64) -> f64 {
    if expected_amount == 0 {
        return 0.0;
    }
    let difference = if amount_out > expected_amount {
        amount_out - expected_amount
    } else {
        expected_amount - amount_out
    };
    (difference as f64 / expected_amount as f64) * 100.0
}
