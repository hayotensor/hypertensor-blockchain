#!/usr/bin/env python3
"""Rebuild the Matrix README's static figures with Matplotlib.

Documentation examples only: this is not runtime or benchmark code. Formula sources:
../math.rs (normalization, accuracy, reputation_rates) and ../settlement.rs.
Run from any directory; --preview-dir additionally writes PNGs for visual inspection.
"""
from __future__ import annotations

import argparse
import json
from fractions import Fraction
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.ticker import PercentFormatter
import numpy as np

Q = 10**18
TOLERANCE = Q // 10
MAX_GAIN = Q // 10
MAX_LOSS = 3 * Q // 10
MIN_REPUTATION = Q // 10
ROOT = Path(__file__).resolve().parent
COLORS = {"gain": "#047857", "loss": "#c2410c", "blue": "#2563eb", "purple": "#7c3aed", "ink": "#172033"}
plt.rcParams.update({
    "font.family": "DejaVu Sans", "font.size": 12, "axes.titlesize": 15,
    "axes.labelsize": 12, "figure.facecolor": "white", "axes.facecolor": "white",
    "text.color": COLORS["ink"], "axes.labelcolor": COLORS["ink"],
    "axes.spines.top": False, "axes.spines.right": False,
    "svg.fonttype": "none", "svg.hashsalt": "matrix-consensus-guide",
})


