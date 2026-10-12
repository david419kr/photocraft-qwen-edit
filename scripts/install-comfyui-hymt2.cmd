@echo off
rem SPDX-License-Identifier: MIT
setlocal
set "installer=%~dp0install-comfyui-hymt2.ps1"
if not exist "%installer%" set "installer=%~dp0..\qwen-image21-installer\scripts\install-comfyui-hymt2.ps1"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%installer%" %*
set "result=%errorlevel%"
if not "%result%"=="0" echo Hy-MT2 installation failed. See the error above.
pause
exit /b %result%
