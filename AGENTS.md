# Repository Instructions

These instructions apply to the entire repository.

## Mission

Build a minimal, immutable Soroban helper that lets one independently
configured owner operate a bound BLNT V3 backstop-manager atomically, subject
to the manager's own authorization rules:

- make BLNT-only deposits into the canonical BLNT:XLM and BLNT:USDC Comet
  V1.1 pools and deposit the resulting LP shares into the configured FirstLoss
  and SecondLoss tiers; and
- claim emissions from both configured tiers.

This repository is financial smart-contract software. Prefer explicit
invariants, checked integer arithmetic, narrow authority, deterministic
accounting, and atomic rollback over convenience.

## Sources of truth

Read these before changing behavior:

1. `docs/SYSTEM_SPEC.md`
2. The pinned backstop-manager ABI and implementation used by the deployment
3. The deployed BLNT V3 backstop and Comet V1.1 ABIs
4. Existing tests and the implementation being changed

If prose conflicts with deployed executable behavior, stop and document the
conflict. Do not silently invent new economic policy.

## Critical invariants

- The helper MUST hold no user funds or backstop positions after a successful
  operation.
- The helper MUST NOT own, manage, upgrade, or recover the backstop-manager.
- Every mutating entrypoint MUST require authorization from the helper's
  immutable configured owner and pass that same address to the manager. The
  manager remains solely responsible for accepting or rejecting that address
  under its own owner and scoped-manager rules.
- `deposit(first_blnt, second_blnt)` MUST treat its arguments as independent
  exact BLNT inputs for the configured FirstLoss and SecondLoss single-sided
  Comet deposits.
- Deposits MUST exclude manager-harvestable BLNT and require the helper owner
  to remain the manager owner because the route uses the manager's owner-only
  token transfer.
- Both Comet deposits and both backstop deposits MUST succeed or roll back
  atomically. No operation BLNT or newly minted LP shares may remain liquid in
  the helper or manager after success.
- `claim(first_min_lp_out, second_min_lp_out)` MUST require one minimum per
  tier, claim both tiers, and roll both claims back atomically if either claim
  fails.
- `deposit_and_claim()` MUST accept no arguments, exclude manager-harvestable
  BLNT from the available balance, split the remaining BLNT equally across the
  two tiers, execute deposit before claim, and roll the entire call tree back
  if any step fails.
- The constructor MUST bind one owner, manager, backstop, pool, BLNT:XLM LP,
  BLNT:USDC LP, BLNT, XLM, and USDC configuration and validate the complete
  relationship before storing it.
- The helper MUST derive the system specification's conservative minimum LP
  output from live Comet V1.1 state and consume no counterasset.
- Never use floating-point arithmetic. Use checked integer arithmetic and
  explicit fixed-point scales.
- Never disable release overflow checks.
- Bound loops and ledger footprints. Both operations must remain executable
  under Soroban transaction limits.
- The helper MUST have no governance, ownership, upgrade, arbitrary-call,
  token-transfer, or rescue entrypoint.

## Development workflow

For contract changes:

1. Update `docs/SYSTEM_SPEC.md` first when behavior changes.
2. Add or update focused native tests.
3. Test authorization, configuration substitution, token ordering, rounding,
   zero and boundary inputs, two-tier claims, combined-operation splitting,
   harvestable-balance exclusion, insufficient balances, single-sided minimums,
   partial-route failures, balance reconciliation, and atomic rollback.
4. Build optimized `wasm32v1-none` WASM with Rust 1.91.1 and Soroban SDK
   27.0.3.
5. Verify `source_repo=github:BLNT-Protocol/blnt-backstop-manager-helper`
   metadata before recording or deploying an artifact.

## Repository hygiene

- Use `apply_patch` for authored changes.
- Do not commit `target/`, secrets, private keys, account configuration, or
  unreviewed deployment records.
- Keep the lockfile committed and builds reproducible from a fresh clone.
- Preserve AGPL-3.0 notices.
- Do not describe the contract as audited or production-ready.
- Do not commit, push, tag, deploy, publish, or alter external protocol state
  unless the user explicitly requests it.
