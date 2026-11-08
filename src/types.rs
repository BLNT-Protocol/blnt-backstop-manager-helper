use soroban_sdk::{contracttype, Address};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackstopTier {
    FirstLoss,
    SecondLoss,
    ThirdLoss,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub backstop: Address,
    pub blnt: Address,
    pub first_blnt_first: bool,
    pub first_lp: Address,
    pub manager: Address,
    pub owner: Address,
    pub pool: Address,
    pub second_blnt_first: bool,
    pub second_lp: Address,
    pub usdc: Address,
    pub xlm: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TierDepositResult {
    pub backstop_shares: i128,
    pub blnt_in: i128,
    pub lp_minted: i128,
    pub paired_in: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepositResult {
    pub first: TierDepositResult,
    pub second: TierDepositResult,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimResult {
    pub first_lp_out: i128,
    pub second_lp_out: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepositAndClaimResult {
    pub claim: ClaimResult,
    pub deposit: DepositResult,
}
