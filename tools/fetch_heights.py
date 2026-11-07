#!/usr/bin/env python3
"""Fetch a level's ground heights from Lantmäteriet as an ESRI ASCII grid (`.asc`).

Markhöjdmodell Nedladdning, a 1 m grid in SWEREF 99 TM / RH 2000, averaged to 2 m. The files
are 10 x 10 km cloud-optimised GeoTIFFs: only the blocks a level touches are downloaded. The
login is read from credentials.txt (two lines, `key: value`, user then password), which is
gitignored. The import only reads the `.asc`, so this needs running once per level.

    python3 tools/fetch_heights.py 59.199295 17.753444 480 assets/osm/map-01.heights.asc

The third number is half the width of the square to fetch, in metres: leave room for the
level's rotation. Needs numpy.
"""

import base64
import json
import math
import struct
import sys
import urllib.request
import zlib
from pathlib import Path

import numpy as np

SEARCH = "https://api.lantmateriet.se/stac-hojd/v1/search"
HOSTS = ("https://api.lantmateriet.se/", "https://dl1.lantmateriet.se/")
BLOCK = 2  # metres averaged into a cell
NODATA = -9999


def login():
    lines = Path("credentials.txt").read_text().splitlines()
    user, password = (line.split(":", 1)[1].strip() for line in lines[:2])
    token = base64.b64encode(f"{user}:{password}".encode()).decode()
    return {"Authorization": f"Basic {token}", "User-Agent": "aliens-vs-suburbia-gis"}


def fetch(url, headers, data=None):
    if not url.startswith(HOSTS):
        raise SystemExit(f"refusing to send the login to {url}")
    with urllib.request.urlopen(
        urllib.request.Request(url, data=data, headers=headers), timeout=60
    ) as response:
        return response.read()


def sweref(lat, lon):
    """Lat/lon to SWEREF 99 TM, same as `elevation.rs`."""
    flattening = 1 / 298.257222101
    n = flattening / (2 - flattening)
    radius = 6378137.0 / (1 + n) * (1 + n * n / 4 + n**4 / 64)
    ecc = 2 * math.sqrt(n) / (1 + n)
    phi, lam = math.radians(lat), math.radians(lon - 15)
    t = math.sinh(math.atanh(math.sin(phi)) - ecc * math.atanh(ecc * math.sin(phi)))
    xi = math.atan2(t, math.cos(lam))
    eta = math.asinh(math.sin(lam) / math.hypot(t, math.cos(lam)))
    alpha = [n / 2 - 2 * n * n / 3 + 5 * n**3 / 16, 13 * n * n / 48 - 3 * n**3 / 5, 61 * n**3 / 240]
    north = xi + sum(a * math.sin(2 * (j + 1) * xi) * math.cosh(2 * (j + 1) * eta) for j, a in enumerate(alpha))
    east = eta + sum(a * math.cos(2 * (j + 1) * xi) * math.sinh(2 * (j + 1) * eta) for j, a in enumerate(alpha))
    return 500000 + 0.9996 * radius * east, 0.9996 * radius * north


