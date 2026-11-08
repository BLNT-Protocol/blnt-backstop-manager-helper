use crate::{
    clients::{BackstopClient, CometClient, ManagerClient},
    events, storage,
    types::{
        BackstopTier, ClaimResult, Config, DepositAndClaimResult, DepositResult, TierDepositResult,
    },
    HelperError,
};
use soroban_sdk::{
    contract, contractimpl, panic_with_error, token::TokenClient, Address, Env, Executable, Vec,
    I256,
};

const COMET_SCALE: i128 = 1_000_000_000_000_000_000;
const BPS_SCALE: i128 = 10_000;
const PAIRED_SLIPPAGE_BPS: i128 = 100;

#[contract]
pub struct BlntBackstopManagerHelper;

struct JoinQuote {
    blnt: i128,
    lp: i128,
    max_pair: i128,
    pair: i128,
}

fn checked_add(e: &Env, left: i128, right: i128) -> i128 {
    left.checked_add(right)
        .unwrap_or_else(|| panic_with_error!(e, HelperError::Overflow))
}

fn checked_sub(e: &Env, left: i128, right: i128) -> i128 {
    left.checked_sub(right)
        .unwrap_or_else(|| panic_with_error!(e, HelperError::Overflow))
}

fn mul_div_floor(e: &Env, value: i128, numerator: i128, denominator: i128) -> i128 {
    if value < 0 || numerator < 0 || denominator <= 0 {
        panic_with_error!(e, HelperError::InvalidState);
    }
    I256::from_i128(e, value)
        .mul(&I256::from_i128(e, numerator))
        .div(&I256::from_i128(e, denominator))
        .to_i128()
        .unwrap_or_else(|| panic_with_error!(e, HelperError::Overflow))
}

fn mul_div_ceil(e: &Env, value: i128, numerator: i128, denominator: i128) -> i128 {
    if value < 0 || numerator < 0 || denominator <= 0 {
        panic_with_error!(e, HelperError::InvalidState);
    }
    let product = I256::from_i128(e, value).mul(&I256::from_i128(e, numerator));
    let divisor = I256::from_i128(e, denominator);
    let quotient = product.div(&divisor);
    let rounded = if product.rem_euclid(&divisor) == I256::from_i32(e, 0) {
        quotient
    } else {
        quotient
            .checked_add(&I256::from_i32(e, 1))
            .unwrap_or_else(|| panic_with_error!(e, HelperError::Overflow))
    };
    rounded
        .to_i128()
        .unwrap_or_else(|| panic_with_error!(e, HelperError::Overflow))
}

fn require_distinct(e: &Env, addresses: &Vec<Address>) {
    for left in 0..addresses.len() {
        for right in (left + 1)..addresses.len() {
            if addresses.get_unchecked(left) == addresses.get_unchecked(right) {
                panic_with_error!(e, HelperError::InvalidConfig);
            }
        }
    }
}

fn contains(values: &Vec<Address>, expected: &Address) -> bool {
    values.iter().any(|value| value == *expected)
}

fn pair_order(tokens: &Vec<Address>, blnt: &Address, paired: &Address) -> Option<bool> {
    if tokens.len() != 2 || !contains(tokens, blnt) || !contains(tokens, paired) {
        return None;
    }
    Some(tokens.get_unchecked(0) == *blnt)
}

fn require_positive_pool(e: &Env, lp: &Address, blnt: &Address, paired: &Address) {
    let comet = CometClient::new(e, lp);
    if comet.get_total_supply() <= 0
        || comet.get_balance(blnt) <= 0
        || comet.get_balance(paired) <= 0
    {
        panic_with_error!(e, HelperError::InvalidState);
    }
}

fn configured_owner(config: &Config) -> Address {
    let owner = config.owner.clone();
    owner.require_auth();
    owner
}

fn quote_join(
    e: &Env,
    config: &Config,
    lp: &Address,
    paired: &Address,
    blnt_budget: i128,
) -> JoinQuote {
    if blnt_budget <= 0 {
        panic_with_error!(e, HelperError::InvalidAmount);
    }
    let comet = CometClient::new(e, lp);
    let supply = comet.get_total_supply();
    let blnt_reserve = comet.get_balance(&config.blnt);
    let pair_reserve = comet.get_balance(paired);
    if supply <= 0 || blnt_reserve <= 0 || pair_reserve <= 0 {
        panic_with_error!(e, HelperError::InvalidState);
    }

    let max_ratio = mul_div_floor(e, blnt_budget, COMET_SCALE, blnt_reserve);
    let lp = mul_div_floor(e, max_ratio, supply, COMET_SCALE);
    if lp <= 0 {
        panic_with_error!(e, HelperError::InvalidAmount);
    }
    let join_ratio = mul_div_ceil(e, lp, COMET_SCALE, supply);
    let blnt = mul_div_ceil(e, blnt_reserve, join_ratio, COMET_SCALE);
    let pair = mul_div_ceil(e, pair_reserve, join_ratio, COMET_SCALE);
    if blnt <= 0 || blnt > blnt_budget || pair <= 0 {
        panic_with_error!(e, HelperError::InvalidState);
    }
    let max_pair = mul_div_ceil(e, pair, BPS_SCALE + PAIRED_SLIPPAGE_BPS, BPS_SCALE);
    JoinQuote {
        blnt,
        lp,
        max_pair,
        pair,
    }
}

