use crate::{BackstopTier, BlntBackstopManagerHelper, BlntBackstopManagerHelperClient};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error,
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    token::{StellarAssetClient, TokenClient},
    vec, Address, Env, IntoVal, Vec,
};
use std::panic::{catch_unwind, AssertUnwindSafe};

#[derive(Clone)]
#[contracttype]
enum CometKey {
    Token0,
    Token1,
    Reserve0,
    Reserve1,
    Weight0,
    Weight1,
    Fee,
    Supply,
    Balance(Address),
    FailDeposit,
    LastMinimum,
}

#[contract]
struct MockComet;

#[contractimpl]
impl MockComet {
    #[allow(clippy::too_many_arguments)]
    pub fn __constructor(
        e: Env,
        token0: Address,
        token1: Address,
        reserve0: i128,
        reserve1: i128,
        weight0: i128,
        weight1: i128,
        fee: i128,
        supply: i128,
    ) {
        e.storage().instance().set(&CometKey::Token0, &token0);
        e.storage().instance().set(&CometKey::Token1, &token1);
        e.storage().instance().set(&CometKey::Reserve0, &reserve0);
        e.storage().instance().set(&CometKey::Reserve1, &reserve1);
        e.storage().instance().set(&CometKey::Weight0, &weight0);
        e.storage().instance().set(&CometKey::Weight1, &weight1);
        e.storage().instance().set(&CometKey::Fee, &fee);
        e.storage().instance().set(&CometKey::Supply, &supply);
    }

    pub fn balance(e: Env, id: Address) -> i128 {
        e.storage()
            .persistent()
            .get(&CometKey::Balance(id))
            .unwrap_or(0)
    }

    pub fn get_balance(e: Env, token: Address) -> i128 {
        let token0: Address = e.storage().instance().get(&CometKey::Token0).unwrap();
        if token == token0 {
            e.storage().instance().get(&CometKey::Reserve0).unwrap()
        } else {
            let token1: Address = e.storage().instance().get(&CometKey::Token1).unwrap();
            assert_eq!(token, token1);
            e.storage().instance().get(&CometKey::Reserve1).unwrap()
        }
    }

    pub fn get_total_supply(e: Env) -> i128 {
        e.storage().instance().get(&CometKey::Supply).unwrap()
    }

    pub fn get_normalized_weight(e: Env, token: Address) -> i128 {
        let token0: Address = e.storage().instance().get(&CometKey::Token0).unwrap();
        if token == token0 {
            e.storage().instance().get(&CometKey::Weight0).unwrap()
        } else {
            let token1: Address = e.storage().instance().get(&CometKey::Token1).unwrap();
            assert_eq!(token, token1);
            e.storage().instance().get(&CometKey::Weight1).unwrap()
        }
    }

    pub fn get_swap_fee(e: Env) -> i128 {
        e.storage().instance().get(&CometKey::Fee).unwrap()
    }

    pub fn get_tokens(e: Env) -> Vec<Address> {
        vec![
            &e,
            e.storage().instance().get(&CometKey::Token0).unwrap(),
            e.storage().instance().get(&CometKey::Token1).unwrap(),
        ]
    }

    pub fn mint(e: Env, to: Address, amount: i128) {
        let balance = Self::balance(e.clone(), to.clone());
        e.storage()
            .persistent()
            .set(&CometKey::Balance(to), &(balance + amount));
    }

