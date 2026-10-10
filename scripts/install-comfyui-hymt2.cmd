@echo off
rem SPDX-License-Identifier: MIT
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0install-comfyui-hymt2.ps1" %*
set "result=%errorlevel%"
if not "%result%"=="0" echo Hy-MT2 installation failed. See the error above.
pause
exit /b %result%
