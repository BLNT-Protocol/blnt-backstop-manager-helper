# Repository Instructions

These instructions apply to the entire repository.

## Mission

Build a minimal, immutable Soroban helper that lets the current owner of one
bound BLNT V3 backstop-manager atomically:

- join the canonical BLNT:XLM and BLNT:USDC Comet V1.1 pools and deposit the
  resulting LP shares into the configured FirstLoss and SecondLoss tiers; and
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
- Every mutating entrypoint MUST require authorization from the
  backstop-manager's current owner and pass that same address to the manager.
- `deposit(first_blnt, second_blnt)` MUST treat its arguments as independent
  BLNT budgets for the configured FirstLoss and SecondLoss Comet joins.
- Both Comet joins and both backstop deposits MUST succeed or roll back
  atomically. No newly minted LP shares may remain liquid in the manager after
  success.
- `claim(first_min_lp_out, second_min_lp_out)` MUST accept one optional minimum
  per tier, claim exactly the selected tiers, and roll all selected claims back
  atomically if any selected claim fails.
- The constructor MUST bind one manager, backstop, pool, BLNT:XLM LP,
  BLNT:USDC LP, BLNT, XLM, and USDC configuration and validate the complete
  relationship before storing it.
- The helper MUST reproduce Comet V1.1 proportional-join rounding and enforce
  the system specification's fixed counterasset ceiling.
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
   zero and boundary inputs, single-tier and two-tier claims, insufficient
   balances, slippage, partial-route failures, balance reconciliation, and
   atomic rollback.
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
