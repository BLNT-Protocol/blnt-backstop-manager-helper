# BLNT Backstop Manager Helper System Specification

## 1. Status and purpose

This document specifies an immutable Soroban helper for atomically operating
two YieldBlent backstop tiers through one existing BLNT V3 backstop-manager.

The helper exists only to compose already-authorized manager and Comet
operations into single calls. It does not retain capital independently, grant
authority, change backstop policy, or alter BLNT V3 contracts.

The initial production use case binds:

- one existing backstop-manager;
- the BLNT V3 backstop;
- the YieldBlent pool;
- FirstLoss backed by the canonical BLNT:XLM Comet V1.1 LP;
- SecondLoss backed by the canonical BLNT:USDC Comet V1.1 LP;
- canonical BLNT, XLM, and USDC token contracts.

Network-specific deployments may use different addresses only when the
deployment record identifies them unambiguously and validates the same
relationships.

## 2. Trust and authority model

The helper has one immutable configured owner and no administrative
entrypoints. For every mutating operation it MUST:

1. read its immutable configured owner;
2. require that address's authorization; and
3. pass that same address as `from` to every nested manager call.

The helper constructor does not require its owner to equal the manager's
current owner. The backstop-manager remains authoritative for deciding whether
that address is its owner or a sufficiently scoped operational manager, as
well as for allowed backstops, allowed pools, custody, deposits, and emissions
claims. A nested manager rejection MUST roll back the complete helper
operation.

Changing the backstop-manager's owner or manager does not change the helper's
immutable owner. Operational deployment MUST therefore choose the helper owner
deliberately and ensure the manager grants it every authority required by the
intended helper operations. Single-sided deposits require the manager owner
because they use the manager's owner-only `transfer_token` entrypoint. Claims
require only the manager's low scope.

## 3. Immutable configuration

The constructor MUST accept and store exactly:

- `owner`;
- `manager`;
- `backstop`;
- `pool`;
- `first_lp`;
- `second_lp`;
- `blnt`;
- `xlm`; and
- `usdc`.

Before storing configuration, construction MUST verify:

1. all addresses are distinct wherever their roles require distinct contracts
   or assets;
2. BLNT, XLM, and USDC expose the expected seven-decimal token interface;
3. the manager's allowed backstop list contains `backstop`;
4. the manager's allowed pool list contains `pool`;
5. `backstop.backstop_token(FirstLoss, pool) == first_lp`;
6. `backstop.backstop_token(SecondLoss, pool) == second_lp`;
7. `first_lp` contains exactly BLNT and XLM;
8. `second_lp` contains exactly BLNT and USDC; and
9. both LP contracts expose positive reserves and total supply.

A mismatch MUST reject construction. The bindings are immutable. Live reserve
amounts are intentionally read again when operations run.

## 4. Public interface

The intended public interface is:

```text
__constructor(owner, manager, backstop, pool, first_lp, second_lp, blnt, xlm, usdc)
deposit(first_blnt: i128, second_blnt: i128) -> DepositResult
claim(first_min_lp_out: i128, second_min_lp_out: i128) -> ClaimResult
deposit_and_claim() -> DepositAndClaimResult
get_config() -> Config
```

The correctly spelled entrypoint is `deposit`.

No public method may replace configuration, transfer tokens, rescue assets,
perform arbitrary calls, or change ownership.

## 5. Deposit operation

`deposit(first_blnt, second_blnt)` treats its two arguments as independent
exact BLNT inputs:

- `first_blnt` funds the BLNT:XLM FirstLoss route;
- `second_blnt` funds the BLNT:USDC SecondLoss route.

Both values MUST be positive. The helper MUST NOT silently move budget between
tiers.

Before either tier is mutated, the helper MUST verify that the manager's liquid
BLNT balance less its recorded harvestable BLNT can cover both inputs. It MUST
NOT consume manager-harvestable BLNT.

For each tier the helper MUST:

1. query the bound Comet pool's live total LP supply, BLNT reserve, normalized
   BLNT weight, and swap fee;
2. derive a positive conservative minimum LP output from that state;
3. call the manager's owner-only `transfer_token` entrypoint to move exactly
   the tier's BLNT input temporarily to the helper;
