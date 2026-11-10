# BLNT Backstop Manager Helper

This repository specifies a minimal Soroban orchestration helper for a BLNT V3
backstop-manager. It is intended to give one independently configured owner
three single-signature atomic operations, subject to the manager's own
authorization rules:

- `deposit(first_blnt, second_blnt)` uses the manager's non-harvestable BLNT
  for independent BLNT-only deposits into the configured BLNT:XLM and
  BLNT:USDC Comet V1.1 pools, then deposits all newly minted LP shares into the
  configured FirstLoss and SecondLoss tiers. No XLM or USDC is required.
- `claim(first_min_lp_out, second_min_lp_out)` claims both tiers atomically with
  independent required minimum LP outputs. A single tier can be claimed
  directly through the backstop-manager instead.
- `deposit_and_claim()` takes no arguments, splits the manager's available
  non-harvestable BLNT equally across both single-sided deposits, and then
  claims both tiers. Any failure rolls back the complete operation.

The helper is an orchestration layer only. Its immutable configured owner may
request the bound operations, but the backstop-manager independently accepts
or rejects that address under its own owner and scoped-manager rules. The
deposit operations require that address to remain the manager owner, while
claims require only low-scope manager authority. The helper has no
ownership-transfer, governance, upgrade, arbitrary-call, public token-transfer,
or rescue authority and must not retain assets after a successful call. The
backstop-manager remains the sole long-lived custodian and final authorization
boundary.

The normative design is in [`docs/SYSTEM_SPEC.md`](docs/SYSTEM_SPEC.md).

Production deployments must use the exact optimized WASM published and
attested by a tagged GitHub release. A deployment is not complete until its
constructor bindings, on-chain bytes, source metadata, and explorer source
linkage have been independently verified and recorded by the deployment
orchestration repository.
