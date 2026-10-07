#!/usr/bin/env python3
"""Generate synthetic test data for Home Hub pilot/perf runs (TEST_PLAN §5).

Creates a realistic mixed library: JPEG photos with EXIF dates, videos
(sparse files), documents, and duplicates, laid out as
  <out>/Photos/YYYY/MM/..., <out>/Documents/..., <out>/Duplicates/...

Usage: tools/gen_test_data.py --out /tmp/hhdata --photos 500 --dupes 20
"""
import argparse
import datetime
import random
import struct
from pathlib import Path


def minimal_jpeg(width: int, height: int, seed: int, taken: datetime.datetime) -> bytes:
    """A tiny valid-enough JPEG-like blob with an EXIF DateTimeOriginal tag.

    Not a real encoder — the point is a deterministic file whose EXIF date
    kamadak-exif can read, sized ~50–200 KB like a compressed phone photo.
    """
    rnd = random.Random(seed)
    body = bytes(rnd.getrandbits(8) for _ in range(rnd.randint(50_000, 200_000)))
    dt = taken.strftime("%Y:%m:%d %H:%M:%S").encode()
    # APP1 EXIF segment with a single DateTimeOriginal tag (little-endian TIFF).
    # Value offset = TIFF header (8) + count (2) + entry (12) + next-IFD (4) = 26.
    tiff = b"II*\x00" + struct.pack("<I", 8)
    ifd = struct.pack("<H", 1) + struct.pack("<HHII", 0x9003, 2, 20, 26) + struct.pack("<I", 0)
    exif = b"Exif\x00\x00" + tiff + ifd + dt + b"\x00"
    app1 = b"\xff\xe1" + struct.pack(">H", len(exif) + 2) + exif
    return b"\xff\xd8" + app1 + b"\xff\xdb" + struct.pack(">H", 69) + bytes(67) + body + b"\xff\xd9"


def sparse_video(path: Path, mib: int, seed: int) -> None:
    rnd = random.Random(seed)
    with open(path, "wb") as f:
        f.truncate(mib * 1024 * 1024)
        for _ in range(16):  # sprinkle real data so hashes differ
            f.seek(rnd.randrange(0, mib * 1024 * 1024 - 4096))
            f.write(bytes(rnd.getrandbits(8) for _ in range(4096)))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--photos", type=int, default=500)
    ap.add_argument("--videos", type=int, default=20)
    ap.add_argument("--docs", type=int, default=50)
    ap.add_argument("--dupes", type=int, default=20, help="exact duplicate photos")
    args = ap.parse_args()

    rnd = random.Random(42)
    start = datetime.datetime(2019, 1, 1)
    span_days = (datetime.datetime(2026, 1, 1) - start).days
    made: list[Path] = []

    for i in range(args.photos):
        taken = start + datetime.timedelta(days=rnd.randrange(span_days),
                                           seconds=rnd.randrange(86400))
        d = args.out / "Photos" / f"{taken.year}" / f"{taken.month:02d}"
        d.mkdir(parents=True, exist_ok=True)
        p = d / f"IMG_{i:05d}.jpg"
        p.write_bytes(minimal_jpeg(4032, 3024, i, taken))
        made.append(p)

    vdir = args.out / "Videos"; vdir.mkdir(parents=True, exist_ok=True)
    for i in range(args.videos):
        sparse_video(vdir / f"VID_{i:03d}.mp4", rnd.randint(20, 400), 10_000 + i)

    ddir = args.out / "Documents"; ddir.mkdir(parents=True, exist_ok=True)
    for i in range(args.docs):
        (ddir / f"doc_{i:03d}.txt").write_text(f"synthetic document {i}\n" * rnd.randint(10, 500))

    # Duplicates: byte-identical copies under a different name/folder.
    dup = args.out / "Duplicates"; dup.mkdir(parents=True, exist_ok=True)
    for i in range(min(args.dupes, len(made))):
        src = made[rnd.randrange(len(made))]
        (dup / f"copy_of_{src.stem}_{i}.jpg").write_bytes(src.read_bytes())

    print(f"wrote {args.photos} photos, {args.videos} videos, {args.docs} docs, "
          f"{min(args.dupes, len(made))} duplicates under {args.out}")


if __name__ == "__main__":
    main()
