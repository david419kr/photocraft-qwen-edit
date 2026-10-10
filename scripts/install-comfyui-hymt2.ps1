param([string]$ComfyRoot = 'D:\ai_drawing\ComfyUI_windows_portable\ComfyUI', [string]$Python = '3.13')
$ErrorActionPreference = 'Stop'
$node = Join-Path $ComfyRoot 'custom_nodes/ComfyUI-Hy-MT2'
if (!(Test-Path -LiteralPath (Join-Path $node 'py/hy_mt_gguf_node.py'))) {
    & git clone https://github.com/1038lab/ComfyUI-Hy-MT2.git $node
    if ($LASTEXITCODE -ne 0) { throw 'Hy-MT2 node clone failed' }
}
$runtime = Join-Path $node 'runtime'
New-Item -ItemType Directory -Path $runtime -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'hy-mt2-runtime/llama_worker.py') -Destination $runtime
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'hy-mt2-runtime/isolated_llama.py') -Destination (Join-Path $node 'py')
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'hy-mt2-runtime/requirements.txt') -Destination (Join-Path $runtime 'requirements.txt')
$venv = Join-Path $node '.venv'
if (!(Test-Path -LiteralPath (Join-Path $venv 'Scripts/python.exe'))) {
    & uv venv --python $Python $venv
    if ($LASTEXITCODE -ne 0) { throw 'Dedicated Python environment creation failed' }
}
& uv pip install --python (Join-Path $venv 'Scripts/python.exe') --requirements (Join-Path $runtime 'requirements.txt')
if ($LASTEXITCODE -ne 0) { throw 'Dedicated llama-cpp-python installation failed' }
$sourcePath = Join-Path $node 'py/hy_mt_gguf_node.py'
$source = [IO.File]::ReadAllText($sourcePath).Replace("`r`n", "`n")
if (!$source.Contains('from .isolated_llama import Llama')) {
    if (!$source.Contains('from llama_cpp import Llama')) { throw 'Unsupported Hy-MT2 node revision: import not found' }
    $source = $source.Replace('from llama_cpp import Llama', 'from .isolated_llama import Llama')
    $source = $source.Replace('        self.llm = Llama(', "        if self.llm is not None:`n            self.llm.close()`n        self.llm = None`n        self.current_signature = None`n        self.llm = Llama(")
    $source = $source.Replace("    def unload_model(self):`n        self.llm = None", "    def unload_model(self):`n        if self.llm is not None:`n            self.llm.close()`n        self.llm = None")
}
$source = $source.Replace("        if self.llm is not None:`n            self.llm.close()`n        self.llm = Llama(", "        if self.llm is not None:`n            self.llm.close()`n        self.llm = None`n        self.current_signature = None`n        self.llm = Llama(")
$source = $source.Replace('[HyMT GGUF] llama-cpp-python is not installed. Please install it using: pip install llama-cpp-python', '[HyMT GGUF] dedicated runtime is missing. Run install-comfyui-hymt2.ps1.')
[IO.File]::WriteAllText($sourcePath, $source, [Text.UTF8Encoding]::new($false))
$requirements = Join-Path $node 'requirements.txt'
$lines = [IO.File]::ReadAllLines($requirements) | Where-Object { $_ -notmatch '^llama-cpp-python|^# llama-cpp-python lives' }
[IO.File]::WriteAllLines($requirements, @('# llama-cpp-python lives in the node dedicated .venv; use the runtime installer.') + @($lines), [Text.UTF8Encoding]::new($false))
$exclude = Join-Path $node '.git/info/exclude'
if ((Test-Path -LiteralPath (Split-Path -Parent $exclude)) -and !(Get-Content -LiteralPath $exclude -ErrorAction SilentlyContinue | Where-Object { $_ -eq '/.venv/' })) {
    Add-Content -LiteralPath $exclude -Value "`n/.venv/"
}
Write-Host "Hy-MT2 isolated runtime installed at $venv. Restart ComfyUI to load the node."