def normalize(values):
    total = sum(values.values())
    if total == 0:
        return dict.fromkeys(values, 0)
    shares = {key: value * Q // total for key, value in values.items()}
    remaining = Q - sum(shares.values())
    order = sorted(values, key=lambda key: (-(values[key] * Q % total), key))
    for key in order[:remaining]:
        shares[key] += 1
    return shares


def rates(distance, tolerance=TOLERANCE, maximum_gain=MAX_GAIN, maximum_loss=MAX_LOSS):
    """Q18 rates, preserving the runtime's one-division quadratic rounding."""
    if distance == 0:
        return maximum_gain, 0
    if distance < tolerance:
        return maximum_gain * (tolerance - distance)**2 // tolerance**2, 0
    if distance > tolerance:
        return 0, maximum_loss * (distance - tolerance)**2 // (Q - tolerance)**2
    return 0, 0


def next_reputation(previous, distance):
    gain, loss = rates(distance)
    value = min(Q, previous + previous * gain // Q)
    return value - value * loss // Q


def worked_example():
    # All three Active nodes reveal and receive assessments. Default stake transforms.
    stake = {"A": 50, "B": 30, "C": 20}
    rows = {
        "A": {"B": 8 * Q // 10, "C": 2 * Q // 10},
        "B": {"A": 6 * Q // 10, "C": 4 * Q // 10},
        "C": {"A": 2 * Q // 10, "B": 8 * Q // 10},
    }
    performance, references, accuracy = {}, {}, {}
    for node in stake:
        voters = [v for v in stake if v != node]
        performance[node] = sum(stake[v] * rows[v][node] for v in voters) // sum(stake[v] for v in voters)
        reference = {}
        for target in rows[node]:
            others = [v for v in stake if v != node and v != target]
            reference[target] = sum(stake[v] * rows[v][target] for v in others) // sum(stake[v] for v in others)
        references[node] = reference
        accuracy[node] = Q - sum(abs(value - references[node][target]) for target, value in rows[node].items()) // len(reference)
    # Every node in this example has an external weighted assessment. The runtime
    # also includes assessed Active workers who do not submit rows (accuracy zero).
    eligible = [node for node in stake if any(stake[v] > 0 for v in rows if v != node)]
    a_share = normalize({node: accuracy[node] for node in eligible})
    p_share = normalize({node: performance[node] for node in eligible})
    weight = normalize({node: (Q // 2) * a_share[node] + (Q // 2) * p_share[node] for node in stake})
    # Independent exact-rational checks for the worked example (within Q18 rounding).
    for node, expected in {"A": Fraction(11, 25), "B": Fraction(4, 5), "C": Fraction(11, 40)}.items():
        assert performance[node] == expected * Q
    for node, expected in {"A": Fraction(9, 10), "B": Fraction(7, 10), "C": Fraction(4, 5)}.items():
        assert abs(Fraction(accuracy[node], Q) - expected) <= Fraction(1, Q)
    assert sum(weight.values()) == Q
    return {"stake": stake, "rows": rows, "references": references, "performance": performance,
            "accuracy": accuracy, "accuracy_share": a_share, "performance_share": p_share, "weight": weight}


def save(fig, name, description, preview_dir):
    fig.savefig(ROOT / f"{name}.svg", bbox_inches="tight", metadata={"Date": None, "Title": name.replace("-", " "), "Description": description})
    if preview_dir:
        preview_dir.mkdir(parents=True, exist_ok=True)
        fig.savefig(preview_dir / f"{name}.png", dpi=140, bbox_inches="tight")
    plt.close(fig)


def scoring_figure(example, preview_dir):
    names = list(example["stake"])
    data = np.full((3, 3), np.nan)
    for row, node in enumerate(names):
        for col, target in enumerate(names):
            if node != target:
                data[row, col] = example["rows"][node][target] / Q
    fig, ax = plt.subplots(figsize=(8.8, 5.4), layout="constrained")
    palette = plt.colormaps["Blues"].copy()
    palette.set_bad("#f1f5f9")
    im = ax.imshow(data, vmin=0, vmax=1, cmap=palette)
    ax.set_xticks(range(3), names)
    ax.set_yticks(range(3), [f"{name}   |   voting weight {example['stake'][name]}" for name in names])
    ax.set_xlabel("Node being scored (column)")
    ax.set_ylabel("Node submitting scores (row)")
    ax.set_title("Absolute scores in a fully assessed example\nPartial rows are allowed; row sums are unrestricted", pad=15)
    for row in range(3):
        for col in range(3):
            value = data[row, col]
            ax.text(col, row, "No self-score" if np.isnan(value) else f"{value:.0%}", ha="center", va="center",
                    color="white" if value >= 0.6 else COLORS["ink"], fontsize=13)
    fig.colorbar(im, ax=ax, format=PercentFormatter(1), label="Absolute score on the subnet scale", shrink=0.85)
    save(fig, "score-matrix", "Three-node absolute score matrix with frozen voting weights 50, 30, 20. Diagonal entries are excluded, not zero votes.", preview_dir)


def emissions_figure(example, preview_dir):
    names = list(example["stake"])
    fig, axes = plt.subplots(1, 2, figsize=(12, 4.9), layout="constrained")
    positions = np.arange(3)
    a = np.array([example["accuracy_share"][n] / Q for n in names])
    p = np.array([example["performance_share"][n] / Q for n in names])
    axes[0].bar(positions - 0.18, a, width=0.36, color=COLORS["blue"], label="Normalized accuracy share")
    axes[0].bar(positions + 0.18, p, width=0.36, color=COLORS["purple"], label="Normalized performance share")
    axes[0].set_title("Normalize each factor separately")
    axes[0].legend(loc="upper left", fontsize=10)
    axes[1].bar(positions, a / 2, color=COLORS["blue"], label="Accuracy contribution (50%)")
    axes[1].bar(positions, p / 2, bottom=a / 2, color=COLORS["purple"], label="Performance contribution (50%)")
    for i, node in enumerate(names):
        axes[1].text(i, example["weight"][node] / Q + 0.015, f"{example['weight'][node] / Q:.2%}", ha="center")
    axes[1].set_title("Blend into the final emission share")
    axes[1].legend(loc="upper left", fontsize=10)
    for ax in axes:
        ax.set_xticks(positions, names)
        ax.set_xlabel("Assessed Active node")
        ax.set_ylim(0, 0.78)
        ax.yaxis.set_major_formatter(PercentFormatter(1))
        ax.grid(axis="y", alpha=0.2)
        ax.set_axisbelow(True)
    axes[0].set_ylabel("Share within the eligible reward cohort")
    save(fig, "emission-weights", "Worked example: each factor normalizes across assessed Active nodes before a 50/50 blend; final shares sum to 100%.", preview_dir)


def rate_figure(preview_dir):
    fig, axes = plt.subplots(1, 2, figsize=(12, 5.0), layout="constrained")
    for ax, end in zip(axes, [100, 15]):
        distances = [i * Q // 1000 for i in range(end * 10 + 1)]
        x = np.array(distances) / Q
        y = np.array([(lambda r: r[0] - r[1])(rates(d)) / Q for d in distances])
        ax.plot(x, y, color=COLORS["blue"], linewidth=2.5)
        ax.fill_between(x, y, 0, where=y >= 0, color=COLORS["gain"], alpha=0.18)
        ax.fill_between(x, y, 0, where=y <= 0, color=COLORS["loss"], alpha=0.16)
        ax.axhline(0, color="#64748b", linewidth=0.8)
        ax.axvline(0.1, color="#64748b", linestyle="--", linewidth=1.2)
        ax.xaxis.set_major_formatter(PercentFormatter(1))
        ax.yaxis.set_major_formatter(PercentFormatter(1))
        ax.set_xlabel("Disagreement d = 1 − accuracy")
        ax.set_ylabel("Change as a fraction of current reputation")
        ax.grid(alpha=0.15)
    axes[0].set_title("The complete reputation curve")
    axes[0].set_xlim(0, 1)
    axes[0].set_ylim(-0.33, 0.13)
    for d, label, xytext in [(0, "+10%", (0.05, 0.105)), (0.55, "−7.5%", (0.60, -0.07)), (1, "−30%", (0.79, -0.29))]:
        gain, loss = rates(round(d * Q))
        axes[0].plot(d, (gain - loss) / Q, "o", color=COLORS["blue"])
        axes[0].annotate(label, (d, (gain - loss) / Q), xytext=xytext, arrowprops={"arrowstyle": "-", "color": "#64748b"})
    axes[1].set_title("Close-up around the 10% tolerance")
    axes[1].set_xlim(0, 0.15)
    axes[1].set_ylim(-0.008, 0.12)
    axes[1].annotate("5% disagreement\n→ +2.5% reputation", (0.05, 0.025), (0.064, 0.065), fontsize=11,
                     arrowprops={"arrowstyle": "->", "color": "#64748b"})
    axes[1].annotate("10%: no accuracy change", (0.1, 0), (0.035, 0.011), fontsize=10,
                     arrowprops={"arrowstyle": "->", "color": "#64748b"})
    save(fig, "reputation-rates", "Default tolerance 10%, maximum gain 10%, maximum loss 30%. Signed rates are fractions of current reputation; other penalties are excluded.", preview_dir)


def history_figure(preview_dir):
    fig, ax = plt.subplots(figsize=(11.3, 5.6), layout="constrained")
    cases = [(0, COLORS["gain"], "0% disagreement: +10% / round"),
             (Q // 20, COLORS["blue"], "5% disagreement: +2.5% / round"),
             (Q // 10, "#64748b", "10% disagreement: unchanged"),
             (11 * Q // 20, "#d97706", "55% disagreement: −7.5% / round"),
             (Q, COLORS["loss"], "100% disagreement: −30% / round")]
    removals = {}
    for distance, color, label in cases:
        history = [4 * Q // 5]
        for _ in range(30):
            if history[-1] < MIN_REPUTATION:
                break
            history.append(next_reputation(history[-1], distance))
        x = list(range(len(history)))
        y = [v / Q for v in history]
        ax.plot(x, y, color=color, linewidth=2.3, label=label,
                linestyle="--" if distance == Q // 10 else "-")
        if history[-1] < MIN_REPUTATION:
            ax.plot(x[-1], y[-1], marker="X", markersize=10, color=color)
            ax.annotate(f"Removal scheduled\nafter round {x[-1]}", (x[-1], y[-1]),
                        (x[-1] - 2, y[-1] + 0.16), fontsize=10,
                        arrowprops={"arrowstyle": "->", "color": color})
            removals[str(distance)] = x[-1]
    ax.axhline(0.1, color="#64748b", linewidth=1, linestyle=":")
    ax.text(0.4, 0.115, "Default minimum reputation: 10%", fontsize=10, color="#475569")
    ax.set_title("Repeated qualifying rounds, starting at 80% reputation\nConstant disagreement; no other penalties; curves stop at removal", pad=14)
    ax.set_xlabel("Completed rounds that qualify for reputation updates")
    ax.set_ylabel("Node reputation")
    ax.set_xlim(0, 30)
    ax.set_ylim(0, 1.07)
    ax.yaxis.set_major_formatter(PercentFormatter(1))
    ax.grid(alpha=0.15)
    ax.legend(loc="center right", fontsize=10, framealpha=0.97)
    assert removals[str(Q)] == 6
    assert removals[str(11 * Q // 20)] == 27
    save(fig, "reputation-over-rounds", "Illustration with default parameters and minimum reputation. No other penalties, policy changes or pauses. Removal happens strictly below 10%; no subsequent reputation trajectory is projected.", preview_dir)
    return removals


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--preview-dir", type=Path)
    args = parser.parse_args()
    for distance, expected in [(0, (Q // 10, 0)), (Q // 20, (Q // 40, 0)),
                               (Q // 10, (0, 0)), (11 * Q // 20, (0, 3 * Q // 40)), (Q, (0, 3 * Q // 10))]:
        assert rates(distance) == expected
    assert rates(0, tolerance=0) == (Q // 10, 0)
    example = worked_example()
    scoring_figure(example, args.preview_dir)
    emissions_figure(example, args.preview_dir)
    rate_figure(args.preview_dir)
    removals = history_figure(args.preview_dir)
    payload = {"description": "Documentation examples, not benchmark measurements", "q18": Q,
               "parameters": {"reputation_tolerance": TOLERANCE, "max_reputation_increase": MAX_GAIN,
                              "max_reputation_decrease": MAX_LOSS, "minimum_reputation": MIN_REPUTATION},
               "worked_example": example, "illustrative_removal_rounds": removals}
    (ROOT / "examples.json").write_text(json.dumps(payload, indent=2) + "\n")
    for node in example["stake"]:
        print(node, {key: round(example[key][node] * 100 / Q, 6) for key in ["performance", "accuracy", "accuracy_share", "performance_share", "weight"]})
    print("Reputation example checks passed; removal rounds:", removals)


if __name__ == "__main__":
    main()