    pub fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        let from_balance = Self::balance(e.clone(), from.clone());
        assert!(from_balance >= amount);
        let to_balance = Self::balance(e.clone(), to.clone());
        e.storage()
            .persistent()
            .set(&CometKey::Balance(from), &(from_balance - amount));
        e.storage()
            .persistent()
            .set(&CometKey::Balance(to), &(to_balance + amount));
    }

    pub fn dep_tokn_amt_in_get_lp_tokns_out(
        e: Env,
        token_in: Address,
        token_amount_in: i128,
        min_pool_amount_out: i128,
        user: Address,
    ) -> i128 {
        if e.storage()
            .persistent()
            .get(&CometKey::FailDeposit)
            .unwrap_or(false)
        {
            panic_with_error!(&e, MockError::ForcedFailure);
        }
        let reserve = Self::get_balance(e.clone(), token_in.clone());
        let supply = Self::get_total_supply(e.clone());
        e.storage()
            .persistent()
            .set(&CometKey::LastMinimum, &min_pool_amount_out);
        let pool_amount_out = token_amount_in * supply / reserve;
        assert!(pool_amount_out > 0 && pool_amount_out >= min_pool_amount_out);

        let approval_ledger = (e.ledger().sequence() / 100_000 + 1) * 100_000;
        let token = TokenClient::new(&e, &token_in);
        token.approve(
            &user,
            &e.current_contract_address(),
            &token_amount_in,
            &approval_ledger,
        );
        token.transfer_from(
            &e.current_contract_address(),
            &user,
            &e.current_contract_address(),
            &token_amount_in,
        );

        let token0: Address = e.storage().instance().get(&CometKey::Token0).unwrap();
        let reserve_key = if token_in == token0 {
            CometKey::Reserve0
        } else {
            CometKey::Reserve1
        };
        e.storage()
            .instance()
            .set(&reserve_key, &(reserve + token_amount_in));
        e.storage()
            .instance()
            .set(&CometKey::Supply, &(supply + pool_amount_out));
        Self::mint(e, user, pool_amount_out);
        pool_amount_out
    }

    pub fn burn(e: Env, from: Address, amount: i128) {
        let balance = Self::balance(e.clone(), from.clone());
        assert!(balance >= amount);
        e.storage()
            .persistent()
            .set(&CometKey::Balance(from), &(balance - amount));
    }

    pub fn set_pool(e: Env, reserve0: i128, reserve1: i128, supply: i128) {
        e.storage().instance().set(&CometKey::Reserve0, &reserve0);
        e.storage().instance().set(&CometKey::Reserve1, &reserve1);
        e.storage().instance().set(&CometKey::Supply, &supply);
    }

    pub fn set_fail_deposit(e: Env, fail: bool) {
        e.storage().persistent().set(&CometKey::FailDeposit, &fail);
    }

    pub fn last_minimum(e: Env) -> i128 {
        e.storage()
            .persistent()
            .get(&CometKey::LastMinimum)
            .unwrap_or(0)
    }
}

#[derive(Clone)]
#[contracttype]
enum BackstopKey {
    First,
    Second,
}

#[contract]
struct MockBackstop;

#[contractimpl]
impl MockBackstop {
    pub fn __constructor(e: Env, first: Address, second: Address) {
        e.storage().instance().set(&BackstopKey::First, &first);
        e.storage().instance().set(&BackstopKey::Second, &second);
    }

    pub fn backstop_token(e: Env, tier: BackstopTier, _pool: Address) -> Address {
        match tier {
            BackstopTier::FirstLoss => e.storage().instance().get(&BackstopKey::First).unwrap(),
            BackstopTier::SecondLoss => e.storage().instance().get(&BackstopKey::Second).unwrap(),
            BackstopTier::ThirdLoss => panic!("unexpected tier"),
        }
    }
}

#[derive(Clone)]
#[contracttype]
enum ManagerKey {
    Owner,
    Manager,
    ManagerScope,
    Backstop,
    Pool,
    FirstLp,
    SecondLp,
    Harvestable(Address),
    Deposited(BackstopTier),
    Claims(BackstopTier),
    ClaimOutput(BackstopTier),
    FailDeposit(BackstopTier),
    FailClaim(BackstopTier),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MockError {
    ForcedFailure = 1,
}

#[contract]
struct MockManager;

#[contractimpl]
impl MockManager {
    pub fn __constructor(
        e: Env,
        owner: Address,
        backstop: Address,
        pool: Address,
        first_lp: Address,
        second_lp: Address,
    ) {
        e.storage().instance().set(&ManagerKey::Owner, &owner);
        e.storage().instance().set(&ManagerKey::Backstop, &backstop);
        e.storage().instance().set(&ManagerKey::Pool, &pool);
        e.storage().instance().set(&ManagerKey::FirstLp, &first_lp);
        e.storage()
            .instance()
            .set(&ManagerKey::SecondLp, &second_lp);
        e.storage()
            .instance()
            .set(&ManagerKey::ManagerScope, &0_u32);
        e.storage()
            .instance()
            .set(&ManagerKey::ClaimOutput(BackstopTier::FirstLoss), &7_i128);
        e.storage()
            .instance()
            .set(&ManagerKey::ClaimOutput(BackstopTier::SecondLoss), &9_i128);
    }

    pub fn owner(e: Env) -> Address {
        e.storage().instance().get(&ManagerKey::Owner).unwrap()
    }

    pub fn harvestable(e: Env, token: Address) -> i128 {
        e.storage()
            .persistent()
            .get(&ManagerKey::Harvestable(token))
            .unwrap_or(0)
    }

    pub fn backstops(e: Env) -> Vec<Address> {
        vec![
            &e,
            e.storage().instance().get(&ManagerKey::Backstop).unwrap(),
        ]
    }

    pub fn pools(e: Env) -> Vec<Address> {
        vec![&e, e.storage().instance().get(&ManagerKey::Pool).unwrap()]
    }

    pub fn transfer_token(e: Env, token: Address, to: Address, amount: i128) {
        let owner: Address = e.storage().instance().get(&ManagerKey::Owner).unwrap();
        owner.require_auth();
        TokenClient::new(&e, &token).transfer(&e.current_contract_address(), &to, &amount);
    }

