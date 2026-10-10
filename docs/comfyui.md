# ComfyUI Qwen Image 2.1 generation and editing

## Optional local prompt translation

The expanded prompt editor's bottom row has a **Languages** icon toggle when a
supported local Hy-MT2 model is detected in ComfyUI. No translation models means
the existing UI and generation behavior stay the same. One detected model is
used automatically without a selector; multiple models show a compact 1.8B/7B
selector. The toggle and selection are persisted through the normal preference
store as `integrations.comfyuiTranslatePrompt` (default false) and
`integrations.comfyuiTranslationModel` (default `hyMt18Heretic`).

Supported files, stored under ComfyUI `models/LLM/GGUF`:

- `Hy-MT2-1.8B-heretic.i1-Q4_K_M.gguf` — `hyMt18Heretic`
- `HY-MT2-7B-Abliterated-Q4_K_M.gguf` — `hyMt7Abliterated`

Use the [isolated-runtime fork](https://github.com/david419kr/ComfyUI-Hy-MT2).
Copy **both** `install-comfyui-hymt2.cmd` and `install-comfyui-hymt2.ps1` from this
build's folder into `ComfyUI_windows_portable`, beside `ComfyUI` and
`python_embeded`. Close ComfyUI, double-click the CMD file, then restart ComfyUI.
The installer detects paths by folder structure. It downloads private uv and
extracts a verified Python 3.13.12 archive without version links, creates the
node's `.venv` using the actual interpreter path, and installs the pinned llama-cpp-python wheel
and CUDA 12.8 runtime/cuBLAS/NVRTC libraries there. System Python, uv, Git,
CUDA Toolkit and a C++ compiler do not need to be installed beforehand. A working
Windows x64 NVIDIA ComfyUI portable installation, suitable NVIDIA driver and
network access are required. GGUF model weights are installed separately.
ComfyUI's llama-cpp-python/Torch packages are not replaced; a missing host
huggingface-hub package is installed if needed. Git-free machines use the fork ZIP.
Existing non-fork nodes are backed up under `.hymt2-backups` outside `custom_nodes`;
clean Git forks update with fast-forward only. Rerun the installer after moving
the portable folder to repair its environment. No global PATH is changed.

If an older installer failed at `Failed to create Python minor version link
directory`, download the updated root PS1 and run the same CMD again. A valid
patch-version Python extracted before the failure is reused. No manual deletion
is needed; `uv python install` and its minor-version junction step are bypassed.

Verified on 2026-10-11: Windows PowerShell 5.1 installation in a separate path
containing spaces/Korean, Git update, Git-free ZIP installation with backup and
environment reuse, and CUDA loading without system Toolkit paths. The installed
node translated a short Korean greeting to English with the 1.8B Heretic model
and released its worker. Other PCs/hardware have not been tested.

The node imports a small proxy; that proxy starts its dedicated Python worker
and exchanges UTF-8 JSON over stdin/stdout. The worker imports its own
`llama-cpp-python`. It opens no HTTP port and requires no standalone LLM server.
Switching models, explicit node unload and ComfyUI termination release the
worker. `unload_after_run=false` avoids an explicit unload after translation;
the model stays loaded while ComfyUI retains that node instance. ComfyUI may
release the instance when switching to a different graph. Its ordinary Torch
model-unload API does not directly manage these worker models.

PhotoCraft submits `HyMT_Translation_GGUF -> PreviewAny` through the existing
ComfyUI API with `translate_to=English`, `translate_style=Default`
and `unload_after_run=false`. Only the user-entered prompt is translated. The
existing mask-edit prompt shaping is applied afterward. A quick character check
skips translation when there are no non-ASCII alphabetic characters: English
letters, numbers, whitespace, punctuation (including typographic punctuation)
and emoji pass through unchanged even with the toggle on. Mixed/non-English
letters still take the translation route. This is a character heuristic, not a
language classifier for languages that use only ASCII letters. Empty-prompt removal
and the built-in English background-removal action skip translation. Reroll
reuses the original translated request. Translation errors leave the document
unchanged, and empty or oversized outputs are reported rather than submitted.
Model detection checks actual advertised filenames; the node's automatic-download
presets do not count as installed supported models.

The supplied 1.8B Heretic GGUF stores EOS token ID 3 (`$`), while its vocabulary
and the [official tokenizer configuration](https://huggingface.co/Tencent/HY-MT2-1.8B/blob/main/config.json)
use ID 120020. The dedicated worker overrides this one metadata value at load
time for that file, without modifying its weights or the node's prompts and
sampler settings. Before the correction, a short Korean request produced a long
repeated response. Afterward, the actual ComfyUI node returned a short English
translation in 1.35 s; the 7B node returned a short translation in 3.07 s on this
PC. These calls demonstrate integration, not universal translation accuracy.
The native PhotoCraft transport also detected both models and received short
English results. The Korean prompt bar was visually checked offscreen, and the
static-CRT portable build includes the icon, saved controls and runtime installer.

The user confirmed local prompt translation works in the updated build on
2026-10-11. No additional tests were run before committing.

This branch adds native AI commands and a contextual canvas bar. Generation runs in an
existing local ComfyUI server. Original pixel data remains intact; each successful request
adds one new top-level layer in one undo step. Whole-image edits also hide the previous layers
on success so the new result's transparency is visible; Undo restores their visibility.
Regeneration replaces the previous AI result layer in one undo step.

## Prompt history (2026-10-11)

The **ⓘ** icon at the far right of either floating bar opens the current document's
ordered AI prompt history. Each successful generation, whole-image edit, generative
fill or background removal adds an entry. Failed/cancelled requests add
nothing. Undo, Redo and history navigation restore the entries with the corresponding
document snapshot. Reroll replaces the previous result layer and retains that
operation's existing prompt record; repeated rerolls do not append duplicates.

Entries contain the operation type, original prompt and, when translation was used,
the translated English prompt. Generative Fill also records the meaningful selected
regions in a 3×3 canvas grid (top left through bottom right); selections spanning
several regions list them together. Empty prompts are shown as removal. Model, seed,
steps, CFG and other generation parameters are not recorded. Prompt inspection remains
available when ComfyUI is offline, via an information-only floating bar.

Each original/translated prompt has a copy icon on its right. It copies only that
complete prompt, preserving line breaks and excluding headings, numbering and
region labels. Prompts are read-only selectable text fields: Ctrl+C copies the
selection, and the right-click Copy menu copies the selection (or the whole
prompt if no text is selected). Typing/pasting cannot alter the stored history.

Copy validation on 2026-10-11: three focused UI tests passed for exact original/
English button output, native Copy-event routing without canvas-image copying,
read-only history preservation and a Korean selection in the right-click menu.
The Korean offscreen layout was inspected; UI Clippy (all targets), wasm/layering
checks and the static-CRT portable release build passed.

PNG, JPEG, WebP, TIFF, AVIF, PSD and PhotoCraft native files preserve this history on
save and reopening. It lives in XMP under `urn:photocraft:ai-prompts:1`, as a
versioned JSON list compressed with zlib and encoded as hexadecimal text. The
ordinary Export As metadata filters retain this property while removing other XMP
when requested. Formats without XMP support (such as BMP/GIF) cannot preserve the
history and retain their normal metadata-loss warnings. JPEG's 64 KiB XMP limit
can reject a very long history; save in another supported format in that case.
Third-party tools that strip metadata also strip the history.

The History panel reuses the selected UI language's menu/tool translations,
including Korean. Stored engine history labels and automation responses remain
stable English strings.

Validation on 2026-10-11: eight focused regression tests passed, including all three
AI modes, bilingual prompts, mask regions, undo/redo, branching after undo and reroll.
The same four-entry history survived 14 save/reopen cases (seven formats, both XMP
filters). AVIF XMLPacket and indexed PNG XMP tests passed. Korean offscreen views
were inspected with AI available and unavailable. Native library Clippy with
`-D warnings`, wasm checks for the changed crates, layering and menu parity passed;
the static-CRT portable release build was updated. AI tests used a synthetic backend;
no actual Qwen generation or translation inference was needed for this change.

The user confirmed the updated build works on 2026-10-11. No additional tests
were run before committing this change.

Prompt refinements on 2026-10-11: English/punctuation-only requests skip the
translator even with its toggle enabled, and rerolls retain the existing prompt
history instead of appending a duplicate. All 26 AI command regression tests
passed, covering all three modes, repeated rerolls and undo/redo. Engine Clippy,
wasm and layering checks passed, and the portable release build was updated.
A broader `prompt` filter also included the unchanged Turbo mock-API test, which
failed with `no supported Qwen Image 2.1 checkpoint found`; the focused command
tests passed. No live ComfyUI generation or translation was called for these
refinements.

The user approved committing these two refinements on 2026-10-11. No further
tests were run for that commit. The failed Turbo check used a test-only mock
server, not the running ComfyUI installation; its exact failure cause remains
unconfirmed.

## AVIF import/export (2026-10-09)

The development portable build also includes a pure-Rust AVIF reader. Open or drag/drop
`.avif` files normally; no DLL, Windows codec installation or ComfyUI connection is needed.
8-bit and 10/12-bit SDR pictures, alpha and embedded metadata are preserved on import. Untagged wide-gamut/HDR
AVIF is refused explicitly; see `crates/codecs/README.md` for decoder limits.

The desktop and CLI builds also enable AVIF writing. Use File > Save As and choose AVIF,
or File > Export > Export As and select AVIF. Export As offers quality 1–100 and transparency;
Save As uses the codec's default quality (90). The output is a flattened 8-bit RGB/RGBA image.
XMP (including prompt history) is preserved through the Exif TIFF XMLPacket tag 700.
HDR/high-depth export and ICC/other EXIF preservation are deferred; existing export warnings
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
`ui.inspect.ai` exposes `available`, `checking`, `availableModels` and `availabilityError`. Prompt drafts survive
temporary service loss; document changes reset the draft while preserving service state.
Supported diffusion checkpoints (the compact selector in both floating bars shares one saved
choice under `integrations.comfyuiModel`):

- `qwen_image_2.1_int8_convrot.safetensors`
- `NoctQ_V4_int8_convrot.safetensors` (NoctQ V4, `noctQv4`; default remains `qwenImage21`)
- `qwen_image_2.1_anime_fusal_int8_convrot.safetensors` (Anime FUSAL, `animeFusal`; see below)
- `NoctQ_V4_FUSAL_int8_convrot.safetensors` (Noct-Q FUSAL, `noctQFusal`; see below)

The Base and NoctQ choices use the same text encoder:

- `qwen3vl_8b_int8_convrot.safetensors` (`qwen_image` CLIP type)

Both FUSAL choices use their matching Heretic encoder; all choices share
`qwen_image_2.1_vae_bf16.safetensors`.

Before uploading an image, PhotoCraft queries `/object_info/UNETLoader`,
`/object_info/CLIPLoader` and `/object_info/VAELoader` for their model lists. The exact filenames
above are resolved to the paths ComfyUI advertises, including nested folders and Windows
backslash paths; the resolved paths are submitted unchanged. An exact root-folder model
takes precedence. A single matching filename in a subfolder is selected automatically;
multiple subfolder matches produce an explicit ambiguity error. Missing models or malformed
model lists fail before uploading or submitting a generation. A detected supported checkpoint
plus the encoder/VAE and required nodes makes the bars available. If exactly one supported
checkpoint is detected, it is selected automatically, saved and used regardless of the previous
choice; the selector is hidden on both closed bars and the expanded prompt editor. If multiple are
detected, the selector appears and keeps the saved choice. No detected checkpoint (or missing
required support models/nodes) hides the bars. Model changes are saved through the normal preferences store and also
apply to Remove Background / Remove. Different model variants or
renamed filenames are not substituted automatically. The server, not PhotoCraft, controls
which model directories and extra model paths are searched. These metadata requests do not
load model weights or run inference.

Single-checkpoint UI verification (2026-10-09): six AI bar tests passed, including either
checkpoint alone with an opposite remembered choice, persisted selection, request model,
repeated probes without extra saves, both checkpoints and zero checkpoints. Korean offscreen
snapshots confirm hidden selectors in edit/fill/generate and expanded input, a hidden bar for
zero models, and a visible selector for both models. UI Clippy, formatting, wasm, layering and
the static-CRT portable release build passed. No additional real model inference was run.

NoctQ V4 was installed on 2026-10-09 from repository revision
`a81b9af51120a78e285e57906f2250a2a02080e9`: 7,256,784,368 bytes, SHA-256
`4d92d5538253ab36e6a73b056cce5f697950f4303cb9e6686a3198d0ea13f9e8`.
Its 649 tensor names, shapes and dtypes match the installed Qwen checkpoint. Weights live in
ComfyUI's `models/diffusion_models`, with the upstream LICENSE/NOTICE beside them; they are
not bundled into PhotoCraft. NoctQ uses the upstream Qwen Research License (non-commercial
use only): [model license](https://huggingface.co/Noctaluna/Noct-Q-Uncensored-Qwen-Image-2.1/blob/main/LICENSE).

Validation: both models completed real Prompt Edit and blank Generative Fill through the
PhotoCraft engine/ComfyUI transport, with new layers and original layers retained. A NoctQ
512x512 synthetic blue-circle test created the circle and removed it back to a white background
(7.9s edit, 5.8s fill). Very small 128x128 removal outputs were less reliable, so these smoke
tests establish integration, not a blanket quality guarantee for arbitrary artwork. Mock tests
cover both workflow modes, NoctQ-only installs, filename/subfolder resolution, all quick actions,
saved choice restoration and older preferences. Korean UI snapshots cover both closed bars
and the open prompt bar.
Preference restoration and availability/UI tests passed; engine/UI Clippy, wasm, layering
and static-CRT portable release build also passed. Saved model choice is shared across image
tabs and both editing modes; a job keeps the model that was selected when it was submitted.

### Anime FUSAL checkpoint (2026-10-10)

The selector also recognises `qwen_image_2.1_anime_fusal_int8_convrot.safetensors` as
**Anime FUSAL** (`animeFusal`). It uses the original, non-Turbo Qwen Image 2.1 sampling
graph at 25 default steps, CFG 1.0, Euler/simple. The matching
Heretic text encoder is required for this choice: prefer
`qwen3-vl-8b-heretic-1.3.0-int8convrot.safetensors`, then
`qwen3-vl-8b-heretic-1.3.0_bf16.safetensors`. Other model choices prefer
`qwen3vl_8b_int8_convrot.safetensors`; if absent, use Heretic INT8 ConvRot,
then Heretic BF16. Detection and actual workflow submission use the same priority,
including files in subfolders. If the FUSAL encoder is
absent, availability omits the FUSAL choices. The shared Qwen Image 2.1 VAE is still required.
No prompt text or mask prompt shaping is changed. For the model author's suggested style
trigger, users may include `fusal style.` in their own prompt.

The installed checkpoint was made from the official Qwen Image 2.1 BF16 Base plus
[Anime FUSAL's Exact rank-80 LoRA](https://huggingface.co/MicksHF/Qwen-Image-2.1-Anime-Fusal?not-for-all-audiences=true)
at strength 1.0, then quantized once with
[Comfy-Org's INT8 ConvRot tool](https://github.com/Comfy-Org/comfy-model-tools).
The pinned source revisions were `Comfy-Org/Qwen-Image-2.1@df94239739eef7205973a145ffa6f441f7b64e84`
and `MicksHF/Qwen-Image-2.1-Anime-Fusal@10adaec162abbf0ce8f88c41e3db16e90d19d38f`.
This avoids merging into an already quantized checkpoint. The LoRA author recommends
30–40 steps and the matching Heretic BF16 encoder. Their LoRA is for the original Base,
not Turbo. The installed checkpoint has 649 tensors, with the same names, shapes and
types as the official INT8 checkpoint; the 192 quantized layers had mean 0.890% and
worst 1.006% relative reconstruction error against the merged BF16 weights. Weights
live only in ComfyUI model folders, not the PhotoCraft repository or executable.
The installed INT8 file is 7,256,783,288 bytes, SHA-256
`cf909682a3893ef5c435ba377b15211d05fb1431a99ee4081988871b4d64078d`.
Upstream Qwen Research and Heretic Apache 2.0 license files are stored beside the
respective model files. The FUSAL repository's model card and license govern usage.
The original BF16 base and Exact LoRA were retained for reproducibility. The
temporary merged BF16 output was removed after quantization and inference checks.
Do not load the Exact LoRA on top of the Anime FUSAL INT8 checkpoint: it is already
merged at strength 1.0.

Before the default was lowered to 25, a real PhotoCraft engine request on the local
RTX 4090 generated a 768×768 anime illustration at 35 steps in 16.11 s. A second request edited the blue
teapot to red in 12.78 s, creating a separate layer and preserving the source.
An empty-prompt selected Generative Fill also removed the teapot in 11.88 s
without replacing the document's other layers. All three outputs were visually
inspected. This checks the installed model, matching encoder and PhotoCraft
transport for those requests; other prompts and masks may behave differently.
PhotoCraft's live availability probe found Anime FUSAL among six installed
supported checkpoints. The native static-CRT portable executable was rebuilt
successfully at `dist/photocraft-comfyui-korean/photocraft.exe`.

### Noct-Q FUSAL trial checkpoint (2026-10-10)

`NoctQ_V4_FUSAL_int8_convrot.safetensors` appears as **Noct-Q FUSAL** (`noctQFusal`)
in both floating bars and uses 25 default steps, CFG 1.0, the matching Heretic
text encoder (INT8 ConvRot preferred over BF16) and the shared Qwen VAE. Anime FUSAL's default was also lowered to 25
at the user's request. Existing prompt shaping and selection behavior are unchanged.

This trial combines the installed NoctQ V4 INT8 ConvRot checkpoint with the same
Exact rank-80 FUSAL LoRA at strength 1.0. Noct-Q does not provide a public BF16 base.
For each of the 192 affected matrices, the converter reconstructs the rotated
weights in FP32 as `INT8 * scale`, rotates the LoRA A matrix to the same coordinates,
adds `B * rotated(A)` and requantizes using per-row absmax scales. It creates no
intermediate BF16 checkpoint. The other tensors and ConvRot settings are retained.
The output is 7,256,783,416 bytes. Additional quantization reconstruction error
against the combined reconstructed source was mean 0.651% and maximum 0.821%; this
does not measure Noct-Q's original quantization loss or generated image quality.
The source NoctQ file is preserved. Do not load the FUSAL LoRA again on this checkpoint.

The author trained FUSAL on official Base Qwen, so its combination with Noct-Q is an
experimental choice. No additional inference or test suite was run for this change,
as requested. Model weights and conversion scripts stay outside tracked app source.

The user confirmed the updated build works on 2026-10-10. This is a user-reported
runtime result; no further automated tests were run before committing.

The API graphs follow the supplied `03_Edit_AutoINT8Cache.json` and
`04_MaskEdit_AutoINT8Cache.json`: 25 steps, CFG 1, Euler/simple, denoise 1, and
QwenImage21Cache auto/INT8. The user's GUI workflow files are not rewritten.

### Turbo checkpoints and sampling (2026-10-10)

The model selector also recognises the official `qwen_image_2.1_turbo_int8_convrot.safetensors`
and the GGUF filenames from [AtomicChat's Turbo collection](https://huggingface.co/AtomicChat/Qwen-Image-2.1-Turbo-GGUF)
(Q8_0, AD-Q2_K/Q3_K/Q4_K/Q5_K/Q6_K and BF16), plus the standard Turbo
Q3_K_M/Q4_K_M/Q4_K_S/Q5_K_M/Q6_K filenames. Only detected models appear in the menu. This PC
has the official INT8 ConvRot, AtomicChat Q8_0 and AD-Q4_K; the other quantizations are
registered for installations that actually contain them. Neither weights nor outputs are
included in the PhotoCraft repository or portable executable.

Base Qwen 2.1 and NoctQ retain 25 steps. Turbo defaults to eight steps with CFG 1.0 and the
[published eight-step sigma schedule](https://huggingface.co/Qwen/Qwen-Image-2.1-Turbo/blob/main/model_index.json),
using ComfyUI's ManualSigmas/KSamplerSelect/SamplerCustom nodes. The little number field in each
closed or expanded bar accepts 1–1000 steps; the adjacent circular arrow restores the selected
model's 25/8 default. Changing the model also restores its default. The UI override lasts for the
current document until it changes; `ui.set {"aiSteps":null}` restores the default.
Other step counts linearly resample the published curve and are experimental; the model maker
evaluated the eight-step setting. All existing prompt shaping, mask handling, colour correction,
layer visibility and one-click regeneration remain the same. Regeneration uses the original
request's step count even if the field has since changed.

For GGUF, install [ComfyUI-GGUF](https://github.com/city96/ComfyUI-GGUF) and its `gguf` Python
dependency in the ComfyUI portable environment. AtomicChat's files omit architecture metadata;
the currently installed GGUF loader also passes Q8_0's packed one-dimensional norm weights
directly to Qwen's fused operator. Run the bundled `fix-comfyui-gguf-qwen21.ps1 -ComfyRoot
<your ComfyUI directory>` once, then restart ComfyUI. This small compatibility patch recognises
Qwen Image 2.1 and unpacks one-dimensional norm weights in memory. It leaves GGUF files and
their quantization unchanged. Reapply it after a GGUF node update if that update has not yet
included equivalent support. The official INT8 variant uses stock UNETLoader and needs no GGUF
node. Keep the common text encoder and VAE from the original setup.

On the local RTX 4090, the AtomicChat Q8_0 model completed a real 1021×765 canvas T2I request
at a 1024×768 working size, plus a new-seed reroll, whole-image edit, background cutout,
selection removal and a custom six-step edit. The first T2I took 15.68 s; its PNG and edit
result were inspected without visible corruption. The reported `[136]` versus `[128]` error
occurred before the norm-loader fix and did not recur in those calls. The user separately
confirmed that AD-Q4_K works and the official INT8 ConvRot runs much faster. These observations
cover those requests, not every prompt, size, or GPU.

### Blank canvas: Prompt Generate

Under the normal whole-image condition (no selection or a full selection), an entirely
transparent or entirely opaque pure-white visible canvas shows **Prompt Generate** instead
of Prompt Edit. Hidden layers are ignored. Other solid colours, near-white, and mixed white
and transparent pixels are not blank. A partial selection always shows **Generative Fill**,
including on a blank canvas. Classification is cached per document revision and selection;
it does not render the full document on every UI frame.

Prompt Generate uses `01_T2I_Quality_4MP_25step.json`: basic CLIPTextEncode,
ConditioningZeroOut, EmptyLatentImage, KSampler, VAEDecode and SaveImage. It uses the selected
Qwen/NoctQ checkpoint with the shared encoder/VAE, 25 steps, CFG 1 and Euler/simple. The user's
prompt is passed unchanged, without an input image, upload or edit/cache graph. The GUI
workflow's resolution selector is replaced by canvas dimensions rounded up to multiples of
32. Right/bottom padding is cropped after decoding, yielding the exact original canvas size.
Unlike editing, T2I does not reduce canvases to 1536 pixels. It retains the 36-megapixel document
limit and caps each side at 16384 pixels; larger sizes can require substantially more VRAM.
The result is a new layer and existing root layers are hidden only on success.

### One-click regeneration

After a successful generation, edit, fill or quick action, the floating bar shows a refresh
icon. It uses the original source image/selection, prompt, model, endpoint and working size
with a different seed. It replaces the same AI layer, preserving the layer ID and avoiding
accumulated results. Undo restores the preceding result. Failure/cancellation keeps it intact.
The icon also appears in the expanded prompt editor; it ignores subsequent draft text and
model-selector changes. Its tooltip identifies the model used by the previous request.

The repeat snapshot is per document and lasts only for the current session. Additional
document/selection edits or Undo invalidate it, so regeneration cannot overwrite subsequent
retouching. Closing the document frees the snapshot. Opening another document preserves each
document's eligible result. A new generation refreshes that document's snapshot.

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

For Prompt Edit and Generative Fill, working resolution has a maximum side of 1536 pixels. Both image and mask use the same
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
{"method":"engine.execute","params":{"command":"edit.promptGenerate","params":{"prompt":"Watercolour landscape","seed":1234},"wait":false}}
```

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
`edit.aiRegenerate` accepts `{}` and always chooses a different seed; an explicit `seed`
is rejected. It requires an unchanged successful AI result in the active document.
`edit.aiRemoveBackground` accepts optional `seed`/`name` and uses its fixed foreground-cutout
prompt, then follows the whole-image result/visibility path. This AI quick action is separate
from the existing local `layer.removeBackground` mask-based command.

`ui.inspect` includes `ai` (open, prompt, mode, rectangle). `ui.set` accepts `aiOpen` and
`aiPrompt`. `ui.menu.invoke` with any prompt command and no parameters opens the input bar.
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
buffers. These synthetic shift tests validate the harmonisation calculation; individual live
fill checks do not establish colour matching for all artwork.

### T2I and regeneration verification — 2026-10-09

Both installed Qwen 2.1 and NoctQ V4 completed real Prompt Generate, Prompt Edit, Generative
Fill and one regeneration of each through the PhotoCraft engine and local ComfyUI server
(12 requests). T2I used a 509x377 canvas, generated at 512x384, and exported at exactly 509x377.
The initial T2I calls took 7.8s / 7.4s respectively; the corresponding rerolls took 4.5s / 4.4s.
The watercolor-teapot results were inspected. Rerolls changed the seed, retained the result
layer ID and kept the layer count unchanged. These are integration smoke checks, not a
quality guarantee or a large-canvas VRAM benchmark.

Targeted engine tests cover blank detection at 8/16/32-bit depth, partial/full selection
precedence, exact padding/crop, T2I above the edit-resolution cap, captured source/prompt/model,
same-layer reroll, Undo/Redo, failure and document closing. The T2I mock transport test checks
both models and rejects any image upload. Korean offscreen UI snapshots were inspected for
the blank-canvas bar, partial-white fill bar, open generator and post-result refresh buttons.
An offscreen refresh-icon click dispatched `edit.aiRegenerate`. Engine/UI Clippy, formatting,
wasm, layering, adversarial-command panic hunt and the static-CRT portable release build passed.
Menu wiring and scorecard documents were regenerated (631/631 live menu items).

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
