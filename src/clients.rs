use soroban_sdk::{contractclient, Address, Env, Vec};

use crate::BackstopTier;

#[allow(dead_code)]
#[contractclient(name = "ManagerClient")]
pub trait Manager {
    fn harvestable(e: Env, token: Address) -> i128;
    fn backstops(e: Env) -> Vec<Address>;
    fn pools(e: Env) -> Vec<Address>;
    fn c_join_pool(
        e: Env,
        from: Address,
        backstop: Address,
        tier: BackstopTier,
        pool_address: Address,
        pool_amount_out: i128,
        max_amounts_in: Vec<i128>,
    );
    fn b_deposit(
        e: Env,
        from: Address,
        backstop: Address,
        tier: BackstopTier,
        pool_address: Address,
        amount: i128,
    ) -> i128;
    fn b_claim(
        e: Env,
        from: Address,
        backstop: Address,
        tier: BackstopTier,
        pool_address: Address,
        min_lp_tokens_out: i128,
    ) -> i128;
}

#[allow(dead_code)]
#[contractclient(name = "BackstopClient")]
pub trait Backstop {
    fn backstop_token(e: Env, tier: BackstopTier, pool: Address) -> Address;
}

#[allow(dead_code)]
#[contractclient(name = "CometClient")]
pub trait Comet {
    fn get_balance(e: Env, token: Address) -> i128;
    fn get_total_supply(e: Env) -> i128;
    fn get_tokens(e: Env) -> Vec<Address>;
}