    pub fn b_deposit(
        e: Env,
        from: Address,
        backstop: Address,
        tier: BackstopTier,
        pool: Address,
        amount: i128,
    ) -> i128 {
        Self::require_actor(&e, &from, 1);
        Self::require_route(&e, &backstop, &pool);
        if e.storage()
            .persistent()
            .get(&ManagerKey::FailDeposit(tier))
            .unwrap_or(false)
        {
            panic_with_error!(&e, MockError::ForcedFailure);
        }
        MockCometClient::new(&e, &Self::lp(&e, tier)).burn(&e.current_contract_address(), &amount);
        let deposited = Self::deposited(e.clone(), tier);
        e.storage()
            .persistent()
            .set(&ManagerKey::Deposited(tier), &(deposited + amount));
        amount
    }

    pub fn b_claim(
        e: Env,
        from: Address,
        backstop: Address,
        tier: BackstopTier,
        pool: Address,
        min_lp_tokens_out: i128,
    ) -> i128 {
        Self::require_actor(&e, &from, 0);
        Self::require_route(&e, &backstop, &pool);
        if e.storage()
            .persistent()
            .get(&ManagerKey::FailClaim(tier))
            .unwrap_or(false)
        {
            panic_with_error!(&e, MockError::ForcedFailure);
        }
        let output: i128 = e
            .storage()
            .instance()
            .get(&ManagerKey::ClaimOutput(tier))
            .unwrap();
        assert!(output >= min_lp_tokens_out);
        let claims = Self::claims(e.clone(), tier);
        e.storage()
            .persistent()
            .set(&ManagerKey::Claims(tier), &(claims + 1));
        output
    }

    pub fn set_harvestable(e: Env, token: Address, amount: i128) {
        e.storage()
            .persistent()
            .set(&ManagerKey::Harvestable(token), &amount);
    }

    pub fn set_owner(e: Env, owner: Address) {
        e.storage().instance().set(&ManagerKey::Owner, &owner);
    }

    pub fn set_manager(e: Env, manager: Address, scope: u32) {
        e.storage().instance().set(&ManagerKey::Manager, &manager);
        e.storage()
            .instance()
            .set(&ManagerKey::ManagerScope, &scope);
    }

    pub fn set_fail_deposit(e: Env, tier: BackstopTier, fail: bool) {
        e.storage()
            .persistent()
            .set(&ManagerKey::FailDeposit(tier), &fail);
    }

    pub fn set_fail_claim(e: Env, tier: BackstopTier, fail: bool) {
        e.storage()
            .persistent()
            .set(&ManagerKey::FailClaim(tier), &fail);
    }

    pub fn deposited(e: Env, tier: BackstopTier) -> i128 {
        e.storage()
            .persistent()
            .get(&ManagerKey::Deposited(tier))
            .unwrap_or(0)
    }

    pub fn claims(e: Env, tier: BackstopTier) -> i128 {
        e.storage()
            .persistent()
            .get(&ManagerKey::Claims(tier))
            .unwrap_or(0)
    }

    fn lp(e: &Env, tier: BackstopTier) -> Address {
        match tier {
            BackstopTier::FirstLoss => e.storage().instance().get(&ManagerKey::FirstLp).unwrap(),
            BackstopTier::SecondLoss => e.storage().instance().get(&ManagerKey::SecondLp).unwrap(),
            BackstopTier::ThirdLoss => panic!("unexpected tier"),
        }
    }

    fn require_route(e: &Env, backstop: &Address, pool: &Address) {
        let expected_backstop: Address = e.storage().instance().get(&ManagerKey::Backstop).unwrap();
        let expected_pool: Address = e.storage().instance().get(&ManagerKey::Pool).unwrap();
        assert_eq!(*backstop, expected_backstop);
        assert_eq!(*pool, expected_pool);
    }

    fn require_actor(e: &Env, from: &Address, required_scope: u32) {
        from.require_auth();
        let owner: Address = e.storage().instance().get(&ManagerKey::Owner).unwrap();
        if *from == owner {
            return;
        }
        let manager: Option<Address> = e.storage().instance().get(&ManagerKey::Manager);
        let scope: u32 = e
            .storage()
            .instance()
            .get(&ManagerKey::ManagerScope)
            .unwrap();
        if manager != Some(from.clone()) || scope < required_scope {
            panic_with_error!(e, MockError::ForcedFailure);
        }
    }
}

struct Fixture<'a> {
    e: Env,
    owner: Address,
    blnt: Address,
    xlm: Address,
    usdc: Address,
    first_lp: Address,
    second_lp: Address,
    backstop: Address,
    pool: Address,
    manager: MockManagerClient<'a>,
    helper: BlntBackstopManagerHelperClient<'a>,
}

