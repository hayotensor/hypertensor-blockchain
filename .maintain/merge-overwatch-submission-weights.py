#!/usr/bin/env python3
"""Retain prior bounds and measured rejection envelopes for complete Overwatch submissions."""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ALTERNATIVES = {
    "commit_overwatch_subnet_weights": (
        "commit_overwatch_subnet_weights_invalid",
        "commit_overwatch_subnet_weights_repeated",
    ),
    "reveal_overwatch_subnet_weights": (
        "reveal_overwatch_subnet_weights_incomplete",
        "reveal_overwatch_subnet_weights_invalid",
        "reveal_overwatch_subnet_weights_repeated",
    ),
}
METHOD = re.compile(r"(?:\t///[^\n]*\n)*\tfn (\w+)\([^\n]*\) -> Weight \{.*?\n\t\}", re.S)


def body(method):
    return method.split("{", 1)[1].rsplit("\n\t}", 1)[0].strip("\n")


def merge(source, generated):
    expected = set(ALTERNATIVES) | {name for names in ALTERNATIVES.values() for name in names}
    methods = {}
    for match in METHOD.finditer(generated):
        methods.setdefault(match[1], []).append(match[0])
    if set(methods) != expected or any(len(values) != 2 for values in methods.values()):
        raise ValueError("Expected both implementations of all seven submission benchmarks")
    counts = dict.fromkeys(ALTERNATIVES, 0)

    def replace(match):
        name = match[1]
        if name not in ALTERNATIVES:
            return match[0]
        index = counts[name]
        counts[name] += 1
        measured = methods[name][index]
        result = measured[:measured.index("{") + 1]
        result += "\n\t\t// Complete batch, worst-case rejections, and prior bounds, componentwise.\n"
        result += "\t\tlet successful = {\n" + body(measured) + "\n\t\t};\n"
        for i, alternate in enumerate(ALTERNATIVES[name]):
            result += f"\t\tlet rejected_{i} = {{\n" + body(methods[alternate][index]) + "\n\t\t};\n"
        result += "\t\tlet prior = {\n" + body(match[0]) + "\n\t\t};\n"
        result += "\t\tsuccessful.max(prior)" + "".join(f".max(rejected_{i})" for i in range(len(ALTERNATIVES[name]))) + "\n\t}"
        return result

    merged = METHOD.sub(replace, source)
    if any(count != 2 for count in counts.values()):
        raise ValueError(f"Expected both existing implementations: {counts}")
    return merged


if __name__ == "__main__":
    destination = ROOT / "pallets/network/src/weights.rs"
    destination.write_text(merge(destination.read_text(), Path(sys.argv[1]).read_text()))
    print("Updated commit and reveal weights in both implementations, retaining prior bounds")
