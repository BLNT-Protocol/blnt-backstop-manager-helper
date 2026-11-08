# BLNT Backstop Manager Helper

This repository specifies a minimal Soroban orchestration helper for a BLNT V3
backstop-manager. It is intended to give the manager's current owner two
single-signature atomic operations:

- `deposit(first_blnt, second_blnt)` joins the configured BLNT:XLM and
  BLNT:USDC Comet V1.1 pools using independent BLNT budgets, then deposits all
  newly minted LP shares into the configured FirstLoss and SecondLoss tiers.
- `claim(first_min_lp_out, second_min_lp_out)` accepts an optional minimum for
  each tier, allowing FirstLoss, SecondLoss, or both to be claimed atomically.

The helper is an orchestration layer only. It has no owner, governance,
upgrade, arbitrary-call, token-transfer, or rescue authority and must not
retain assets after a successful call. The backstop-manager remains the sole
custodian and authorization boundary.

The normative design is in [`docs/SYSTEM_SPEC.md`](docs/SYSTEM_SPEC.md).

No production deployment is authorized or recorded yet.