impl Fixture<'_> {
    fn create() -> Self {
        let e = Env::default();
        e.mock_all_auths();
        let owner = Address::generate(&e);
        let asset_admin = Address::generate(&e);
        let blnt = e
            .register_stellar_asset_contract_v2(asset_admin.clone())
            .address();
        let xlm = e
            .register_stellar_asset_contract_v2(asset_admin.clone())
            .address();
        let usdc = e
            .register_stellar_asset_contract_v2(asset_admin.clone())
            .address();
        let first_lp = e.register(
            MockComet,
            (
                blnt.clone(),
                xlm.clone(),
                1_000_i128,
                200_i128,
                8_000_000_i128,
                2_000_000_i128,
                30_000_i128,
                100_i128,
            ),
        );
        let second_lp = e.register(
            MockComet,
            (
                usdc.clone(),
                blnt.clone(),
                500_i128,
                2_000_i128,
                2_000_000_i128,
                8_000_000_i128,
                30_000_i128,
                100_i128,
            ),
        );
        let pool = Address::generate(&e);
        let backstop = e.register(MockBackstop, (first_lp.clone(), second_lp.clone()));
        let manager_address = e.register(
            MockManager,
            (
                owner.clone(),
                backstop.clone(),
                pool.clone(),
                first_lp.clone(),
                second_lp.clone(),
            ),
        );
        let helper_address = e.register(
            BlntBackstopManagerHelper,
            (
                owner.clone(),
                manager_address.clone(),
                backstop.clone(),
                pool.clone(),
                first_lp.clone(),
                second_lp.clone(),
                blnt.clone(),
                xlm.clone(),
                usdc.clone(),
            ),
        );
        let manager = MockManagerClient::new(&e, &manager_address);
        let helper = BlntBackstopManagerHelperClient::new(&e, &helper_address);
        StellarAssetClient::new(&e, &blnt).mint(&manager_address, &1_000);
        StellarAssetClient::new(&e, &xlm).mint(&manager_address, &1_000);
        StellarAssetClient::new(&e, &usdc).mint(&manager_address, &1_000);
        // Constructor and fixture setup costs are unrelated to each tested
        // invocation. Reset before handing the environment to the test.
        e.cost_estimate().budget().reset_unlimited();
        Self {
            e,
            owner,
            blnt,
            xlm,
            usdc,
            first_lp,
            second_lp,
            backstop,
            pool,
            manager,
            helper,
        }
    }

    fn token_balance(&self, token: &Address) -> i128 {
        TokenClient::new(&self.e, token).balance(&self.manager.address)
    }
}

#[test]
fn deposit_uses_blnt_budgets_and_deposits_both_tiers() {
    let fixture = Fixture::create();
    let result = fixture.helper.deposit(&100, &200);

    assert_eq!(result.first.blnt_in, 100);
    assert_eq!(result.first.paired_in, 0);
    assert_eq!(result.first.lp_minted, 10);
    assert_eq!(result.first.backstop_shares, 10);
    assert_eq!(result.second.blnt_in, 200);
    assert_eq!(result.second.paired_in, 0);
    assert_eq!(result.second.lp_minted, 10);
    assert_eq!(result.second.backstop_shares, 10);
    assert_eq!(fixture.token_balance(&fixture.blnt), 700);
    assert_eq!(fixture.token_balance(&fixture.xlm), 1_000);
    assert_eq!(fixture.token_balance(&fixture.usdc), 1_000);
    assert_eq!(fixture.manager.deposited(&BackstopTier::FirstLoss), 10);
    assert_eq!(fixture.manager.deposited(&BackstopTier::SecondLoss), 10);
    assert_eq!(
        MockCometClient::new(&fixture.e, &fixture.first_lp).balance(&fixture.manager.address),
        0
    );
    assert_eq!(
        MockCometClient::new(&fixture.e, &fixture.second_lp).balance(&fixture.manager.address),
        0
    );
}

#[test]
fn claim_claims_both_tiers_with_required_minima() {
    let fixture = Fixture::create();
    let result = fixture.helper.claim(&3, &4);

    assert_eq!(result.first_lp_out, 7);
    assert_eq!(result.second_lp_out, 9);
    assert_eq!(fixture.manager.claims(&BackstopTier::FirstLoss), 1);
    assert_eq!(fixture.manager.claims(&BackstopTier::SecondLoss), 1);
}

