# Fixed Size marquee geometry comparison

Captured from PhotoCraft commit `0b24c0c0c` using the offscreen `snapshot` example on Linux.
Both captures use the same binary and a blank 100 x 100 pixel document at 400% zoom.
The JSON scripts record the exact control-protocol calls.

- `before-clipped.png`: the existing `select.rect` canvas-clipping path (the default), followed by an outline move of -50 pixels in X, leaves a 50 x 50 selection.
- `after-preserved.png`: `select.rect` with `clipToCanvas: false`, followed by the identical move, retains the complete 100 x 50 selection.

The captures compare engine selection geometry; they are not captures from two different builds. The options bar is unchanged by these programmatic commands. Actual Fixed Size mouse clicks and outline drags are separately covered by `fixed_size_keeps_the_off_canvas_part_when_its_outline_moves` in `marquee_tests.rs`.

These files are PR evidence only. The fix branch does not include this evidence commit.
