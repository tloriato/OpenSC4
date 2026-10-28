@echo off
rem Build the x64 helper that runs one x87 transcendental on the host CPU: build.cmd <output folder>
setlocal
set "PROVE_OUT=%~1"
if "%PROVE_OUT%"=="" exit /b 2
if not exist "%PROVE_OUT%" mkdir "%PROVE_OUT%"
rem The Python caller resolves this installation with vswhere.
set "PROVE_VS=%~2"
if "%PROVE_VS%"=="" (echo no Visual Studio C++ tools found & exit /b 1)
call "%PROVE_VS%\VC\Auxiliary\Build\vcvars64.bat" >nul || exit /b 1
ml64 /nologo /c /Fo"%PROVE_OUT%\x87host.obj" "%~dp0host.asm" || exit /b 1
link /nologo /DLL /NOENTRY /NODEFAULTLIB /EXPORT:x87host /OUT:"%PROVE_OUT%\x87host.dll" "%PROVE_OUT%\x87host.obj" || exit /b 1