#[test]
fn deposit_and_claim_splits_non_harvestable_blnt_and_claims_all() {
    let fixture = Fixture::create();
    fixture.manager.set_harvestable(&fixture.blnt, &1);

    let result = fixture.helper.deposit_and_claim();

    assert_eq!(result.deposit.first.blnt_in, 499);
    assert_eq!(result.deposit.first.paired_in, 0);
    assert_eq!(result.deposit.first.lp_minted, 49);
    assert_eq!(result.deposit.second.blnt_in, 500);
    assert_eq!(result.deposit.second.paired_in, 0);
    assert_eq!(result.deposit.second.lp_minted, 25);
    assert_eq!(result.claim.first_lp_out, 7);
    assert_eq!(result.claim.second_lp_out, 9);
    assert_eq!(fixture.token_balance(&fixture.blnt), 1);
    assert_eq!(fixture.token_balance(&fixture.xlm), 1_000);
    assert_eq!(fixture.token_balance(&fixture.usdc), 1_000);
    assert_eq!(fixture.manager.claims(&BackstopTier::FirstLoss), 1);
    assert_eq!(fixture.manager.claims(&BackstopTier::SecondLoss), 1);
}

#[test]
fn second_deposit_failure_rolls_back_first_deposit_and_transfers() {
    let fixture = Fixture::create();
    fixture
        .manager
        .set_fail_deposit(&BackstopTier::SecondLoss, &true);

    assert!(fixture.helper.try_deposit(&100, &200).is_err());
    assert_eq!(fixture.token_balance(&fixture.blnt), 1_000);
    assert_eq!(fixture.token_balance(&fixture.xlm), 1_000);
    assert_eq!(fixture.token_balance(&fixture.usdc), 1_000);
    assert_eq!(fixture.manager.deposited(&BackstopTier::FirstLoss), 0);
    assert_eq!(fixture.manager.deposited(&BackstopTier::SecondLoss), 0);
}

#[test]
fn second_claim_failure_rolls_back_first_claim() {
    let fixture = Fixture::create();
    fixture
        .manager
        .set_fail_claim(&BackstopTier::SecondLoss, &true);

    assert!(fixture.helper.try_claim(&0, &0).is_err());
    assert_eq!(fixture.manager.claims(&BackstopTier::FirstLoss), 0);
    assert_eq!(fixture.manager.claims(&BackstopTier::SecondLoss), 0);
}

#[test]
fn combined_claim_failure_rolls_back_deposits_and_transfers() {
    let fixture = Fixture::create();
    fixture
        .manager
        .set_fail_claim(&BackstopTier::SecondLoss, &true);

    assert!(fixture.helper.try_deposit_and_claim().is_err());
    assert_eq!(fixture.token_balance(&fixture.blnt), 1_000);
    assert_eq!(fixture.token_balance(&fixture.xlm), 1_000);
    assert_eq!(fixture.token_balance(&fixture.usdc), 1_000);
    assert_eq!(fixture.manager.deposited(&BackstopTier::FirstLoss), 0);
    assert_eq!(fixture.manager.deposited(&BackstopTier::SecondLoss), 0);
    assert_eq!(fixture.manager.claims(&BackstopTier::FirstLoss), 0);
}

#[test]
fn rejects_negative_claim_minimum() {
    let fixture = Fixture::create();
    assert!(fixture.helper.try_claim(&-1, &0).is_err());
}

#[test]
fn configured_helper_owner_must_authorize() {
    let fixture = Fixture::create();
    fixture.e.set_auths(&[]);

    let result = fixture
        .helper
        .mock_auths(&[MockAuth {
            address: &Address::generate(&fixture.e),
            invoke: &MockAuthInvoke {
                contract: &fixture.helper.address,
                fn_name: "deposit",
                args: vec![
                    &fixture.e,
                    100_i128.into_val(&fixture.e),
                    200_i128.into_val(&fixture.e),
                ],
                sub_invokes: &[],
            },
        }])
        .try_deposit(&100, &200);
    assert!(result.is_err());
    assert_eq!(fixture.owner, fixture.helper.get_config().owner);
}

#[test]
fn scoped_manager_can_claim_but_cannot_fund_after_manager_ownership_transfer() {
    let fixture = Fixture::create();
    let new_owner = Address::generate(&fixture.e);
    fixture.manager.set_owner(&new_owner);

    assert!(fixture.helper.try_claim(&0, &0).is_err());
    fixture.manager.set_manager(&fixture.owner, &0);
    assert_eq!(fixture.helper.claim(&0, &0).first_lp_out, 7);
    assert!(fixture.helper.try_deposit(&100, &200).is_err());
    fixture.manager.set_manager(&fixture.owner, &2);
    assert!(fixture.helper.try_deposit(&100, &200).is_err());
    assert_eq!(fixture.helper.get_config().owner, fixture.owner);
}

