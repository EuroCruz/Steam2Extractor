@echo off
setlocal enabledelayedexpansion
cd /d "%~dp0"

if exist dist rmdir /s /q dist
mkdir dist

cargo build --release --bin steam2extract_f
set "HASHER=%CD%\target\release\steam2extract_f.exe"

for %%T in (x86_64-unknown-linux-gnu i686-unknown-linux-gnu x86_64-pc-windows-gnu i686-pc-windows-gnu) do (
    cargo build --release --target %%T --features steam2-cli/debug-tools --bin steam2extract_d
    cargo build --release --target %%T --bin steam2extract_f

    mkdir dist\%%T
    set EXT=
    echo %%T | findstr "windows" >nul && set EXT=.exe

    copy /y "target\%%T\release\steam2extract_f!EXT!" "dist\%%T\" >nul
    copy /y "target\%%T\release\steam2extract_d!EXT!" "dist\%%T\" >nul
    xcopy /e /i /y "target\%%T\release\bin" "dist\%%T\bin" >nul

    copy /y "NOTICE" "dist\%%T\NOTICE" >nul
    copy /y "LICENSE" "dist\%%T\LICENSE" >nul

    set "PREFIX=%CD%\dist\%%T\"
    (
        echo Steam2Extract build log
        echo target: %%T
        echo date:   %date% %time%
        echo.
        echo files:
        for /r "dist\%%T" %%F in (*) do (
            if /i not "%%~nxF"=="build.log" (
                set "FULL=%%F"
                set "REL=!FULL:%PREFIX%=!"
                for /f "delims=" %%H in ('"!HASHER!" hash "!REL!"') do set "PHASH=%%H"
                echo   !REL! %%~zF bytes  pandemic=!PHASH!
            )
        )
    ) > "dist\%%T\build.log"
)
