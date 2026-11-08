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

The helper has no stored owner and no administrative entrypoints. For every
mutating operation it MUST:

1. read the current owner from the bound backstop-manager;
2. require that address's authorization; and
3. pass that same address as `from` to every nested manager call.

The backstop-manager remains authoritative for owner authorization, allowed
backstops, allowed pools, custody, Comet approvals, deposits, and emissions
claims. Transferring manager ownership automatically changes which address can
use the helper; the helper configuration itself does not change.

The operational manager role does not authorize these two helper entrypoints.
Supporting an operational-manager caller would require an explicit caller
address or separate entrypoints and is outside this version.

## 3. Immutable configuration

The constructor MUST accept and store exactly:

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
amounts and manager ownership are intentionally read again when operations run.

## 4. Public interface

The intended public interface is:

```text
__constructor(manager, backstop, pool, first_lp, second_lp, blnt, xlm, usdc)
deposit(first_blnt: i128, second_blnt: i128) -> DepositResult
claim(first_min_lp_out: Option<i128>, second_min_lp_out: Option<i128>) -> ClaimResult
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

The exact claim signature is:

```text
claim(
    first_min_lp_out: Option<i128>,
    second_min_lp_out: Option<i128>,
) -> ClaimResult
```

`None` skips the corresponding tier. `Some(minimum)` selects the tier and sets
its minimum acceptable LP output. This supports FirstLoss only, SecondLoss
only, or both tiers without parallel arrays or caller-selected tier ordering.

`claim(first_min_lp_out, second_min_lp_out)` MUST:

1. require at least one selected tier;
2. reject a selected minimum when negative;
3. obtain and authenticate the manager's current owner;
4. when FirstLoss is selected, call its manager emissions claim with the
   supplied minimum;
5. when SecondLoss is selected, call its manager emissions claim with the
   supplied minimum;
6. return the actual LP output for each selected tier and `None` for each
   skipped tier; and
7. emit the selection, minima, and actual outputs.

All selected claims MUST be atomic. When both tiers are selected, failure of
either claim MUST roll back the other. A zero minimum is permitted when
deliberately selected by the owner.

`ClaimResult` MUST contain `Option<i128>` FirstLoss and SecondLoss LP outputs in
that order.

## 7. Arithmetic and rounding

All amounts use seven-decimal token units. Contract code MUST use checked
integer arithmetic and MUST NOT use floating point.

Comet proportional joins use a fixed scale of `10^18`. Division that derives
required inputs MUST round upward exactly as Comet V1.1 does. Calculations that
derive the largest affordable LP output MUST round downward, then re-evaluate
the exact rounded inputs before invoking the manager.

A zero reserve, zero supply, zero derived LP output, negative value, overflow,
or inconsistent result MUST fail before the first manager mutation.

## 8. Custody and reconciliation

The helper MUST never receive BLNT, XLM, USDC, LP shares, backstop shares, or
emissions. Every nested operation acts on assets already held by the bound
backstop-manager, and every resulting position remains owned by that manager.

The helper MUST have no token approval, transfer, or rescue entrypoint. Any
tokens sent directly to the helper are outside the supported design and cannot
be recovered through it.

## 9. Storage and TTL

Only immutable configuration is stored. Every public operation MUST extend the
instance TTL using the repository's documented threshold and bump values.
There is no persistent per-user accounting.

## 10. Events and observability

Successful deposits and claims MUST emit compact, deterministic events
containing enough information to reconcile every manager and backstop balance
delta. Events are observability only and confer no rights.

## 11. Required tests

Tests MUST cover:

- constructor validation for every binding and both Comet token orderings;
- current-owner authorization and ownership transfer;
- rejection of operational-manager-only authorization;
- positive, zero, negative, minimum-unit, and near-overflow inputs;
- Comet V1.1 upward rounding and greatest-affordable-output derivation;
- fixed 1% paired-asset ceilings in both token orderings;
- insufficient BLNT, XLM, and USDC balances;
- first-join, second-join, first-deposit, and second-deposit failure rollback;
- exact LP balance deltas and no liquid LP residue after deposit;
- unused BLNT remaining in manager custody;
- FirstLoss-only, SecondLoss-only, and two-tier emissions claims with
  independent minimum outputs;
- rejection when no tier is selected and rollback when either selected claim
  fails;
- substituted manager, backstop, pool, tier, LP, or asset rejection; and
- resource usage within current Soroban transaction limits.

## 12. Deployment requirements

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
