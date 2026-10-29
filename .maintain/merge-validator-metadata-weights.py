#!/usr/bin/env python3
"""Merge the reviewed metadata benchmark methods, retaining existing safety additions.

Usage: python3 .maintain/merge-validator-metadata-weights.py generated-weights.rs
Only accepts the explicit benchmark set from benchmark-validator-metadata.sh.
"""
import re
import sys
from pathlib import Path

METHODS = {
    "register_validator", "update_validator_hotkey", "update_validator_delegate_account",
    "update_validator_identity", "update_validator_identity_create", "update_validator_identity_clear",
    "update_validator_delegate_reward_rate", "add_validator_delegate_stake",
    "swap_from_subnet_to_validator", "set_overwatch_validator_whitelist",
    "execute_ready_swap_calls", "execute_ready_swap_mixed_validator",
    "execute_ready_swap_mixed_subnet", "execute_ready_swap_mixed_refund", "emission_step",
}
METHOD = re.compile(r"(?:\t///[^\n]*\n)*\tfn (\w+)\([^\n]*\) -> Weight \{.*?\n\t\}", re.S)
SAFETY = re.compile(
    r"^\t\t\t\.saturating_add\((?:pending_liability_key_proof|settlement_pending_marker_envelope|consensus_round_liability_proof)\(\).*?$",
    re.M,
)


def merge(generated_path):
    generated = Path(generated_path).read_text()
    methods = {}
    for match in METHOD.finditer(generated):
        methods.setdefault(match[1], []).append(match[0])
    if set(methods) != METHODS or any(len(v) != 2 for v in methods.values()):
        raise ValueError("Expected exactly the 15 metadata methods in both implementations")
    destination = Path(__file__).resolve().parents[1] / "pallets/network/src/weights.rs"
    source = destination.read_text()
    counts = dict.fromkeys(METHODS, 0)

    def replace(match):
        name = match[1]
        if name not in METHODS:
            return match[0]
        replacement = methods[name][counts[name]]
        counts[name] += 1
        additions = SAFETY.findall(match[0])
        if additions:
            replacement = replacement[:-3] + "\n" + "\n".join(additions) + "\n\t}"
        return replacement

    merged = METHOD.sub(replace, source)
    if any(n != 2 for n in counts.values()):
        raise ValueError(f"Expected two existing implementations: {counts}")
    if SAFETY.findall(merged) != SAFETY.findall(source):
        raise ValueError("Manual safety additions changed")
    destination.write_text(merged)
    print("Updated 15 methods in both implementations; retained manual safety additions")


if __name__ == "__main__":
    merge(sys.argv[1])