class Tiff:
    """The blocks of a classic, little-endian, deflated float32 GeoTIFF."""

    SIZES = {3: ("H", 2), 4: ("I", 4)}

    def __init__(self, url, headers):
        self.url, self.headers, self.bytes = url, headers, 0
        head = self.read(0, 1 << 16)
        if head[:4] != b"II*\0":
            raise SystemExit(f"not a classic little-endian TIFF: {head[:4]!r}")
        ifd = struct.unpack_from("<I", head, 4)[0]
        if ifd + 4000 > len(head):
            head = self.read(0, ifd + (1 << 16))
        count = struct.unpack_from("<H", head, ifd)[0]
        tags = {}
        for i in range(count):
            tag, kind, n = struct.unpack_from("<HHI", head, ifd + 2 + i * 12)
            tags[tag] = (kind, n, ifd + 2 + i * 12 + 8)

        def values(tag):
            kind, n, at = tags[tag]
            code, size = self.SIZES[kind]
            if n * size > 4:
                at = struct.unpack_from("<I", head, at)[0]
                if at + n * size > len(head):
                    return struct.unpack(f"<{n}{code}", self.read(at, n * size))
            return struct.unpack_from(f"<{n}{code}", head, at)

        self.width, self.height = values(256)[0], values(257)[0]
        self.block = values(322)[0]
        if values(323)[0] != self.block or values(259)[0] not in (8, 32946):
            raise SystemExit("unexpected TIFF layout")
        if values(258)[0] != 32 or values(339)[0] != 3 or values(317)[0] != 3:
            raise SystemExit("expected float32 with the floating point predictor")
        self.offsets, self.counts = values(324), values(325)

    def read(self, start, length):
        headers = {**self.headers, "Range": f"bytes={start}-{start + length - 1}"}
        body = fetch(self.url, headers)
        self.bytes += len(body)
        return body

    def tile(self, col, row):
        across = math.ceil(self.width / self.block)
        index = row * across + col
        raw = zlib.decompress(self.read(self.offsets[index], self.counts[index]))
        size = self.block
        # Predictor 3: bytes split into planes, big end first, each row differenced.
        planes = np.cumsum(np.frombuffer(raw, np.uint8).reshape(size, size * 4), axis=1, dtype=np.uint8)
        stacked = np.stack([planes[:, i * size : (i + 1) * size] for i in range(4)], axis=2)
        return np.frombuffer(stacked.tobytes(), ">f4").reshape(size, size).astype("f4")


def main():
    lat, lon, half = map(float, sys.argv[1:4])
    output = Path(sys.argv[4])
    east, north = sweref(lat, lon)
    west_edge = math.floor((east - half) / BLOCK) * BLOCK
    north_edge = math.ceil((north + half) / BLOCK) * BLOCK
    size = math.ceil(2 * half / BLOCK) * BLOCK

    headers = login()
    margin_lat, margin_lon = half / 111000, half / (111000 * math.cos(math.radians(lat)))
    query = {
        "bbox": [lon - margin_lon, lat - margin_lat, lon + margin_lon, lat + margin_lat],
        "collections": ["dtm-cog"],
        "limit": 10,
    }
    found = json.loads(fetch(SEARCH, {**headers, "Content-Type": "application/json"}, json.dumps(query).encode()))
    items = found["features"]
    if len(items) != 1:
        raise SystemExit(f"the area touches {len(items)} height files; this tool reads one")
    url = items[0]["assets"]["data"]["href"]
    name = Path(url).stem  # m656_65: 6560 km north, 650 km east
    file_north = (int(name[1:4]) + 1) * 10000
    file_east = int(name[5:7]) * 10000
    tiff = Tiff(url, headers)

    left, top = int(west_edge - file_east), int(file_north - north_edge)
    if left < 0 or top < 0 or left + size > tiff.width or top + size > tiff.height:
        raise SystemExit("the area runs off the height file; this tool reads one")
    grid = np.empty((size, size), "f4")
    block = tiff.block
    for row in range(top // block, (top + size - 1) // block + 1):
        for col in range(left // block, (left + size - 1) // block + 1):
            tile = tiff.tile(col, row)
            x0, y0 = max(left, col * block), max(top, row * block)
            x1, y1 = min(left + size, (col + 1) * block), min(top + size, (row + 1) * block)
            grid[y0 - top : y1 - top, x0 - left : x1 - left] = tile[
                y0 - row * block : y1 - row * block, x0 - col * block : x1 - col * block
            ]
    grid[grid < -9000] = np.nan
    cells = size // BLOCK
    averaged = grid.reshape(cells, BLOCK, cells, BLOCK).mean(axis=(1, 3))

    lines = [
        f"ncols {cells}",
        f"nrows {cells}",
        f"xllcorner {west_edge}",
        f"yllcorner {north_edge - size}",
        f"cellsize {BLOCK}",
        f"NODATA_value {NODATA}",
    ]
    for row in averaged:
        lines.append(" ".join(str(NODATA) if np.isnan(v) else f"{v:.2f}" for v in row))
    output.write_text("\n".join(lines) + "\n")
    print(f"{output}: {cells} x {cells} cells, {np.nanmin(averaged):.1f} to {np.nanmax(averaged):.1f} m, "
          f"{tiff.bytes / 1e6:.1f} MB fetched")


if __name__ == "__main__":
    main()
