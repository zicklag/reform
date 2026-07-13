# CRF Training Algorithms

The `crfs` crate provides 5 training algorithms, each with different tradeoffs.

## Summary

| Algorithm | Gradient | Regularization | Per-iteration cost | Accuracy | Speed |
|---|---|---|---|---|---|
| **L-BFGS** | Exact (FB) | L1 + L2 | Full dataset FB | Highest | Slowest |
| **L2SGD** | Exact (FB) | L2 (decay) | Single instance FB | High | Fast (large data) |
| **Averaged Perceptron** | Approx (Viterbi) | None (averaging) | Single instance Viterbi | Medium | Fastest |
| **AROW** | Approx (Viterbi) | Per-feature covariance | Single instance Viterbi | Medium-High | Fast |
| **Passive Aggressive** | Approx (Viterbi) | Margin + slack | Single instance Viterbi | Medium | Fast |

**Key distinction:** L-BFGS and L2SGD use **forward-backward** (exact gradient of log-likelihood). Averaged Perceptron, AROW, and Passive Aggressive use **Viterbi decoding** (approximate gradient via the predicted path). The former are more accurate per update but more expensive; the latter are faster but noisier.

---

## L-BFGS (`Trainer::lbfgs()`)

**Type:** Batch, second-order optimization

**Core idea:** Limited-memory BFGS — a quasi-Newton method that approximates the Hessian to find the optimal weights by minimizing the **negative log-likelihood** of the full dataset, with L1 and/or L2 regularization.

**How it works:**
- Runs a single optimization over the **entire dataset** each iteration
- Computes the exact gradient: `expected_counts - observed_counts` (from forward-backward) + regularization gradient
- Uses `liblbfgs` under the hood with configurable line search (More-Thuente, backtracking Armijo/Wolfe/strong Wolfe)
- L1 regularization (`c1 > 0`) switches to OWL-QN (orthant-wise L-BFGS) with forced backtracking line search

**Parameters:**
- `c1` — L1 regularization coefficient (default 0.0)
- `c2` — L2 regularization coefficient (default 1.0)
- `max_iterations` (default 100)
- `epsilon` — convergence tolerance on gradient norm (default 1e-5)
- `period` / `delta` — early stopping if `|fx - fx_prev| < delta` over `period` iterations
- `linesearch` — line search algorithm enum
- `num_memories` — stored (currently ignored, liblbfgs uses its default)
- `max_linesearch` — max line search steps per iteration

**Convergence:** Monotonic — each iteration decreases the objective. Converges to the global optimum (convex CRF objective). Most accurate, but slowest per-iteration due to full forward-backward on every instance.

**Best for:** Small-to-medium datasets where accuracy matters more than training speed. The default choice.

---

## Averaged Perceptron (`Trainer::averaged_perceptron()`)

**Type:** Online, first-order, mistake-driven

**Core idea:** A structural perceptron that updates weights only when the Viterbi prediction differs from the true labels, then averages all weight vectors at the end to reduce overfitting.

**How it works:**
- For each instance: Viterbi decode with current weights
- If prediction differs from truth: `w += (true_features - predicted_features) * weight`
- Tracks `summed_updates[i] += c * delta` for averaging
- After all epochs: `w[i] -= summed_updates[i] / c` (the "averaged" step)
- Uses `ScoreContext` (Viterbi only) — no forward-backward, no marginals

**Parameters:**
- `max_iterations` (default 100)
- `epsilon` — convergence threshold on per-instance error rate (default 1e-5)
- `shuffle_seed` — optional RNG seed

**Convergence:** No guarantee of monotonic decrease. Stops when average error rate < epsilon. Fast per-iteration (Viterbi only, no partition function).

**Best for:** Large datasets where speed is critical and some accuracy loss is acceptable. No regularization parameters to tune.

---

## AROW (`Trainer::arow()`)

**Type:** Online, confidence-weighted, second-order

**Core idea:** Adaptive Regularization of Weights — maintains a diagonal Gaussian confidence distribution over weights (`N(w, Σ)`). On each mistake, it updates both the mean and the covariance, scaling updates by feature-specific confidence.

