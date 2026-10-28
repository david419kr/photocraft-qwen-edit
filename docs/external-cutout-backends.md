# Experimental external cutout backends

The native app can optionally use a locally installed external program for background removal. The backend is a trusted local executable; install only backends you trust. No network service or particular model is required by this protocol.

Put each backend in its own directory under `cutout-backends` beside the PhotoCraft executable. PhotoCraft discovers the first valid backend in directory-name order. If one is found, its short label is shown beside Remove Background; the toggle is saved in preferences. When disabled, the normal background-removal path is used. A missing or invalid backend is not shown.

## Manifest

Each backend directory has a UTF-8 `backend.json`:

```json
{"version":1,"id":"example","label":"AI","executable":"bin/worker.exe","args":[],"required":["model.dat"]}
```

`id` is 1–64 ASCII letters, digits, `.`, `_` or `-`; `label` is 1–8 printable characters. `executable` and all `required` entries are relative files inside the backend directory; paths that escape it are rejected. The executable is started directly, without a shell. `args` is an optional list of fixed arguments; the backend directory is its working directory. The backend owns its dependencies and must not depend on a particular document or hardcoded temporary path.

## Process contract

PhotoCraft supplies absolute paths in these environment variables:

- `PHOTOCRAFT_CUTOUT_ROOT`: backend directory.
- `PHOTOCRAFT_CUTOUT_INPUT`: PNG of the active pixel layer on the full document canvas, with existing masks, effects and other layers excluded.
- `PHOTOCRAFT_CUTOUT_OUTPUT`: destination for the backend's result.
- `PHOTOCRAFT_CUTOUT_CANCEL`: a file that appears when the operation is cancelled or times out.

The backend must write an 8-bit grayscale PNG at the output path, with exactly the input canvas dimensions: black hides and white reveals the source layer. Intermediate values represent partial coverage. It must exit zero only after the output is complete, return a nonzero code on failure, and periodically stop its own subprocesses when the cancellation file appears. The host terminates an unresponsive process after a grace period. Jobs have a ten-minute limit. Output is validated before applying a layer mask as one undoable document edit; the source pixels remain intact. Temporary input and output files are removed after the job.

Backends are optional, native-only local programs. This protocol does not grant access to other document layers or replace the WebAssembly filter plug-in API.
