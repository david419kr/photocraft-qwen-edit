# ComfyUI Qwen Image 2.1 image editing

This branch adds two native commands and a contextual canvas bar. Generation runs in an
existing local ComfyUI server. Original pixel data remains intact; each successful request
adds one new top-level layer in one undo step. Whole-image edits also hide the previous layers
on success so the new result's transparency is visible; Undo restores their visibility.

## AVIF import/export (2026-10-09)

The development portable build also includes a pure-Rust AVIF reader. Open or drag/drop
`.avif` files normally; no DLL, Windows codec installation or ComfyUI connection is needed.
8-bit and 10/12-bit SDR pictures, alpha and embedded metadata are preserved on import. Untagged wide-gamut/HDR
AVIF is refused explicitly; see `crates/codecs/README.md` for decoder limits.

The desktop and CLI builds also enable AVIF writing. Use File > Save As and choose AVIF,
or File > Export > Export As and select AVIF. Export As offers quality 1–100 and transparency;
Save As uses the codec's default quality (90). The output is a flattened 8-bit RGB/RGBA image.
HDR/high-depth export and ICC/EXIF/XMP preservation are deferred; existing export warnings
report depth/metadata/layer loss. Colours are converted to sRGB when the document carries
another RGB profile. The encoder's license notices ship under `licenses/avif-encoder.txt`.
CLI example: `photocraft-cli convert input.png output.avif --quality 85`.

AVIF export validation: 361 codec tests (including 24 format/depth/layout AVIF round trips),
898 codec/IO corpus-regression tests passed (five existing opt-in benchmarks ignored),
80 desktop/CLI tests and three export-dialog tests passed. Clippy, wasm with optional AVIF
encode/decode, layering and the static-CRT portable release build passed. Korean offscreen
Export As shows AVIF, quality and transparency correctly. A release benchmark of a uniform
6000x4000 RGB8 image at quality 85 took 21.655 seconds; the saved AVIF reopens at the original size.

Verified on 2026-10-09: 10 AVIF tests; 564 codec regression/corpus tests passed (three existing
opt-in benchmarks ignored); 79 desktop/CLI tests and 46 build-tool tests passed. Codec/tool
Clippy, native portable release build, wasm checks (including optional AVIF), and layering
checks passed. The Korean offscreen UI opens a synthetic RGBA AVIF with transparency.
The reported 1216x1600 file decodes with at most 2/255 RGB difference from FFmpeg; a synthetic
24 MP file decodes in 883 ms in release. The executable depends on Windows system DLLs only.

## Use

Run `scripts/run-comfyui-korean.ps1` on Windows. It builds a portable copy under
`dist/photocraft-comfyui-korean` with a separate settings directory and Korean selected on
first launch. It does not replace the installed PhotoCraft or its preferences. Rust 1.95.0
is required. `-DebugBuild` builds the debug profile; `-NoBuild` reuses an existing binary.

Prebuilt Windows x64 users need no Rust installation. The launcher now follows official
Windows packaging and statically links the Microsoft C runtime into `photocraft.exe`
(`--target x86_64-pc-windows-msvc`, `-C target-feature=+crt-static`). No separate
`VCRUNTIME140.dll` or Visual C++ runtime installer is needed for this executable. Standard
Windows system libraries remain required. `portable.txt` is optional for execution and
enables settings beside the executable. For sharing, use the executable, marker and `licenses/` folder rather
than copying personal `PhotoCraftData` (preferences, recent
files and recovery data); the recipient can select Korean in Preferences.

`-NoBuild` reuses the static build in `target/x86_64-pc-windows-msvc/<profile>`, not an older
dynamic development build in `target/<profile>`. Packaging removes the runtime DLL bundled
by the previous launcher. Target build flags are restored when the launcher exits.

Start ComfyUI separately. The default endpoint is `http://127.0.0.1:8188`. Change it in
Edit > Preferences > Integrations > ComfyUI URL. Only loopback HTTP(S) endpoints are supported.
The AI floating bar starts hidden. A background, read-only startup check verifies the three
model filenames and the Qwen encoder/cache nodes; it never starts ComfyUI, loads weights or
submits a generation. Failed checks keep the bar hidden. Availability is rechecked about
every 15 seconds, so starting/stopping ComfyUI or adding missing models updates the bar
without restarting PhotoCraft. Changing the endpoint resets the check immediately. Checks
use a 1-second connect timeout and 2-second request timeout and do not block UI interaction.
`ui.inspect.ai` exposes `available`, `checking` and `availabilityError`. Prompt drafts survive
temporary service loss; document changes reset the draft while preserving service state.
This first version uses these model files:

- `qwen_image_2.1_int8_convrot.safetensors`
- `qwen3vl_8b_int8_convrot.safetensors` (`qwen_image` CLIP type)
- `qwen_image_2.1_vae_bf16.safetensors`

Before uploading an image, PhotoCraft queries `/object_info/UNETLoader`,
`/object_info/CLIPLoader` and `/object_info/VAELoader` for their model lists. The exact filenames
above are resolved to the paths ComfyUI advertises, including nested folders and Windows
backslash paths; the resolved paths are submitted unchanged. An exact root-folder model
takes precedence. A single matching filename in a subfolder is selected automatically;
multiple subfolder matches produce an explicit ambiguity error. Missing models or malformed
model lists fail before uploading or submitting a generation. Different model variants or
renamed filenames are not substituted automatically. The server, not PhotoCraft, controls
which model directories and extra model paths are searched. These metadata requests do not
load model weights or run inference.