**How it works:**
- Maintains `weights` (mean) and `covariance` (diagonal variance per feature, initialized to `variance`)
- For each instance: Viterbi decode, compute Hamming loss
- On mistake: compute `diff = true_features - predicted_features`
- `frac = gamma + Σ diff[i]² * covariance[i]`
- `alpha = cost / frac` (cost = `pred_score - true_score + num_diff`)
- Update: `w[i] += alpha * covariance[i] * diff[i]`
- Shrink covariance: `Σ[i] = 1 / (1/Σ[i] + diff[i]² / gamma)`
- Uses `ScoreContext` (Viterbi only)

**Parameters:**
- `variance` — initial diagonal covariance (default 1.0). Higher = more aggressive initial updates. Lower = more conservative.
- `gamma` — regularization parameter (default 1.0). Higher = covariance shrinks less → more adaptive per-feature learning rates. Lower = covariance shrinks more → closer to fixed learning rate.
- `max_iterations` (default 100)
- `epsilon` — convergence threshold (default 1e-5)
- `shuffle_seed` — optional RNG seed

**Convergence:** Online, no monotonic guarantee. Stops when average loss < epsilon.

**Best for:** Online learning scenarios where feature-specific confidence matters. More robust than plain perceptron because it adapts learning rates per feature via the covariance.

**Overfitting risk:** AROW has no explicit L2 penalty. After enough epochs, covariance collapses for common features, the model locks in, and it memorizes training noise. Early stopping or switching to Averaged Perceptron / L-BFGS with L2 is recommended if you see a large train/test accuracy gap.

---

## L2SGD (`Trainer::l2sgd()`)

**Type:** Online, stochastic gradient descent with L2 regularization

**Core idea:** SGD with a decaying learning rate `η = 1 / (λ · (t₀ + t))` and L2 weight decay. Uses forward-backward to compute the true gradient of the negative log-likelihood for each instance.

**How it works:**
- Calibration phase: runs a few random instances to find `t₀` (optimal learning rate schedule)
- For each instance:
  - `η = 1 / (λ · (t₀ + t))` — learning rate decays as 1/t
  - Weight decay: `w *= (1 - η · λ)` (L2 regularization via decay)
  - Forward-backward to get expected counts
  - `w += η · (observed - expected)`
- Tracks best weights by objective value (with L2 norm penalty)
- Convergence check: relative improvement over `period` epochs < `delta`
- Uses `ForwardBackwardContext` (full forward-backward)

**Parameters:**
- `c2` — L2 regularization coefficient (default 1.0)
- `max_iterations` (default 100)
- `period` — convergence check window (default 10)
- `delta` — relative improvement threshold (default 1e-5)
- `shuffle_seed` — optional RNG seed

**Convergence:** Online, noisy but converges in expectation. The calibration phase is critical — a bad `t₀` can cause divergence.

**Best for:** Large datasets where full-batch L-BFGS is too expensive, but you want the accuracy of forward-backward gradients (unlike perceptron/AROW which use Viterbi approximations).

---

## Passive Aggressive (`Trainer::passive_aggressive()`)

**Type:** Online, margin-based, mistake-driven

**Core idea:** Updates weights to achieve at least unit margin on the current instance, while staying as close as possible to the previous weights ("passive" when correct, "aggressive" when wrong). Three variants control slack.

**How it works:**
- For each instance: Viterbi decode, compute Hamming loss
- On mistake: `err = pred_score - true_score`
- `cost = err + (sqrt(num_diff) if error_sensitive else 1.0)`
- `diff = true_features - predicted_features`
- `norm_sq = ||diff||²`
- `tau` depends on PA variant:
  - **PA:** `tau = cost / norm_sq` (hard margin, no slack)
  - **PA-I:** `tau = min(C, cost / norm_sq)` (soft margin, caps update)
  - **PA-II:** `tau = cost / (norm_sq + 1/(2C))` (squared slack)
- `w += tau · diff` (optionally averaged)
- Uses `ScoreContext` (Viterbi only)

**Parameters:**
- `pa_type` — `Pa`, `PaI`, or `PaII` (default `PaI`)
- `c` — aggressiveness parameter C (default 1.0)
- `error_sensitive` — use `sqrt(num_diff)` instead of `1.0` in cost (default false)
- `averaging` — enable weight averaging (default false)
- `max_iterations` (default 100)
- `epsilon` — convergence threshold (default 1e-5)
- `shuffle_seed` — optional RNG seed

**Convergence:** Online, no monotonic guarantee. Stops when average loss < epsilon.

**Best for:** Fast online training where you want margin-based updates. PA-I is the default and usually the safest PA variant.
