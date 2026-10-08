# Synthetic AVIF fixtures

Original solid colours, generated for PhotoCraft; no artwork or personal files.
MIT OR Apache-2.0 (see LICENSE.txt).

- `rgb.avif`: 32x16 red, 8-bit YUV444, FFmpeg git `cbbc927a67` (2025-03-24)/libaom, lossless (`-crf 0`).
- `ten-bit.avif`: 32x16 grey, 10-bit YUV444, FFmpeg/libaom, lossless.
- `twelve-bit.avif`: the same grey image at 12 bits (`yuv444p12le`).
- `rgba.avif`: 32x16, opaque red left half, green with alpha 128 right half;
  ImageMagick 7.1.2-21/libheif, `-define heic:chroma=444 -quality 100`.

FFmpeg input: `-f lavfi -i color=red:s=32x16` (or `gray`), `-frames:v 1`,
`-pix_fmt yuv444p` (or `yuv444p10le`), `-c:v libaom-av1 -cpu-used 8 -crf 0`,
`-color_primaries bt709 -color_trc iec61966-2-1 -colorspace bt470bg` (or `bt709`).
Tests compare the decoded pixels with these independently specified colours and alpha.
