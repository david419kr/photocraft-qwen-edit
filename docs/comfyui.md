# ComfyUI Qwen Image 2.1 image editing

This branch adds two native commands and a contextual canvas bar. Generation runs in an
existing local ComfyUI server. Original layers remain intact; each successful request adds
one new top-level layer in one undo step.

## Use

Run `scripts/run-comfyui-korean.ps1` on Windows. It builds a portable copy under
`dist/photocraft-comfyui-korean` with a separate settings directory and Korean selected on
first launch. It does not replace the installed PhotoCraft or its preferences. Rust 1.95.0
is required. `-DebugBuild` builds the debug profile; `-NoBuild` reuses an existing binary.

Start ComfyUI separately. The default endpoint is `http://127.0.0.1:8188`. Change it in
Edit > Preferences > Integrations > ComfyUI URL. Only loopback HTTP(S) endpoints are supported.
This first version uses these model files:

- `qwen_image_2.1_int8_convrot.safetensors`
- `qwen3vl_8b_int8_convrot.safetensors` (`qwen_image` CLIP type)
- `qwen_image_2.1_vae_bf16.safetensors`

The API graphs follow the supplied `03_Edit_AutoINT8Cache.json` and
`04_MaskEdit_AutoINT8Cache.json`: 25 steps, CFG 1, Euler/simple, denoise 1, and
QwenImage21Cache auto/INT8. The user's GUI workflow files are not rewritten.

With no selection or a full selection, click **Prompt Edit** below the image and describe
the change. The prompt is passed through unchanged. With a partial selection, click
**Generative Fill** below the selection. A blank prompt removes the content in the selected
region; a nonblank prompt specifies its replacement. Cancel stops only this ComfyUI job.

The original selection becomes the result layer's mask. Outside-selection pixels stay
unchanged even if the model changes the corresponding pixels. Feather is applied once.
Existing layers and the selection are preserved. Hide or undo the new layer to return to
the previous image.

## Input and output

Supported document modes are RGB, Gray, CMYK and Lab at 8/16/32-bit depth, with a maximum
document size of 36 megapixels. CMS converts the visible composite to sRGB for the model
and converts its result back to the document profile and depth. The document itself is
not converted. Model exchange is 8-bit PNG, so generated content has 8-bit source precision.
Transparent input is composited over white for the model.

Working resolution has a maximum side of 1536 pixels. Both image and mask use the same
coordinates. Right/bottom padding makes their dimensions multiples of 32 without the
workflow's central crop. The output is cropped to remove that padding and resampled to
the original document dimensions. Small details and narrow selections in large images
are limited by this working resolution.

MaskEdit supplies two reference images, not a latent noise mask: image 1 is the source,
image 2 is the selection. The uploaded PNG stores `alpha = 1 - selection`, preserving
RGB under transparent mask pixels, so LoadImage produces the correct mask orientation.
The result is read specifically from SaveImage node 56, never the mask preview.

## Commands and automation

Commands are available through the desktop control channel/MCP bridge and trusted local CLI:

```json
{"method":"engine.execute","params":{"command":"edit.promptEdit","params":{"prompt":"Change the sky to sunset","seed":1234},"wait":false}}
```

```json
{"method":"engine.execute","params":{"command":"edit.generativeFill","params":{"prompt":"","seed":1234},"wait":false}}
```

`name` optionally supplies the new layer name. `jobs.list` and `jobs.cancel` expose existing
background-job state. A job is bound to the source document; switching tabs cannot put the
result into another document. Conflicting edits to the source are blocked while it runs.

`ui.inspect` includes `ai` (open, prompt, mode, rectangle). `ui.set` accepts `aiOpen` and
`aiPrompt`. `ui.menu.invoke` with either command and no parameters opens the input bar.
The normal engine command executes only when a prompt string is supplied.

Unconfigured engine/test sessions do not contact ComfyUI. Web and unconfigured headless
MCP sessions return an explicit unsupported-frontend error; the desktop MCP bridge works.

## Failure and cancellation

Input is frozen before worker execution. Network failures, bad output dimensions, missing
models or cancellation do not modify the document. Requests have unique UUIDs. Prompt POSTs
are not blindly retried; cancellation uses `/api/jobs/{id}/cancel`, not global `/interrupt`
or queue clearing. Only this request is stopped.

## Validation

Unit tests use synthetic images and a loopback mock HTTP server. They cover prompt/mask
wiring, full versus partial selection, mask direction/padding, native bit depths, new-layer
Undo/Redo, document switching, cancellation, malformed parameters and image limits.
Build and automated visual QA use mock results and do not execute real Qwen inference.

### User runtime verification — 2026-10-08

The user tested the portable build against their actual local ComfyUI installation and
reported that the integration works successfully. This is user-confirmed runtime evidence;
the agent did not run additional real Qwen generation. The user also noticed some
improvements to make; their details remain to be collected and addressed in follow-up work.
Generation quality, latency and VRAM usage have not been measured quantitatively.
