use soroban_sdk::{contracttype, unwrap::UnwrapOptimized, Env};

use crate::Config;

const ONE_DAY_LEDGERS: u32 = 17_280;
const TTL_THRESHOLD: u32 = 100 * ONE_DAY_LEDGERS;
const TTL_BUMP: u32 = 120 * ONE_DAY_LEDGERS;

#[contracttype]
enum Key {
    Config,
}

pub fn extend(e: &Env) {
    e.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_BUMP);
}

pub fn get_config(e: &Env) -> Config {
    e.storage().instance().get(&Key::Config).unwrap_optimized()
}

pub fn set_config(e: &Env, config: &Config) {
    e.storage().instance().set(&Key::Config, config);
}
