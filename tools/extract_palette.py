#!/usr/bin/env python3
"""Pull a palette out of a reference picture (docs/retro-look.md).

k-means in OKLab over the picture's pixels, so the clusters follow how different colours
look rather than how far apart their RGB numbers are. Rare but striking colours (a glowing
sign, an orange suit) would lose to the big areas, so a pixel's weight grows with its
chroma. The result is sorted by hue family, dark to light, and written as a Lospec .hex.

    python3 tools/extract_palette.py docs/gfx-reference/giraud.png giraud 128
"""

import sys

import numpy as np
from PIL import Image

M1 = np.array([
    [0.4122214708, 0.5363325363, 0.0514459929],
    [0.2119034982, 0.6806995451, 0.1073969566],
    [0.0883024619, 0.2817188376, 0.6299787005],
])
M2 = np.array([
    [0.2104542553, 0.7936177850, -0.0040720468],
    [1.9779984951, -2.4285922050, 0.4505937099],
    [0.0259040371, 0.7827717662, -0.8086757660],
])


def srgb_to_linear(c):
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def linear_to_srgb(c):
    c = np.clip(c, 0.0, 1.0)
    return np.where(c <= 0.0031308, c * 12.92, 1.055 * c ** (1 / 2.4) - 0.055)


def to_lab(rgb):
    return np.cbrt(srgb_to_linear(rgb) @ M1.T) @ M2.T


def from_lab(lab):
    lms = (lab @ np.linalg.inv(M2).T) ** 3
    return linear_to_srgb(lms @ np.linalg.inv(M1).T)


def kmeans(points, weights, k, rounds=40, seed=1):
    rng = np.random.default_rng(seed)
    # k-means++ start: spread the first centres out.
    centres = [points[rng.choice(len(points), p=weights / weights.sum())]]
    d2 = np.full(len(points), np.inf)
    for _ in range(1, k):
        d2 = np.minimum(d2, ((points - centres[-1]) ** 2).sum(1))
        p = d2 * weights
        centres.append(points[rng.choice(len(points), p=p / p.sum())])
    centres = np.array(centres)
    for _ in range(rounds):
        nearest = np.argmin(((points[:, None, :] - centres[None]) ** 2).sum(2), axis=1)
        for i in range(k):
            mask = nearest == i
            if mask.any():
                w = weights[mask]
                centres[i] = (points[mask] * w[:, None]).sum(0) / w.sum()
    return centres


def main():
    path, name, k = sys.argv[1], sys.argv[2], int(sys.argv[3])
    img = Image.open(path).convert("RGB")
    img.thumbnail((320, 320))
    rgb = np.asarray(img, dtype=np.float64).reshape(-1, 3) / 255.0
    lab = to_lab(rgb)
    chroma = np.hypot(lab[:, 1], lab[:, 2])
    weights = 1.0 + 12.0 * chroma
    centres = kmeans(lab, weights, k)

    # Group: greys first, then by hue; dark to light inside a group.
    c = np.hypot(centres[:, 1], centres[:, 2])
    hue = (np.degrees(np.arctan2(centres[:, 2], centres[:, 1])) + 360.0) % 360.0
    family = np.where(c < 0.03, -1, (hue // 30).astype(int))
    order = np.lexsort((centres[:, 0], family))
    out = []
    for colour in from_lab(centres[order]):
        h = "".join(f"{round(v * 255):02x}" for v in colour)
        if h not in out:
            out.append(h)
    dest = f"assets/palettes/{name}.hex"
    with open(dest, "w") as f:
        f.write(f"; {len(out)} colours from {path} - tools/extract_palette.py\n")
        f.write("\n".join(out) + "\n")
    print(f"{dest}: {len(out)} colours")


if __name__ == "__main__":
    main()
