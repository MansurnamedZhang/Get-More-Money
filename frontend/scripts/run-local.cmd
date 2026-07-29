@echo off
setlocal
cd /d "%~dp0.."
call "D:\FlyEnv-Data\env\node\npm.cmd" run dev >> "local-ui.stdout.log" 2>> "local-ui.stderr.log"
