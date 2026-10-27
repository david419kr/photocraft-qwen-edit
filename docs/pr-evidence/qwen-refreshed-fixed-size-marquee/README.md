# Qwen Fixed Size marquee: refreshed-main verification

PR: https://github.com/david419kr/photocraft-qwen-edit/pull/15

Source: `4e1a3638e9bb1a98856979d16e36bc8dd7799fc6`, including fork main `ef86857658a638de21792092a7fa79d705a2c0b2` and its explicit/reversible Fill color controls and FXC GPU fix.

The source was rebuilt using the native offscreen snapshot example on Linux (CPU canvas/software Vulkan, 1440 x 900, scale 1). Both captures use one binary and a blank 100 x 100 document at 400% zoom. Scripts are included.

A 100 x 50 rectangle is placed at (50, 25), then its outline moves 50 pixels left:

- `before-clipped.png`: `clipToCanvas: true`, leaving 50 x 50.
- `after-preserved.png`: `clipToCanvas: false`, retaining 100 x 50.

The engine commands compare mask geometry and leave the options bar at Normal; actual Fixed Size clicks/drags are covered by the separate passing marquee UI regression. These captures were visually inspected after rebuilding against the new main. They do not validate live Qwen inference, actual CUDA/VRAM, Windows graphics, or the later release build.

The related Fill integration regression verifies that the input consumes only in-canvas coverage without clipping the document's movable selection. Fully outside selections return an error before any model request.

This evidence branch is separate from the application's feature diff. Public release/tag creation remains subject to the user's later approval.
