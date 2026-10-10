param([Parameter(Mandatory=$true)][string]$ComfyRoot)
$ErrorActionPreference = 'Stop'
$convertPath = Join-Path $ComfyRoot 'custom_nodes\ComfyUI-GGUF\tools\convert.py'
$loaderPath = Join-Path $ComfyRoot 'custom_nodes\ComfyUI-GGUF\loader.py'
$source = Get-Content -LiteralPath $convertPath -Raw
if (-not $source.Contains('class ModelQwenImage21(') -and -not $source.Contains('arch_list = [')) {
    throw 'Unrecognised ComfyUI-GGUF detector; no file was changed'
}
$loader = Get-Content -LiteralPath $loaderPath -Raw
$oldNorm = 'if len(shape) <= 1 and tensor.tensor_type == gguf.GGMLQuantizationType.BF16:'
$newNorm = 'if len(shape) <= 1 and tensor.tensor_type not in (gguf.GGMLQuantizationType.F32, gguf.GGMLQuantizationType.F16):'
if (-not $loader.Contains($oldNorm) -and -not $loader.Contains($newNorm)) {
    throw 'Unrecognised ComfyUI-GGUF norm loader; no file was changed'
}
$detector = @'
class ModelQwenImage21(ModelTemplate):
    arch = "qwen_image"
    keys_detect = [("img_in.weight", "modulation.1.weight", "transformer_blocks.0.attn.to_q.weight")]

'@
if (-not $source.Contains('class ModelQwenImage21(')) {
    $source = $source.Replace('arch_list = [', $detector + "`narch_list = [ModelQwenImage21, ")
    [IO.File]::WriteAllText($convertPath, $source, [Text.UTF8Encoding]::new($false))
    Write-Output 'Added Qwen Image 2.1 architecture detection'
}
if ($loader.Contains($oldNorm)) {
    $loader = $loader.Replace($oldNorm, $newNorm).Replace('# 1D tensors shouldn''t be quantized, this is a fix for BF16', '# Small 1D weights must be unpacked before fused norm and RoPE operators use them')
    [IO.File]::WriteAllText($loaderPath, $loader, [Text.UTF8Encoding]::new($false))
    Write-Output 'Enabled in-memory dequantization of 1D GGUF norm weights'
}
