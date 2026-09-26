@echo off
rem Starts the TENANT RS-92 standalone on the default WASAPI device (the app uses the device's own sample rate and the device's own buffer size).
rem Set RS92_MIDI to your MIDI input name (run with --midi-input "" to list them).
set EXE=%~dp0tenant-rs92.exe
if "%RS92_MIDI%"=="" (
  "%EXE%" --backend auto %*
) else (
  "%EXE%" --backend auto --midi-input "%RS92_MIDI%" %*
)
