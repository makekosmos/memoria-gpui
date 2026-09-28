# /// script
# requires-python = ">=3.11"
# dependencies = ["pillow>=10"]
# ///
"""Regenerate `windows/app.ico` from the committed `windows/app-icon.png`
(1000x1000 RGBA). Usage: `python scripts/make-icon.py` (or `uv run
scripts/make-icon.py`). Output is deterministic for a fixed Pillow version.
"""
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SIZES = [16, 24, 32, 48, 64, 128, 256]

source = Image.open(ROOT / "windows" / "app-icon.png")
assert source.mode == "RGBA" and source.width == source.height, (
    f"app-icon.png must be square RGBA, got {source.size} {source.mode}"
)
# Pillow LANCZOS-downscales to each listed size; 256px is PNG-compressed.
source.save(
    ROOT / "windows" / "app.ico",
    format="ICO",
    sizes=[(s, s) for s in SIZES],
)
print(f"wrote windows/app.ico ({', '.join(map(str, SIZES))} px)")