4. authorize only the bound Comet pool to pull that exact BLNT amount;
5. call Comet V1.1's BLNT-only
   `dep_tokn_amt_in_get_lp_tokns_out` entrypoint with the derived minimum;
6. require the returned LP output to be positive and equal the helper's actual
   LP-token increase;
7. transfer exactly those LP shares to the manager;
8. call the manager's backstop deposit with exactly that LP output; and
9. require the helper's BLNT and LP balances and the manager's liquid LP
   balance to return to their respective pre-route values.

The route MUST consume no XLM or USDC. `TierDepositResult.paired_in` is retained
for ABI compatibility and MUST be zero.

Both tier routes form one atomic call tree. Any invalid input, arithmetic
failure, insufficient non-harvestable manager BLNT, Comet failure, unexpected token delta,
deposit failure, authorization failure, or allowlist failure MUST roll back
both routes completely.

The execution-derived minimum protects against zero output, arithmetic
divergence, and an unexpectedly weak result relative to the live Comet state.
Because the no-argument combined operation intentionally derives its values at
execution time, it is not an independent fair-market-price guarantee or a
caller-supplied pre-inclusion slippage bound. Off-chain clients SHOULD inspect
the live reserve composition before requesting owner authorization.

`DepositResult` MUST report, for each tier, the BLNT consumed, paired asset
consumed, LP shares minted, and backstop shares received. A successful event
MUST contain the same accounting.

## 6. Claim operation

`claim(first_min_lp_out, second_min_lp_out)` MUST:

1. reject either minimum when negative;
2. authenticate the helper's configured owner;
3. call the manager's FirstLoss emissions claim with
   `first_min_lp_out`;
4. call the manager's SecondLoss emissions claim with
   `second_min_lp_out`;
5. return both actual LP outputs; and
6. emit both minima and both actual outputs.

The two claims MUST be atomic. Failure of either claim MUST roll back the other.
A zero minimum is permitted when deliberately selected by the owner. A caller
that wants to claim only one tier SHOULD invoke that tier directly through the
backstop-manager instead of using this helper.

`ClaimResult` MUST contain the FirstLoss and SecondLoss LP outputs in that
order.

## 7. Combined deposit and claim operation

`deposit_and_claim()` MUST accept no arguments. It is the single-click owner
operation for consuming the manager's available BLNT across both tiers and
then claiming both tiers.

The operation MUST:

1. authenticate the helper's configured owner once;
2. read the manager's current BLNT token balance and its manager-harvestable
   BLNT accounting;
3. define available BLNT as liquid BLNT balance less harvestable BLNT and fail
   if that subtraction is negative or leaves too little value to derive a
   positive join for both tiers;
4. set `first_blnt = floor(available_blnt / 2)` and
   `second_blnt = available_blnt - first_blnt`, assigning any odd raw unit to
   SecondLoss;
5. execute the complete internal single-sided behavior of
   `deposit(first_blnt, second_blnt)`;
6. after both deposits succeed, execute the complete internal behavior of
   `claim(0, 0)`; and
7. return both the deposit and claim results.

The implementation SHOULD share private deposit and claim routines with the
two parameterized public entrypoints. It SHOULD NOT invoke itself through
nested public contract calls merely to reuse behavior.

The available-balance calculation MUST prevent capital already accounted as
harvestable for the operational manager from being consumed. The complete
available amount is assigned across the two exact single-sided inputs.

The zero claim minima are an intentional consequence of the no-argument API.
The deployed backstop exposes no production view that can preview the exact
claimable BLNT and resulting single-sided Comet LP output. Clients SHOULD
inspect the relevant Comet state before requesting owner authorization.

The entire combined operation MUST be one atomic call tree. A failure in
owner authorization, balance accounting, either quote, either Comet deposit,
either backstop deposit, either emissions claim, or final reconciliation MUST
roll back every effect, including all successful steps that preceded the
failure.

`DepositAndClaimResult` MUST contain one `DepositResult` and one `ClaimResult`.
The successful event MUST contain the same accounting.

