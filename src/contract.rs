use crate::{
    clients::{BackstopClient, CometClient, ManagerClient},
    events, storage,
    types::{
        BackstopTier, ClaimResult, Config, DepositAndClaimResult, DepositResult, TierDepositResult,
    },
    HelperError,
};
use soroban_sdk::{
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    contract, contractimpl, panic_with_error,
    token::TokenClient,
    vec, Address, Env, Executable, IntoVal, Symbol, Vec, I256,
};

const BPS_SCALE: i128 = 10_000;
const OUTPUT_MARGIN_BPS: i128 = 100;
const STROOP: i128 = 10_000_000;

#[contract]
pub struct BlntBackstopManagerHelper;

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

fn minimum_single_sided_lp(e: &Env, config: &Config, lp: &Address, blnt_in: i128) -> i128 {
    if blnt_in <= 0 {
        panic_with_error!(e, HelperError::InvalidAmount);
    }
    let comet = CometClient::new(e, lp);
    let supply = comet.get_total_supply();
    let blnt_reserve = comet.get_balance(&config.blnt);
    let weight = comet.get_normalized_weight(&config.blnt);
    let fee = comet.get_swap_fee();
    if supply <= 0
        || blnt_reserve <= 0
        || weight <= 0
        || weight >= STROOP
        || !(0..STROOP).contains(&fee)
    {
        panic_with_error!(e, HelperError::InvalidState);
    }

    // For 0 < weight < 1 and x >= 0:
    // (1 + x)^weight - 1 >= weight * x / (1 + x).
    // Use that lower bound after Comet's taxable-balance fee adjustment, then
    // apply a fixed 1% margin for integer approximation differences.
    let taxable_fee = mul_div_floor(e, STROOP - weight, fee, STROOP);
    let after_fee = mul_div_floor(e, blnt_in, STROOP - taxable_fee, STROOP);
    let weighted_in = mul_div_floor(e, after_fee, weight, STROOP);
    let denominator = checked_add(e, blnt_reserve, after_fee);
    let lower_bound = mul_div_floor(e, supply, weighted_in, denominator);
    let minimum = mul_div_floor(e, lower_bound, BPS_SCALE - OUTPUT_MARGIN_BPS, BPS_SCALE);
    if minimum <= 0 {
        panic_with_error!(e, HelperError::InvalidAmount);
    }
    minimum
}

fn single_sided_deposit_and_deposit(
    e: &Env,
    config: &Config,
    owner: &Address,
    tier: BackstopTier,
    lp: &Address,
    blnt_in: i128,
    min_lp_out: i128,
) -> TierDepositResult {
    let manager = ManagerClient::new(e, &config.manager);
    let helper = e.current_contract_address();
    let blnt = TokenClient::new(e, &config.blnt);
    let lp_token = TokenClient::new(e, lp);
    let manager_blnt_before = blnt.balance(&config.manager);
    let helper_blnt_before = blnt.balance(&helper);
    let manager_lp_before = lp_token.balance(&config.manager);
    let helper_lp_before = lp_token.balance(&helper);
    manager.transfer_token(&config.blnt, &helper, &blnt_in);
    if manager_blnt_before.checked_sub(blnt.balance(&config.manager)) != Some(blnt_in)
        || blnt.balance(&helper).checked_sub(helper_blnt_before) != Some(blnt_in)
    {
        panic_with_error!(e, HelperError::BalanceMismatch);
    }

    let approval_ledger = (e.ledger().sequence() / 100_000 + 1) * 100_000;
    e.authorize_as_current_contract(vec![
        e,
        InvokerContractAuthEntry::Contract(SubContractInvocation {
            context: ContractContext {
                contract: config.blnt.clone(),
                fn_name: Symbol::new(e, "approve"),
                args: vec![
                    e,
                    helper.clone().into_val(e),
                    lp.clone().into_val(e),
                    blnt_in.into_val(e),
                    approval_ledger.into_val(e),
                ],
            },
            sub_invocations: vec![e],
        }),
    ]);
    let lp_minted = CometClient::new(e, lp).dep_tokn_amt_in_get_lp_tokns_out(
        &config.blnt,
        &blnt_in,
        &min_lp_out,
        &helper,
    );
    if lp_minted < min_lp_out
        || blnt.balance(&helper) != helper_blnt_before
        || lp_token.balance(&helper).checked_sub(helper_lp_before) != Some(lp_minted)
    {
        panic_with_error!(e, HelperError::BalanceMismatch);
    }

    lp_token.transfer(&helper, &config.manager, &lp_minted);
    if lp_token.balance(&helper) != helper_lp_before
        || lp_token
            .balance(&config.manager)
            .checked_sub(manager_lp_before)
            != Some(lp_minted)
    {
        panic_with_error!(e, HelperError::BalanceMismatch);
    }

    let backstop_shares =
        manager.b_deposit(owner, &config.backstop, &tier, &config.pool, &lp_minted);
    if backstop_shares <= 0 || lp_token.balance(&config.manager) != manager_lp_before {
        panic_with_error!(e, HelperError::BalanceMismatch);
    }

    TierDepositResult {
        backstop_shares,
        blnt_in,
        lp_minted,
        paired_in: 0,
    }
}

fn execute_deposit(
    e: &Env,
    config: &Config,
    owner: &Address,
    first_blnt: i128,
    second_blnt: i128,
) -> DepositResult {
    if first_blnt <= 0 || second_blnt <= 0 {
        panic_with_error!(e, HelperError::InvalidAmount);
    }
    let blnt = TokenClient::new(e, &config.blnt);
    let xlm = TokenClient::new(e, &config.xlm);
    let usdc = TokenClient::new(e, &config.usdc);
    let blnt_before = blnt.balance(&config.manager);
    let xlm_before = xlm.balance(&config.manager);
    let usdc_before = usdc.balance(&config.manager);
    let harvestable = ManagerClient::new(e, &config.manager).harvestable(&config.blnt);
    let blnt_required = checked_add(e, first_blnt, second_blnt);
    if harvestable < 0
        || harvestable > blnt_before
        || checked_sub(e, blnt_before, harvestable) < blnt_required
    {
        panic_with_error!(e, HelperError::InsufficientBalance);
    }
    // Derive both execution bounds before the first external mutation so an
    // invalid second route cannot begin the first route.
    let first_min_lp_out = minimum_single_sided_lp(e, config, &config.first_lp, first_blnt);
    let second_min_lp_out = minimum_single_sided_lp(e, config, &config.second_lp, second_blnt);

    let first = single_sided_deposit_and_deposit(
        e,
        config,
        owner,
        BackstopTier::FirstLoss,
        &config.first_lp,
        first_blnt,
        first_min_lp_out,
    );
    let second = single_sided_deposit_and_deposit(
        e,
        config,
        owner,
        BackstopTier::SecondLoss,
        &config.second_lp,
        second_blnt,
        second_min_lp_out,
    );

    if blnt_before.checked_sub(blnt.balance(&config.manager)) != Some(blnt_required)
        || xlm.balance(&config.manager) != xlm_before
        || usdc.balance(&config.manager) != usdc_before
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