#[test]
fn zero_deposit_budget_is_rejected_before_mutation() {
    let fixture = Fixture::create();
    assert!(fixture.helper.try_deposit(&0, &200).is_err());
    assert_eq!(fixture.token_balance(&fixture.blnt), 1_000);
    assert_eq!(fixture.manager.deposited(&BackstopTier::FirstLoss), 0);
}

#[test]
fn zero_counterasset_balances_do_not_block_single_sided_deposits() {
    let fixture = Fixture::create();
    TokenClient::new(&fixture.e, &fixture.xlm).transfer(
        &fixture.manager.address,
        Address::generate(&fixture.e),
        &1_000,
    );
    TokenClient::new(&fixture.e, &fixture.usdc).transfer(
        &fixture.manager.address,
        Address::generate(&fixture.e),
        &1_000,
    );

    let result = fixture.helper.deposit(&100, &200);
    assert_eq!(result.first.paired_in, 0);
    assert_eq!(result.second.paired_in, 0);
    assert_eq!(fixture.token_balance(&fixture.blnt), 700);
    assert_eq!(fixture.token_balance(&fixture.xlm), 0);
    assert_eq!(fixture.token_balance(&fixture.usdc), 0);
}

#[test]
fn combined_operation_rejects_harvestable_above_balance() {
    let fixture = Fixture::create();
    fixture.manager.set_harvestable(&fixture.blnt, &1_001);

    assert!(fixture.helper.try_deposit_and_claim().is_err());
    assert_eq!(fixture.token_balance(&fixture.blnt), 1_000);
    assert_eq!(fixture.manager.deposited(&BackstopTier::FirstLoss), 0);
}

#[test]
fn deposit_applies_exact_blnt_and_changes_no_counterasset_balance() {
    let fixture = Fixture::create();
    let first_comet = MockCometClient::new(&fixture.e, &fixture.first_lp);
    let second_comet = MockCometClient::new(&fixture.e, &fixture.second_lp);
    let result = fixture.helper.deposit(&100, &200);

    assert_eq!(result.first.blnt_in, 100);
    assert_eq!(result.second.blnt_in, 200);
    assert_eq!(first_comet.get_balance(&fixture.blnt), 1_100);
    assert_eq!(first_comet.get_balance(&fixture.xlm), 200);
    assert_eq!(second_comet.get_balance(&fixture.blnt), 2_200);
    assert_eq!(second_comet.get_balance(&fixture.usdc), 500);
    assert_eq!(first_comet.last_minimum(), 6);
    assert_eq!(second_comet.last_minimum(), 6);
}

#[test]
fn deposit_rejects_amount_too_small_for_a_positive_minimum() {
    let fixture = Fixture::create();
    MockCometClient::new(&fixture.e, &fixture.first_lp).set_pool(&1, &1, &1);
    MockCometClient::new(&fixture.e, &fixture.second_lp).set_pool(&1, &1, &1);

    assert!(fixture.helper.try_deposit(&1, &1).is_err());
    assert_no_deposit_effects(&fixture);
}

#[test]
fn negative_and_near_overflow_deposit_inputs_fail_before_mutation() {
    let fixture = Fixture::create();
    assert!(fixture.helper.try_deposit(&-1, &200).is_err());
    assert!(fixture.helper.try_deposit(&i128::MAX, &i128::MAX).is_err());
    assert_eq!(fixture.token_balance(&fixture.blnt), 1_000);
    assert_eq!(fixture.manager.deposited(&BackstopTier::FirstLoss), 0);
    assert_eq!(fixture.manager.deposited(&BackstopTier::SecondLoss), 0);
}

#[test]
fn insufficient_blnt_fails_before_mutation() {
    let fixture = Fixture::create();
    TokenClient::new(&fixture.e, &fixture.blnt).transfer(
        &fixture.manager.address,
        Address::generate(&fixture.e),
        &1_000,
    );

    assert!(fixture.helper.try_deposit(&100, &200).is_err());
    assert_eq!(fixture.manager.deposited(&BackstopTier::FirstLoss), 0);
}

#[test]
fn absent_xlm_does_not_block_single_sided_deposit() {
    let fixture = Fixture::create();
    TokenClient::new(&fixture.e, &fixture.xlm).transfer(
        &fixture.manager.address,
        Address::generate(&fixture.e),
        &1_000,
    );

    let result = fixture.helper.deposit(&100, &200);
    assert_eq!(result.first.lp_minted, 10);
    assert_eq!(fixture.token_balance(&fixture.blnt), 700);
    assert_eq!(fixture.token_balance(&fixture.xlm), 0);
}

#[test]
fn first_comet_failure_leaves_all_balances_unchanged() {
    let fixture = Fixture::create();
    MockCometClient::new(&fixture.e, &fixture.first_lp).set_fail_deposit(&true);
    assert!(fixture.helper.try_deposit(&100, &200).is_err());
    assert_no_deposit_effects(&fixture);
}

