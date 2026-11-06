@echo off
set PATH=%USERPROFILE%\.cargo\bin;%PATH%
cd /d "%~dp0"
cargo build --release || exit /b 1
if not exist dist mkdir dist
copy /y "target\release\welffi_animate.exe" "dist\welffi animate.exe" >nul || (echo Close welffi animate first, then run this again & exit /b 1)
echo Built dist\welffi animate.exe
