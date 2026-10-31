@echo off
rem Builds a shareable Windows installer (.exe) of this fork. CPU build, not code-signed.
rem Unlike clean_build_windows.bat this keeps node_modules, so repeat builds are much faster.
cd /d "%~dp0"

echo Installing dependencies...
call pnpm install --frozen-lockfile
if errorlevel 1 goto fail

echo.
echo Building the installer. The first build takes a long time (often 20-40 minutes).
call pnpm tauri build --bundles nsis
if errorlevel 1 goto fail

echo.
echo Done. Your installer is here:
for %%D in ("%~dp0..\target\release\bundle\nsis" "%~dp0src-tauri\target\release\bundle\nsis") do (
  if exist "%%~D\*.exe" for %%F in ("%%~D\*.exe") do echo   %%~fF
)
goto :eof

:fail
echo.
echo The build failed. Scroll up to the FIRST line that says "error" to see why.
exit /b 1