#[test]
fn second_comet_failure_rolls_back_first_route() {
    let fixture = Fixture::create();
    MockCometClient::new(&fixture.e, &fixture.second_lp).set_fail_deposit(&true);
    assert!(fixture.helper.try_deposit(&100, &200).is_err());
    assert_no_deposit_effects(&fixture);
}

#[test]
fn first_backstop_deposit_failure_rolls_back_comet_deposit() {
    let fixture = Fixture::create();
    fixture
        .manager
        .set_fail_deposit(&BackstopTier::FirstLoss, &true);
    assert!(fixture.helper.try_deposit(&100, &200).is_err());
    assert_no_deposit_effects(&fixture);
}

#[test]
fn first_claim_failure_does_not_run_second_claim() {
    let fixture = Fixture::create();
    fixture
        .manager
        .set_fail_claim(&BackstopTier::FirstLoss, &true);
    assert!(fixture.helper.try_claim(&0, &0).is_err());
    assert_eq!(fixture.manager.claims(&BackstopTier::FirstLoss), 0);
    assert_eq!(fixture.manager.claims(&BackstopTier::SecondLoss), 0);
}

#[test]
fn combined_deposit_failure_does_not_claim_and_rolls_back() {
    let fixture = Fixture::create();
    fixture
        .manager
        .set_fail_deposit(&BackstopTier::SecondLoss, &true);
    assert!(fixture.helper.try_deposit_and_claim().is_err());
    assert_no_deposit_effects(&fixture);
    assert_eq!(fixture.manager.claims(&BackstopTier::FirstLoss), 0);
    assert_eq!(fixture.manager.claims(&BackstopTier::SecondLoss), 0);
}

#[derive(Clone, Copy)]
enum CombinedFailure {
    FirstComet,
    SecondComet,
    FirstDeposit,
    SecondDeposit,
    FirstClaim,
    SecondClaim,
}

#[test]
fn combined_operation_rolls_back_at_every_external_failure_point() {
    for failure in [
        CombinedFailure::FirstComet,
        CombinedFailure::SecondComet,
        CombinedFailure::FirstDeposit,
        CombinedFailure::SecondDeposit,
        CombinedFailure::FirstClaim,
        CombinedFailure::SecondClaim,
    ] {
        let fixture = Fixture::create();
        match failure {
            CombinedFailure::FirstComet => {
                MockCometClient::new(&fixture.e, &fixture.first_lp).set_fail_deposit(&true)
            }
            CombinedFailure::SecondComet => {
                MockCometClient::new(&fixture.e, &fixture.second_lp).set_fail_deposit(&true)
            }
            CombinedFailure::FirstDeposit => fixture
                .manager
                .set_fail_deposit(&BackstopTier::FirstLoss, &true),
            CombinedFailure::SecondDeposit => fixture
                .manager
                .set_fail_deposit(&BackstopTier::SecondLoss, &true),
            CombinedFailure::FirstClaim => fixture
                .manager
                .set_fail_claim(&BackstopTier::FirstLoss, &true),
            CombinedFailure::SecondClaim => fixture
                .manager
                .set_fail_claim(&BackstopTier::SecondLoss, &true),
        }
        assert!(fixture.helper.try_deposit_and_claim().is_err());
        assert_no_deposit_effects(&fixture);
        assert_eq!(fixture.manager.claims(&BackstopTier::FirstLoss), 0);
        assert_eq!(fixture.manager.claims(&BackstopTier::SecondLoss), 0);
    }
}

#[test]
fn combined_operation_assigns_odd_unit_to_second_budget() {
    let fixture = Fixture::create();
    fixture.manager.set_harvestable(&fixture.blnt, &1);
    let result = fixture.helper.deposit_and_claim();

    assert_eq!(result.deposit.first.lp_minted, 49);
    assert_eq!(result.deposit.second.lp_minted, 25);
    assert_eq!(result.deposit.first.blnt_in, 499);
    assert_eq!(result.deposit.second.blnt_in, 500);
}

#[test]
fn combined_operation_stays_within_soroban_resource_limits() {
    let fixture = Fixture::create();
    fixture.e.cost_estimate().budget().reset_unlimited();
    fixture.helper.deposit_and_claim();
    let resources = fixture.e.cost_estimate().resources();

    assert!(resources.instructions <= 600_000_000);
    assert!(resources.mem_bytes <= 41_943_040);
    assert!(resources.disk_read_entries <= 100);
    assert!(resources.write_entries <= 50);
    assert!(
        resources.disk_read_entries + resources.memory_read_entries + resources.write_entries <= 90
    );
    assert!(resources.disk_read_bytes <= 200_000);
    assert!(resources.write_bytes <= 132_096);
    assert!(resources.contract_events_size_bytes <= 16_384);
}

