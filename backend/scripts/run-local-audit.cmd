@echo off
setlocal
cd /d "%~dp0.."
"target-planning-check\debug\personal-investment-backend.exe" >> "audit.stdout.log" 2>> "audit.stderr.log"
