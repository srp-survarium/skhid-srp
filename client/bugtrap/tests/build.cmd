@echo off
rem Run from an x86 VS developer command prompt. Rust DLL must be built first.
setlocal
set OUT=%~dp0..\target\i686-pc-windows-msvc\release
pushd "%OUT%"
cl /nologo /MT /D_WIN32_WINNT=0x0600 /Fecrash_probe.exe "%~dp0crash_probe.cpp" /link BugTrap.dll.lib user32.lib
if errorlevel 1 exit /b 1
cl /nologo /MT /LD /Fedelayed_dbghelp.dll "%~dp0delayed_dbghelp.cpp" /link /DEF:"%~dp0delayed_dbghelp.def"
if errorlevel 1 exit /b 1
cl /nologo /MT /D_WIN32_WINNT=0x0600 /Fegame_crash_test.exe "%~dp0game_crash_test.cpp" /link user32.lib gdi32.lib
if errorlevel 1 exit /b 1
cl /nologo /c /GS- /Zl /Fogame_crash_proxy.obj "%~dp0game_crash_proxy.cpp"
if errorlevel 1 exit /b 1
lld-link /dll /machine:x86 /nodefaultlib /entry:DllMain@12 /out:d3d11.dll /def:"%~dp0game_crash_proxy.def" game_crash_proxy.obj kernel32.lib user32.lib
cl /nologo /MT /D_WIN32_WINNT=0x0600 /Feui_probe.exe "%~dp0ui_probe.cpp" /link user32.lib gdi32.lib
if errorlevel 1 exit /b 1
cl /nologo /MT /D_WIN32_WINNT=0x0600 /Felogs_probe.exe "%~dp0logs_probe.cpp" /link advapi32.lib
if errorlevel 1 exit /b 1
popd