## 8. Arithmetic and rounding

All amounts use seven-decimal token units. Contract code MUST use checked
integer arithmetic and MUST NOT use floating point.

The single-sided minimum MUST be conservative relative to Comet V1.1's live
BLNT reserve, normalized weight, swap fee, and total LP supply. Intermediate
products MUST use 256-bit checked arithmetic. The derived minimum MUST include
a fixed 1% execution margin and MUST remain positive.

For an exact BLNT input `I`, let `S` be total LP supply, `B` the BLNT reserve,
`W` the normalized BLNT weight, and `F` the swap fee, with weights and fees
scaled by `10^7`. The helper MUST calculate, in order and with floor rounding:

1. `taxable_fee = floor((10^7 - W) * F / 10^7)`;
2. `after_fee = floor(I * (10^7 - taxable_fee) / 10^7)`;
3. `weighted_in = floor(after_fee * W / 10^7)`;
4. `denominator = B + after_fee`;
5. `lower_bound = floor(S * weighted_in / denominator)`; and
6. `minimum = floor(lower_bound * 9,900 / 10,000)`.

The lower bound follows from `(1 + x)^W - 1 >= W*x/(1+x)` for normalized
`0 < W < 1` and `x >= 0`. Comet remains authoritative for its maximum-input
ratio and exact mint calculation; a Comet rejection MUST roll back the full
helper operation.

A zero reserve, zero supply, zero derived LP output, negative value, overflow,
or inconsistent result MUST fail before the first manager mutation.

## 9. Custody and reconciliation

The helper MAY hold the exact BLNT input and newly minted LP shares only during
an active atomic call. Every successful route MUST restore its BLNT and LP
balances to their pre-call values. The helper MUST never receive XLM, USDC,
backstop shares, or emissions, and every resulting position remains owned by
the manager.

The helper MUST have no token approval, transfer, or rescue entrypoint. Any
tokens sent directly to the helper are outside the supported design and cannot
be recovered through it.

## 10. Storage and TTL

Only immutable configuration is stored. Every public operation MUST extend the
instance TTL when fewer than 100 days remain, restoring it to 120 days. There
is no persistent per-user accounting.

## 11. Events and observability

Successful deposits and claims MUST emit compact, deterministic events
containing enough information to reconcile every manager and backstop balance
delta. Events are observability only and confer no rights.

## 12. Required tests

Tests MUST cover:

- constructor validation for every binding and both Comet token orderings;
- configured-owner authorization independent of manager ownership changes;
- claim acceptance or rejection under the manager's scoped-manager rules and
  owner-only rejection of deposits after manager ownership changes;
- positive, zero, negative, minimum-unit, and near-overflow inputs;
- conservative single-sided minimum derivation in both token orderings;
- insufficient non-harvestable BLNT;
- first-Comet, second-Comet, first-backstop-deposit, and
  second-backstop-deposit failure rollback;
- exact BLNT and LP balance deltas with no operation residue in the helper or
  manager after deposit;
- zero XLM and USDC consumption;
- both emissions claims with independent minimum outputs;
- rollback when either emissions claim fails;
- no-argument available-BLNT calculation excluding manager-harvestable BLNT;
- exact floor/remainder equal split, including an odd raw BLNT unit;
- combined deposit-before-claim ordering and complete rollback for every
  failure point in either phase;
- combined-operation zero claim minima and exact available-BLNT consumption;
- substituted manager, backstop, pool, tier, LP, or asset rejection; and
- resource usage within current Soroban transaction limits.

## 13. Deployment requirements

Production deployment requires:

1. an optimized reproducible WASM build using Rust 1.91.1, Soroban SDK 27.0.3,
   and `wasm32v1-none`;
2. embedded
   `source_repo=github:BLNT-Protocol/blnt-backstop-manager-helper` metadata;
3. recorded source commit, optimized WASM hash, constructor arguments,
   transaction hash, contract address, and network passphrase;
4. independent verification of all immutable bindings against live contracts;
5. successful simulation and resource review of both full two-tier operations;
   and
6. explicit owner review of live Comet reserve ratios before first use.

No deployment is authorized by this specification alone.
