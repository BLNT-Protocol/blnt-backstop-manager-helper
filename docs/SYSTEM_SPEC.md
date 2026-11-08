# BLNT Backstop Manager Helper System Specification

## 1. Status and purpose

This document specifies an immutable Soroban helper for atomically operating
two YieldBlent backstop tiers through one existing BLNT V3 backstop-manager.

The helper exists only to compose already-authorized manager operations into
single calls. It does not custody capital independently, grant authority,
change backstop policy, or alter BLNT V3 contracts.

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
well as for allowed backstops, allowed pools, custody, Comet approvals,
deposits, and emissions claims. A nested manager rejection MUST roll back the
complete helper operation.

Changing the backstop-manager's owner or manager does not change the helper's
immutable owner. Operational deployment MUST therefore choose the helper owner
deliberately and ensure the manager grants it every scope required by the
intended helper operations. Deposits require the manager's high scope; claims
require its low scope.

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
maximum BLNT budgets:

- `first_blnt` funds the BLNT:XLM FirstLoss route;
- `second_blnt` funds the BLNT:USDC SecondLoss route.

Both values MUST be positive. The helper MUST NOT silently move budget between
tiers.

For each tier the helper MUST:

1. query the bound Comet pool's live total LP supply, BLNT reserve, and paired
   reserve;
2. derive the greatest positive LP output whose Comet V1.1 upward-rounded BLNT
   input does not exceed the corresponding BLNT budget;
3. derive the upward-rounded paired-asset input;
4. set the BLNT maximum to the exact derived BLNT input;
5. set the XLM or USDC maximum to 101% of the derived paired-asset input,
   rounded upward;
6. record the manager's LP-token balance;
7. call the manager's high-scope Comet join for the bound tier;
8. require the manager's LP-token balance to increase by exactly the expected
   LP output; and
9. call the manager's backstop deposit with exactly that LP output.

After each deposit, the manager's liquid balance of that LP token MUST equal
its pre-call balance. The manager's BLNT, XLM, and USDC balance reductions MUST
equal the amounts reported by the successful route. Unused BLNT budget remains
in manager custody.

Both tier routes form one atomic call tree. Any invalid input, arithmetic
failure, insufficient manager balance, join failure, unexpected token delta,
deposit failure, authorization failure, or allowlist failure MUST roll back
both routes completely.

The fixed 1% paired-asset ceiling limits execution movement relative to the
live Comet reserve composition. It does not establish an independent fair
market price. Off-chain clients SHOULD inspect the live reserve ratios before
requesting owner authorization.

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
5. execute the complete internal behavior of
   `deposit(first_blnt, second_blnt)`;
6. after both deposits succeed, execute the complete internal behavior of
   `claim(0, 0)`; and
7. return both the deposit and claim results.

The implementation SHOULD share private deposit and claim routines with the
two parameterized public entrypoints. It SHOULD NOT invoke itself through
nested public contract calls merely to reuse behavior.

The available-balance calculation MUST prevent capital already accounted as
harvestable for the operational manager from being consumed. Any BLNT left
unused because of proportional-join rounding remains liquid in manager
custody.

The zero claim minima are an intentional consequence of the no-argument API.
The deployed backstop exposes no production view that can preview the exact
claimable BLNT and resulting single-sided Comet LP output. Clients SHOULD
inspect the relevant Comet state before requesting owner authorization.

The entire combined operation MUST be one atomic call tree. A failure in
owner authorization, balance accounting, either quote, either Comet join,
either backstop deposit, either emissions claim, or final reconciliation MUST
roll back every effect, including all successful steps that preceded the
failure.

`DepositAndClaimResult` MUST contain one `DepositResult` and one `ClaimResult`.
The successful event MUST contain the same accounting.

## 8. Arithmetic and rounding

All amounts use seven-decimal token units. Contract code MUST use checked
integer arithmetic and MUST NOT use floating point.

Comet proportional joins use a fixed scale of `10^18`. Division that derives
required inputs MUST round upward exactly as Comet V1.1 does. Calculations that
derive the largest affordable LP output MUST round downward, then re-evaluate
the exact rounded inputs before invoking the manager.

A zero reserve, zero supply, zero derived LP output, negative value, overflow,
or inconsistent result MUST fail before the first manager mutation.

## 9. Custody and reconciliation

The helper MUST never receive BLNT, XLM, USDC, LP shares, backstop shares, or
emissions. Every nested operation acts on assets already held by the bound
backstop-manager, and every resulting position remains owned by that manager.

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
- acceptance or rejection under the manager's scoped-manager rules;
- positive, zero, negative, minimum-unit, and near-overflow inputs;
- Comet V1.1 upward rounding and greatest-affordable-output derivation;
- fixed 1% paired-asset ceilings in both token orderings;
- insufficient BLNT, XLM, and USDC balances;
- first-join, second-join, first-deposit, and second-deposit failure rollback;
- exact LP balance deltas and no liquid LP residue after deposit;
- unused BLNT remaining in manager custody;
- both emissions claims with independent minimum outputs;
- rollback when either emissions claim fails;
- no-argument available-BLNT calculation excluding manager-harvestable BLNT;
- exact floor/remainder equal split, including an odd raw BLNT unit;
- combined deposit-before-claim ordering and complete rollback for every
  failure point in either phase;
- combined-operation zero claim minima and unused-BLNT reconciliation;
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
