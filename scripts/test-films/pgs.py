#!/usr/bin/env python3
"""Writes a subtitle made of pictures, the kind a Blu-ray carries (PGS).

    pgs.py FFMPEG WIDTH HEIGHT OUT.sup < LINES

Each line of LINES is "start|end|place|text": seconds where the words appear
and leave, "top" or "bottom", and the words, "\\n" breaking them. FFmpeg can
read this format but not write it, and no free film carries one, hence this.

Each picture is drawn by FFmpeg (drawtext, white with a black border), cut to
the words, brought to a palette of a few greys and levels of transparency, and
written as the format lays it out: a set of segments to show it, another to
take it away.
"""

import struct
import subprocess
import sys
import tempfile

# The clock the format counts in.
TICKS_PER_SECOND = 90_000

# Segment types.
PALETTE = 0x14
OBJECT = 0x15
COMPOSITION = 0x16
WINDOW = 0x17
END = 0x80

# Grey levels and opacities a pixel is brought to: enough for smooth edges.
LEVELS = 4

# Room left between the words and the edge of the picture, as a share of its
# height.
MARGIN = 0.06

# The most picture data one segment can carry once its header is counted.
LARGEST_SEGMENT = 0xFFFF


def drawn(ffmpeg, width, height, text):
    """The words drawn on a transparent strip as wide as the picture, as RGBA."""
    size = max(height // 18, 12)
    strip = size * 4
    with tempfile.NamedTemporaryFile("w", suffix=".txt", encoding="utf-8") as words:
        words.write(text.replace("\\n", "\n"))
        words.flush()
        graph = (
            f"drawtext=textfile={words.name}:font=DejaVu Sans:fontsize={size}"
            f":fontcolor=white:borderw={max(size // 12, 2)}:bordercolor=black"
            f":line_spacing={size // 4}:x=(w-text_w)/2:y=(h-text_h)/2"
        )
        out = subprocess.run(
            [ffmpeg, "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i",
             f"color=c=black@0.0:s={width}x{strip},format=rgba", "-vf", graph,
             "-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgba", "-"],
            check=True, capture_output=True,
        ).stdout
    return out, width, strip


def cut_to_the_words(rgba, width, height):
    """The smallest box holding every pixel that shows, as (x, y, w, h)."""
    rows = [y for y in range(height)
            if any(rgba[(y * width + x) * 4 + 3] for x in range(width))]
    columns = [x for x in range(width)
               if any(rgba[(y * width + x) * 4 + 3] for y in rows)]
    left, right, top, bottom = columns[0], columns[-1], rows[0], rows[-1]
    # Even sizes, which every reader takes.
    w = (right - left + 2) // 2 * 2
    h = (bottom - top + 2) // 2 * 2
    return left, top, min(w, width - left), min(h, height - top)


def palette_of():
    """Entry 0 transparent, then every grey level at every opacity."""
    entries = [(0, 16, 128, 128, 0)]
    for alpha in range(1, LEVELS):
        for grey in range(LEVELS):
            luma = 16 + round(grey * 219 / (LEVELS - 1))
            opacity = round(alpha * 255 / (LEVELS - 1))
            entries.append((len(entries), luma, 128, 128, opacity))
    return entries


def index_of(red, green, blue, alpha):
    """The palette entry closest to one pixel."""
    opacity = round(alpha * (LEVELS - 1) / 255)
    if opacity == 0:
        return 0
    grey = round((red + green + blue) / 3 * (LEVELS - 1) / 255)
    return 1 + (opacity - 1) * LEVELS + grey


def run_length(indices, width, height):
    """The picture as the format compresses it, line by line."""
    out = bytearray()
    for y in range(height):
        line = indices[y * width:(y + 1) * width]
        x = 0
        while x < width:
            colour = line[x]
            run = 1
            while x + run < width and line[x + run] == colour and run < 0x3FFF:
                run += 1
            if colour != 0 and run == 1:
                out.append(colour)
            elif colour == 0:
                out += bytes([0, run]) if run < 64 else bytes([0, 0x40 | run >> 8, run & 0xFF])
            elif run < 64:
                out += bytes([0, 0x80 | run, colour])
            else:
                out += bytes([0, 0xC0 | run >> 8, run & 0xFF, colour])
            x += run
        out += b"\x00\x00"
    return bytes(out)


def segment(kind, at, payload):
    ticks = round(at * TICKS_PER_SECOND)
    return b"PG" + struct.pack(">IIBH", ticks, 0, kind, len(payload)) + payload


def composition(width, height, number, shown, x=0, y=0):
    """What is on screen from this moment: one picture at (x, y), or nothing."""
    head = struct.pack(">HHBHBBBB", width, height, 0x10, number,
                       0x80 if shown else 0x00, 0, 0, 1 if shown else 0)
    if shown:
        head += struct.pack(">HBBHH", 0, 0, 0, x, y)
    return head


def window(x, y, w, h):
    return struct.pack(">BBHHHH", 1, 0, x, y, w, h)


def objects(at, data, w, h):
    """The picture's segments, split where one segment cannot hold it all."""
    full = struct.pack(">I", len(data) + 4)[1:] + struct.pack(">HH", w, h) + data
    first_room = LARGEST_SEGMENT - 4
    pieces = [full[:first_room]]
    rest = full[first_room:]
    while rest:
        pieces.append(rest[:LARGEST_SEGMENT - 4])
        rest = rest[LARGEST_SEGMENT - 4:]
    out = b""
    for number, piece in enumerate(pieces):
        flags = (0x80 if number == 0 else 0) | (0x40 if number == len(pieces) - 1 else 0)
        out += segment(OBJECT, at, struct.pack(">HBB", 0, 0, flags) + piece)
    return out


def main():
    ffmpeg, width, height, written = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
    palette = b"".join(struct.pack(">BBBBB", *entry) for entry in palette_of())
    number = 0
    out = bytearray()
    for line in sys.stdin.read().splitlines():
        if not line.strip():
            continue
        start, end, place, text = line.split("|", 3)
        start, end = float(start), float(end)
        rgba, strip_width, strip_height = drawn(ffmpeg, width, height, text)
        left, top, w, h = cut_to_the_words(rgba, strip_width, strip_height)
        indices = bytes(
            index_of(*rgba[((top + y) * strip_width + left + x) * 4:
                           ((top + y) * strip_width + left + x) * 4 + 4])
            for y in range(h) for x in range(w)
        )
        margin = round(height * MARGIN)
        x = left
        y = margin if place == "top" else height - margin - h

        out += segment(COMPOSITION, start, composition(width, height, number, True, x, y))
        out += segment(WINDOW, start, window(x, y, w, h))
        out += segment(PALETTE, start, struct.pack(">BB", 0, 0) + palette)
        out += objects(start, run_length(indices, w, h), w, h)
        out += segment(END, start, b"")
        number += 1

        out += segment(COMPOSITION, end, composition(width, height, number, False))
        out += segment(WINDOW, end, window(x, y, w, h))
        out += segment(END, end, b"")
        number += 1

    with open(written, "wb") as sup:
        sup.write(out)


if __name__ == "__main__":
    main()
