use soroban_sdk::{contractevent, Env};

use crate::{ClaimResult, DepositAndClaimResult, DepositResult};

#[contractevent(topics = ["deposit"], data_format = "single-value")]
struct DepositEvent {
    result: DepositResult,
}

#[contractevent(topics = ["claim"])]
struct ClaimEvent {
    first_min_lp_out: i128,
    second_min_lp_out: i128,
    result: ClaimResult,
}

#[contractevent(topics = ["deposit_claim"], data_format = "single-value")]
struct DepositAndClaimEvent {
    result: DepositAndClaimResult,
}

pub fn deposit(e: &Env, result: DepositResult) {
    DepositEvent { result }.publish(e);
}

pub fn claim(e: &Env, first_min_lp_out: i128, second_min_lp_out: i128, result: ClaimResult) {
    ClaimEvent {
        first_min_lp_out,
        second_min_lp_out,
        result,
    }
    .publish(e);
}

pub fn deposit_and_claim(e: &Env, result: DepositAndClaimResult) {
    DepositAndClaimEvent { result }.publish(e);
}
