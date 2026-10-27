# Qwen fork Fixed Size marquee port: initial verification

Source: `f4c245af77147cb7b8fd31e3d97234d7977f15ea`, based on fork main `d2fc4f0b944c27e966753e3df7357074d1360ae5` (PhotoCraft 0.16.0). Draft PR: https://github.com/david419kr/photocraft-qwen-edit/pull/15 .

Captured with this fork's offscreen snapshot example on Linux, CPU canvas / software Vulkan, 1440 x 900, scale 1. Both images use the same binary and a blank 100 x 100 document at 400% zoom.

The scripts create a 100 x 50 rectangle at (50, 25), then move its outline 50 pixels left:

- `before-clipped.png` uses the existing command's default canvas clipping and leaves 50 x 50 selected.
- `after-preserved.png` uses `clipToCanvas: false`, the new Fixed Size commit path, and keeps 100 x 50 selected.

These programmatic calls compare selection geometry and do not change the options bar. Actual Fixed Size clicks and outline drags are covered by the separate UI regression. This is not live Qwen/ComfyUI/CUDA acceptance and not a release build.

The PR is intentionally held in draft: the ongoing large Qwen work must land first, followed by refreshing the branch, resolving conflicts and repeating validation. These captures verify only this initial base.

Evidence files are kept separately from the application patch.
