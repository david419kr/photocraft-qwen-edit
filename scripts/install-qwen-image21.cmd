@echo off
rem SPDX-License-Identifier: MIT
setlocal
chcp 65001 >nul
set "setup=%~dp0scripts\install-qwen-image21.ps1"
rem The fallback supports the source checkout and older flat bundles.
if not exist "%setup%" set "setup=%~dp0install-qwen-image21.ps1"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%setup%" -InstallerRoot "%~dp0." %*
set "result=%errorlevel%"
if not "%result%"=="0" echo Installation failed. See the message above. Rerun to resume.
pause
exit /b %result%
