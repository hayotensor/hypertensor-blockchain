#!/usr/bin/env python3
"""Install measured batch appends; retain the independent single-enqueue safety floor."""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
METHOD = re.compile(r"(?:\t///[^\n]*\n)*\tfn (\w+)\([^\n]*\) -> Weight \{.*?\n\t\}", re.S)
SINGLE = "pending_consensus_operation_enqueue"
BATCH = "pending_consensus_operations_enqueue"
EMPTY = "pending_consensus_operations_enqueue_empty_queue"


def body(method):
    return method.split("{", 1)[1].rsplit("\n\t}", 1)[0].strip("\n")


def merge(source, generated):
    methods = {}
    for match in METHOD.finditer(generated):
        methods.setdefault(match[1], []).append(match[0])
    if set(methods) != {SINGLE, BATCH, EMPTY} or any(len(x) != 2 for x in methods.values()):
        raise ValueError("Expected both implementations of all three enqueue measurements")
    counts = {SINGLE: 0, BATCH: 0}

    def replace(match):
        name = match[1]
        if name not in counts:
            return match[0]
        index = counts[name]
        counts[name] += 1
        measured = methods[name][index]
        alternate = methods[EMPTY][index] if name == BATCH else match[0]
        result = measured[:measured.index("{") + 1]
        result += "\n\t\tlet measured = {\n" + body(measured) + "\n\t\t};\n"
        result += "\t\tlet alternate = {\n" + body(alternate) + "\n\t\t};\n"
        result += "\t\tmeasured.max(alternate)\n\t}"
        return result

    merged = METHOD.sub(replace, source)
    if counts != {SINGLE: 2, BATCH: 2}:
        raise ValueError(f"Unexpected weight implementation counts: {counts}")
    return merged.replace("\t// Temporary conservative bound; replaced by the measured batch model.\n", "")


if __name__ == "__main__":
    destination = ROOT / "pallets/network/src/weights.rs"
    destination.write_text(merge(destination.read_text(), Path(sys.argv[1]).read_text()))
    print("Installed both batch models and retained the single-enqueue floor")
