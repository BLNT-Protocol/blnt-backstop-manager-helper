# BLNT Backstop Manager Helper

This repository specifies a minimal Soroban orchestration helper for a BLNT V3
backstop-manager. It is intended to give one independently configured owner
three single-signature atomic operations, subject to the manager's own
authorization rules:

- `deposit(first_blnt, second_blnt)` joins the configured BLNT:XLM and
  BLNT:USDC Comet V1.1 pools using independent BLNT budgets, then deposits all
  newly minted LP shares into the configured FirstLoss and SecondLoss tiers.
- `claim(first_min_lp_out, second_min_lp_out)` claims both tiers atomically with
  independent required minimum LP outputs. A single tier can be claimed
  directly through the backstop-manager instead.
- `deposit_and_claim()` takes no arguments, splits the manager's available
  non-harvestable BLNT equally across both tiers, executes the two deposits,
  and then claims both tiers. Any failure rolls back the complete operation.

The helper is an orchestration layer only. Its immutable configured owner may
request the bound operations, but the backstop-manager independently accepts
or rejects that address under its own owner and scoped-manager rules. The
helper has no ownership-transfer, governance, upgrade, arbitrary-call,
token-transfer, or rescue authority and must not retain assets after a
successful call. The backstop-manager remains the sole custodian and final
authorization boundary.

The normative design is in [`docs/SYSTEM_SPEC.md`](docs/SYSTEM_SPEC.md).

No production deployment is authorized or recorded yet.
