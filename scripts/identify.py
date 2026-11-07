#!/usr/bin/env python3
"""Rung 5, the reference's half: the scheme each image is identified as.

    identify.py FILE [...]

The same lines as ``target/release/identify``; run it with the reference's
interpreter.
"""

from __future__ import annotations

import sys
from pathlib import Path

from a2600_python.cartridge import UnsupportedCartridge, identify, read_image


def main() -> int:
    for arg in sys.argv[1:]:
        path = Path(arg)
        try:
            image = read_image(path)[0]
        except Exception:  # noqa: BLE001 - any unreadable archive
            print(f"{path.name}\tunreadable")
            continue
        try:
            family, sc, source, _ = identify(image)
            print(f"{path.name}\t{family}\t{sc}\t{source}")
        except UnsupportedCartridge:
            print(f"{path.name}\tunsupported")
    return 0


if __name__ == "__main__":
    sys.exit(main())