fn join_and_deposit(
    e: &Env,
    config: &Config,
    owner: &Address,
    tier: BackstopTier,
    lp: &Address,
    blnt_first: bool,
    quote: &JoinQuote,
) -> TierDepositResult {
    let mut max_amounts = Vec::new(e);
    if blnt_first {
        max_amounts.push_back(quote.blnt);
        max_amounts.push_back(quote.max_pair);
    } else {
        max_amounts.push_back(quote.max_pair);
        max_amounts.push_back(quote.blnt);
    }

    let manager = ManagerClient::new(e, &config.manager);
    let lp_token = TokenClient::new(e, lp);
    let lp_before = lp_token.balance(&config.manager);
    manager.c_join_pool(
        owner,
        &config.backstop,
        &tier,
        &config.pool,
        &quote.lp,
        &max_amounts,
    );
    if lp_token.balance(&config.manager).checked_sub(lp_before) != Some(quote.lp) {
        panic_with_error!(e, HelperError::BalanceMismatch);
    }
    let backstop_shares =
        manager.b_deposit(owner, &config.backstop, &tier, &config.pool, &quote.lp);
    if backstop_shares <= 0 || lp_token.balance(&config.manager) != lp_before {
        panic_with_error!(e, HelperError::BalanceMismatch);
    }

    TierDepositResult {
        backstop_shares,
        blnt_in: quote.blnt,
        lp_minted: quote.lp,
        paired_in: quote.pair,
    }
}

fn execute_deposit(
    e: &Env,
    config: &Config,
    owner: &Address,
    first_blnt: i128,
    second_blnt: i128,
) -> DepositResult {
    let first_quote = quote_join(e, config, &config.first_lp, &config.xlm, first_blnt);
    let second_quote = quote_join(e, config, &config.second_lp, &config.usdc, second_blnt);
    let blnt = TokenClient::new(e, &config.blnt);
    let xlm = TokenClient::new(e, &config.xlm);
    let usdc = TokenClient::new(e, &config.usdc);
    let blnt_before = blnt.balance(&config.manager);
    let xlm_before = xlm.balance(&config.manager);
    let usdc_before = usdc.balance(&config.manager);
    let blnt_required = checked_add(e, first_quote.blnt, second_quote.blnt);
    if blnt_before < blnt_required
        || xlm_before < first_quote.pair
        || usdc_before < second_quote.pair
    {
        panic_with_error!(e, HelperError::InsufficientBalance);
    }

    let first = join_and_deposit(
        e,
        config,
        owner,
        BackstopTier::FirstLoss,
        &config.first_lp,
        config.first_blnt_first,
        &first_quote,
    );
    let second = join_and_deposit(
        e,
        config,
        owner,
        BackstopTier::SecondLoss,
        &config.second_lp,
        config.second_blnt_first,
        &second_quote,
    );

    if blnt_before.checked_sub(blnt.balance(&config.manager)) != Some(blnt_required)
        || xlm_before.checked_sub(xlm.balance(&config.manager)) != Some(first_quote.pair)
        || usdc_before.checked_sub(usdc.balance(&config.manager)) != Some(second_quote.pair)
    {
        panic_with_error!(e, HelperError::BalanceMismatch);
    }
    DepositResult { first, second }
}

fn execute_claim(
    e: &Env,
    config: &Config,
    owner: &Address,
    first_min_lp_out: i128,
    second_min_lp_out: i128,
) -> ClaimResult {
    if first_min_lp_out < 0 || second_min_lp_out < 0 {
        panic_with_error!(e, HelperError::InvalidAmount);
    }
    let manager = ManagerClient::new(e, &config.manager);
    let first_lp_out = manager.b_claim(
        owner,
        &config.backstop,
        &BackstopTier::FirstLoss,
        &config.pool,
        &first_min_lp_out,
    );
    let second_lp_out = manager.b_claim(
        owner,
        &config.backstop,
        &BackstopTier::SecondLoss,
        &config.pool,
        &second_min_lp_out,
    );
    ClaimResult {
        first_lp_out,
        second_lp_out,
    }
}

