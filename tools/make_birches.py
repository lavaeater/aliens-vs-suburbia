#!/usr/bin/env python3
"""Make the birches in assets/packs/nature from the pack's leafy trees.

Same mesh, white bark with black dashes, paler leaves. Bark UVs repeat, so the marks wrap
around the texture's edges. Needs numpy and Pillow; always writes the same files.

    python3 tools/make_birches.py
"""

import io
import json
import struct
from pathlib import Path

import numpy as np
from PIL import Image

NATURE = Path("assets/packs/nature")
BIRCHES = {"Tree.glb": "Birch.glb", "Tree-aVOxaHRPWe.glb": "Birch-2.glb"}
SIZE = 512
BARK_WHITE = np.array([232.0, 228.0, 216.0])
BARK_BLACK = np.array([38.0, 34.0, 32.0])
LEAF_GAIN = np.array([1.75, 1.6, 1.0])
LEAF_LIFT = np.array([0.0, 0.0, 60.0])


def read_glb(path):
    data = path.read_bytes()
    json_len = struct.unpack_from("<I", data, 12)[0]
    gltf = json.loads(data[20 : 20 + json_len])
    return gltf, data[20 + json_len + 8 :]


def write_glb(path, gltf, views):
    """Lay the buffer views out again, as images change size."""
    binary = bytearray()
    for view, content in zip(gltf["bufferViews"], views):
        binary.extend(b"\0" * (-len(binary) % 4))
        view["byteOffset"] = len(binary)
        view["byteLength"] = len(content)
        binary.extend(content)
    binary.extend(b"\0" * (-len(binary) % 4))
    gltf["buffers"] = [{"byteLength": len(binary)}]
    text = json.dumps(gltf, separators=(",", ":")).encode()
    text += b" " * (-len(text) % 4)
    total = 12 + 8 + len(text) + 8 + len(binary)
    path.write_bytes(
        struct.pack("<III", 0x46546C67, 2, total)
        + struct.pack("<II", len(text), 0x4E4F534A)
        + text
        + struct.pack("<II", len(binary), 0x004E4942)
        + binary
    )


def png(rgba):
    out = io.BytesIO()
    Image.fromarray(rgba.astype(np.uint8), "RGBA").save(out, "PNG", optimize=True)
    return out.getvalue()


def bark(original):
    """The original's relief, in white, with black dashes that run around the trunk."""
    image = np.asarray(original.convert("RGBA").resize((SIZE, SIZE)), dtype=float)
    light = image[..., :3].mean(axis=2)
    relief = np.clip(0.8 + 0.2 * light / light.mean(), 0.0, 1.05)
    colour = BARK_WHITE * relief[..., None]

    rng = np.random.default_rng(1)
    ink = np.zeros((SIZE, SIZE))
    for _ in range(90):
        x, y = rng.integers(SIZE, size=2)
        half_width = rng.integers(10, 60)
        half_height = rng.integers(1, 5)
        for row in range(-half_height, half_height + 1):
            reach = half_width * (1 - abs(row) / (half_height + 1))
            for col in range(-int(reach), int(reach) + 1):
                ink[(y + row) % SIZE, (x + col) % SIZE] = 1.0
    colour = colour * (1 - ink[..., None]) + BARK_BLACK * ink[..., None]
    return np.dstack([colour, np.full((SIZE, SIZE), 255.0)])


def leaves(original):
    image = np.asarray(original.convert("RGBA").resize((SIZE, SIZE)), dtype=float)
    colour = np.clip(image[..., :3] * LEAF_GAIN + LEAF_LIFT, 0, 255)
    return np.dstack([colour, image[..., 3]])


def main():
    for source, target in BIRCHES.items():
        gltf, binary = read_glb(NATURE / source)
        views = []
        for view in gltf["bufferViews"]:
            start = view.get("byteOffset", 0)
            views.append(binary[start : start + view["byteLength"]])
        for image in gltf["images"]:
            view = image["bufferView"]
            original = Image.open(io.BytesIO(views[view]))
            if image["name"] == "Bark_NormalTree.png":
                views[view] = png(bark(original))
            elif image["name"] == "Leaves_NormalTree_C.png":
                views[view] = png(leaves(original))
        write_glb(NATURE / target, gltf, views)
        print(f"{target}: {(NATURE / target).stat().st_size} bytes")


if __name__ == "__main__":
    main()