The API graphs follow the supplied `03_Edit_AutoINT8Cache.json` and
`04_MaskEdit_AutoINT8Cache.json`: 25 steps, CFG 1, Euler/simple, denoise 1, and
QwenImage21Cache auto/INT8. The user's GUI workflow files are not rewritten.

With no selection or a full selection, click **Prompt Edit** below the image and describe
the change. The prompt is passed through unchanged. With a partial selection, click
**Generative Fill** below the selection. A blank prompt removes the content in the selected
region; a nonblank prompt specifies its replacement. Cancel stops only this ComfyUI job.
The closed whole-image bar also has a one-click **Remove Background** button. It uses the
05 BackgroundRemoval workflow's RGBA/full-transparency instruction, generalised from its
example woman to foreground subjects, hair/fur, clothing and held objects. The selected-region
bar has a one-click **Remove** button, identical to Generative Fill with an empty prompt.
These shortcuts leave any draft prompt unchanged and are disabled while the document is busy.
With the prompt field focused, **Ctrl+Enter** starts generation through the same command and
validation as the Generate button. Plain Enter still inserts a newline. Empty whole-image
prompts and busy jobs do not submit; empty Generative Fill prompts retain removal behaviour.

The original selection becomes the result layer's mask. Outside-selection pixels stay
unchanged even if the model changes the corresponding pixels. Feather is applied once.
Generative Fill also applies source-gradient Poisson blending at model resolution before
resampling and conversion to the document profile. It interpolates the original/generated
colour difference from outside the selection into the filled region, preserving generated
texture while matching surrounding colour and brightness. Original content inside the
selection and synthetic padding are excluded from the boundary reference. If no unselected
context survives at model resolution, blending is skipped. This applies to both blank and
nonblank fill prompts; new objects can also inherit the surrounding colour cast. Prompt Edit
does not apply this correction. The solve supports job cancellation and uses no AI inference.
Existing pixels and the selection are preserved. Fill leaves source visibility unchanged;
hide or undo its result to return to the previous image. Prompt Edit and AI Remove Background
hide all existing root layers only after the generated layer is ready. This includes visible
source groups; their child visibility flags are preserved. Undo restores the exact old root
visibility and active layer in the same step. Failure/cancellation keeps the source visible.
To compare a whole-image result manually, hide it and re-enable the source layers.

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

Empty-prompt removal supplies a primary reference with the selected area painted bright
green, not a latent noise mask. The prompt identifies the paint as an edit annotation and
removes the marked shapes to reconstruct their background. This replaces the
previous binary silhouette secondary reference, which could cause Qwen to edit the wrong
object despite a correctly positioned mask. A secondary colour guide could also be copied
as object colour; placing the annotation directly in the primary reference avoids that case.
The uploaded PNG stores `alpha = 1 - selection`, preserving RGB under transparent mask
pixels, so LoadImage produces the correct selection. Stock `EmptyImage` and
`ImageCompositeMasked` nodes build the guide without a second upload or an extra model.
Inputs are already padded to multiples of 32, so the annotation shares exact image coordinates.
Original pixels underneath the paint are not reference content for the model; unmarked image
context supplies the style and background. Fill with a nonblank prompt retains the original
two-reference MaskEdit graph (unmarked original + separate binary selection) and its prompt
formatting, preserving existing content-edit behaviour. Prompt Edit also keeps its original
unmarked reference. The engine passes an explicit removal flag to the native backend.
The selection still independently masks the result layer; the green guide does not change
selection coverage or Poisson blending. The guide paint is excluded from the requested output.
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
The desktop UI does not supply a fixed seed. Each request defaults to the current native
system time in nanoseconds as a 64-bit seed; an explicitly supplied `seed` is honoured for
reproducible automation and diagnostic comparisons.
`edit.aiRemoveBackground` accepts optional `seed`/`name` and uses its fixed foreground-cutout
prompt, then follows the whole-image result/visibility path. This AI quick action is separate
from the existing local `layer.removeBackground` mask-based command.

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
Fill harmonisation regression tests cover a simulated colour shift over removed content,
texture preservation, selections at canvas edges, unchanged exterior pixels, 8/16/32-bit
documents, Undo, padding exclusion, missing exterior context, cancellation and malformed
buffers. The new harmonisation step has not yet been evaluated with a real Qwen result.

### User runtime verification — 2026-10-08

The user tested the portable build against their actual local ComfyUI installation and
reported that the integration works successfully. This is user-confirmed runtime evidence;
the agent did not run additional real Qwen generation. The user also noticed some
improvements to make; their details remain to be collected and addressed in follow-up work.
Generation quality, latency and VRAM usage have not been measured quantitatively.

### Removal region regression — 2026-10-09

The reported white-background drawing had a correct lower-left character selection, but
the old binary-mask request left that character and removed the upper-left mirror in the
raw ComfyUI output. The inverted-background request removed the character. At fixed original
seeds and unchanged model/sampler settings, a primary reference annotated in bright green
removed the character on both backgrounds. A secondary colour guide was rejected because
it could recolour the character instead. Prompted edits retain their original two-reference
route; the change is limited to empty-prompt removal. Diagnostic outputs are local-only under
`target/qwen-mask-diagnostic`, not committed source assets. Model adherence remains
probabilistic; these checks establish the supplied regression cases, not every possible image.