#[contractimpl]
impl BlntBackstopManagerHelper {
    #[allow(clippy::too_many_arguments)]
    pub fn __constructor(
        e: Env,
        owner: Address,
        manager: Address,
        backstop: Address,
        pool: Address,
        first_lp: Address,
        second_lp: Address,
        blnt: Address,
        xlm: Address,
        usdc: Address,
    ) {
        let contract = e.current_contract_address();
        let addresses = soroban_sdk::vec![
            &e,
            contract,
            owner.clone(),
            manager.clone(),
            backstop.clone(),
            pool.clone(),
            first_lp.clone(),
            second_lp.clone(),
            blnt.clone(),
            xlm.clone(),
            usdc.clone(),
        ];
        require_distinct(&e, &addresses);
        if blnt.executable() != Some(Executable::StellarAsset)
            || xlm.executable() != Some(Executable::StellarAsset)
            || usdc.executable() != Some(Executable::StellarAsset)
            || TokenClient::new(&e, &blnt).decimals() != 7
            || TokenClient::new(&e, &xlm).decimals() != 7
            || TokenClient::new(&e, &usdc).decimals() != 7
        {
            panic_with_error!(&e, HelperError::InvalidConfig);
        }

        let manager_client = ManagerClient::new(&e, &manager);
        if !contains(&manager_client.backstops(), &backstop) {
            panic_with_error!(&e, HelperError::InvalidBackstop);
        }
        if !contains(&manager_client.pools(), &pool) {
            panic_with_error!(&e, HelperError::InvalidPool);
        }
        let backstop_client = BackstopClient::new(&e, &backstop);
        if backstop_client.backstop_token(&BackstopTier::FirstLoss, &pool) != first_lp
            || backstop_client.backstop_token(&BackstopTier::SecondLoss, &pool) != second_lp
        {
            panic_with_error!(&e, HelperError::InvalidTierToken);
        }
        let first_tokens = CometClient::new(&e, &first_lp).get_tokens();
        let second_tokens = CometClient::new(&e, &second_lp).get_tokens();
        let first_blnt_first = pair_order(&first_tokens, &blnt, &xlm)
            .unwrap_or_else(|| panic_with_error!(&e, HelperError::InvalidPair));
        let second_blnt_first = pair_order(&second_tokens, &blnt, &usdc)
            .unwrap_or_else(|| panic_with_error!(&e, HelperError::InvalidPair));
        require_positive_pool(&e, &first_lp, &blnt, &xlm);
        require_positive_pool(&e, &second_lp, &blnt, &usdc);

        storage::set_config(
            &e,
            &Config {
                backstop,
                blnt,
                first_blnt_first,
                first_lp,
                manager,
                owner,
                pool,
                second_blnt_first,
                second_lp,
                usdc,
                xlm,
            },
        );
        storage::extend(&e);
    }

    pub fn deposit(e: Env, first_blnt: i128, second_blnt: i128) -> DepositResult {
        storage::extend(&e);
        let config = storage::get_config(&e);
        let owner = configured_owner(&config);
        let result = execute_deposit(&e, &config, &owner, first_blnt, second_blnt);
        events::deposit(&e, result.clone());
        result
    }

    pub fn claim(e: Env, first_min_lp_out: i128, second_min_lp_out: i128) -> ClaimResult {
        storage::extend(&e);
        let config = storage::get_config(&e);
        let owner = configured_owner(&config);
        let result = execute_claim(&e, &config, &owner, first_min_lp_out, second_min_lp_out);
        events::claim(&e, first_min_lp_out, second_min_lp_out, result.clone());
        result
    }

    pub fn deposit_and_claim(e: Env) -> DepositAndClaimResult {
        storage::extend(&e);
        let config = storage::get_config(&e);
        let owner = configured_owner(&config);
        let manager = ManagerClient::new(&e, &config.manager);
        let blnt_balance = TokenClient::new(&e, &config.blnt).balance(&config.manager);
        let harvestable = manager.harvestable(&config.blnt);
        if harvestable < 0 || harvestable > blnt_balance {
            panic_with_error!(&e, HelperError::InvalidState);
        }
        let available = checked_sub(&e, blnt_balance, harvestable);
        let first_blnt = available / 2;
        let second_blnt = checked_sub(&e, available, first_blnt);
        let deposit = execute_deposit(&e, &config, &owner, first_blnt, second_blnt);
        let claim = execute_claim(&e, &config, &owner, 0, 0);
        let result = DepositAndClaimResult { claim, deposit };
        events::deposit_and_claim(&e, result.clone());
        result
    }

    pub fn get_config(e: Env) -> Config {
        storage::extend(&e);
        storage::get_config(&e)
    }
}