#[derive(Clone, Copy)]
enum InvalidBinding {
    Owner,
    Manager,
    Backstop,
    Pool,
    FirstLp,
    SecondLp,
    Blnt,
    Xlm,
    Usdc,
}

#[test]
fn constructor_accepts_owner_independent_of_manager_owner() {
    let fixture = Fixture::create();
    let independent_owner = Address::generate(&fixture.e);
    fixture.manager.set_manager(&independent_owner, &2);
    let helper_address = fixture.e.register(
        BlntBackstopManagerHelper,
        (
            independent_owner.clone(),
            fixture.manager.address.clone(),
            fixture.backstop.clone(),
            fixture.pool.clone(),
            fixture.first_lp.clone(),
            fixture.second_lp.clone(),
            fixture.blnt.clone(),
            fixture.xlm.clone(),
            fixture.usdc.clone(),
        ),
    );
    let helper = BlntBackstopManagerHelperClient::new(&fixture.e, &helper_address);
    assert_eq!(helper.get_config().owner, independent_owner);
    assert_eq!(helper.claim(&0, &0).first_lp_out, 7);
    assert!(helper.try_deposit(&100, &200).is_err());
}

#[test]
fn constructor_rejects_every_substituted_binding() {
    for binding in [
        InvalidBinding::Owner,
        InvalidBinding::Manager,
        InvalidBinding::Backstop,
        InvalidBinding::Pool,
        InvalidBinding::FirstLp,
        InvalidBinding::SecondLp,
        InvalidBinding::Blnt,
        InvalidBinding::Xlm,
        InvalidBinding::Usdc,
    ] {
        assert_invalid_constructor(binding);
    }
}

fn assert_invalid_constructor(binding: InvalidBinding) {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let fixture = Fixture::create();
        let mut owner = fixture.owner.clone();
        let mut manager = fixture.manager.address.clone();
        let mut backstop = fixture.backstop.clone();
        let mut pool = fixture.pool.clone();
        let mut first_lp = fixture.first_lp.clone();
        let mut second_lp = fixture.second_lp.clone();
        let mut blnt = fixture.blnt.clone();
        let mut xlm = fixture.xlm.clone();
        let mut usdc = fixture.usdc.clone();
        match binding {
            InvalidBinding::Owner => owner = manager.clone(),
            InvalidBinding::Manager => manager = Address::generate(&fixture.e),
            InvalidBinding::Backstop => backstop = Address::generate(&fixture.e),
            InvalidBinding::Pool => pool = Address::generate(&fixture.e),
            InvalidBinding::FirstLp => first_lp = Address::generate(&fixture.e),
            InvalidBinding::SecondLp => second_lp = Address::generate(&fixture.e),
            InvalidBinding::Blnt => {
                blnt = fixture
                    .e
                    .register_stellar_asset_contract_v2(Address::generate(&fixture.e))
                    .address()
            }
            InvalidBinding::Xlm => {
                xlm = fixture
                    .e
                    .register_stellar_asset_contract_v2(Address::generate(&fixture.e))
                    .address()
            }
            InvalidBinding::Usdc => {
                usdc = fixture
                    .e
                    .register_stellar_asset_contract_v2(Address::generate(&fixture.e))
                    .address()
            }
        }
        fixture.e.register(
            BlntBackstopManagerHelper,
            (
                owner, manager, backstop, pool, first_lp, second_lp, blnt, xlm, usdc,
            ),
        );
    }));
    assert!(result.is_err());
}

fn assert_no_deposit_effects(fixture: &Fixture<'_>) {
    assert_eq!(fixture.token_balance(&fixture.blnt), 1_000);
    assert_eq!(fixture.token_balance(&fixture.xlm), 1_000);
    assert_eq!(fixture.token_balance(&fixture.usdc), 1_000);
    assert_eq!(fixture.manager.deposited(&BackstopTier::FirstLoss), 0);
    assert_eq!(fixture.manager.deposited(&BackstopTier::SecondLoss), 0);
    assert_eq!(
        TokenClient::new(&fixture.e, &fixture.blnt).balance(&fixture.helper.address),
        0
    );
    assert_eq!(
        MockCometClient::new(&fixture.e, &fixture.first_lp).balance(&fixture.manager.address),
        0
    );
    assert_eq!(
        MockCometClient::new(&fixture.e, &fixture.second_lp).balance(&fixture.manager.address),
        0
    );
    assert_eq!(
        MockCometClient::new(&fixture.e, &fixture.first_lp).balance(&fixture.helper.address),
        0
    );
    assert_eq!(
        MockCometClient::new(&fixture.e, &fixture.second_lp).balance(&fixture.helper.address),
        0
    );
}
