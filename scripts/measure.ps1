<#
.SYNOPSIS
Measures what Snowfall costs: the average CPU it takes, as a percentage of one core, and its peak memory.

.DESCRIPTION
Starts Snowfall at the given snowfall level, lets it settle, then measures it for -Seconds and prints the average CPU
use of one core and the peak working set while it runs (the passing peak of starting up is shown apart). Exits with 1
when either is over budget (by default the M1 budget: 5% of a core and 80 MB, meant for Snow on a 1080p 60 Hz
display), 2 when it can't measure.

Your Snowfall settings and its start-at-login entry are saved first and put back after, and no other Snowfall may be
running. Build first with `cargo build --release`.

.EXAMPLE
pwsh scripts/measure.ps1
.EXAMPLE
pwsh scripts/measure.ps1 -Level HeavySnow -Seconds 120 -MaxCpu 8
#>
param(
    [ValidateSet('Off', 'Snow', 'HeavySnow')] [string] $Level = 'Snow',
    [int] $Seconds = 60,
    [int] $Settle = 15,
    [double] $MaxCpu = 5.0,
    [double] $MaxMemoryMB = 80.0,
    [string] $Exe = (Join-Path $PSScriptRoot '..\target\release\Snowfall.exe')
)

$ErrorActionPreference = 'Stop'
$key = 'HKCU:\Software\Snowfall'
$run = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'

if (-not (Test-Path $Exe)) {
    Write-Host "No Snowfall at $Exe; run cargo build --release first." -ForegroundColor Red
    exit 2
}
if (Get-Process -Name Snowfall -ErrorAction SilentlyContinue) {
    Write-Host 'Snowfall is running; quit it first, so only the measured one runs.' -ForegroundColor Red
    exit 2
}

# Keep the person's own settings and autostart entry to put back afterwards.
$backup = Join-Path ([IO.Path]::GetTempPath()) "snowfall-settings-$PID.reg"
$hadKey = Test-Path $key
if ($hadKey) {
    $ErrorActionPreference = 'Continue'
    & reg.exe export 'HKCU\Software\Snowfall' $backup /y 2>&1 | Out-Null
    $ErrorActionPreference = 'Stop'
    if (-not (Test-Path $backup)) {
        Write-Host "Couldn't save your Snowfall settings to put back after; nothing was changed." -ForegroundColor Red
        exit 2
    }
}
$autostart = (Get-ItemProperty $run -ErrorAction SilentlyContinue).Snowfall

$process = $null
try {
    if ($hadKey) { Remove-Item $key -Recurse }
    New-Item $key -Force | Out-Null
    $code = @{ Off = 0; Snow = 1; HeavySnow = 2 }[$Level]
    Set-ItemProperty $key level $code -Type DWord
    # Already set, so this isn't a first run: no notification, and autostart is left alone.
    Set-ItemProperty $key openAtLogin 0 -Type DWord

    Add-Type -AssemblyName System.Windows.Forms
    $screens = [System.Windows.Forms.Screen]::AllScreens | ForEach-Object { "$($_.Bounds.Width)x$($_.Bounds.Height)" }
    Write-Host "Snowfall at $Level on $($screens -join ', '); settling for $Settle s, then measuring for $Seconds s..."

    $process = Start-Process $Exe -PassThru
    Start-Sleep -Seconds $Settle
    $process.Refresh()
    if ($process.HasExited) { throw 'Snowfall quit while settling.' }
    $cpuStart = $process.TotalProcessorTime
    $clock = [Diagnostics.Stopwatch]::StartNew()
    # The working set is sampled while measuring: starting up, compiling shaders and loading the graphics driver
    # costs a passing peak that isn't what Snowfall takes while it runs.
    $memory = 0
    while ($clock.Elapsed.TotalSeconds -lt $Seconds) {
        Start-Sleep -Milliseconds 500
        $process.Refresh()
        if ($process.HasExited) { throw 'Snowfall quit while being measured.' }
        $memory = [Math]::Max($memory, $process.WorkingSet64 / 1MB)
    }
    $cpu = ($process.TotalProcessorTime - $cpuStart).TotalSeconds / $clock.Elapsed.TotalSeconds * 100
    $startup = $process.PeakWorkingSet64 / 1MB
}
catch {
    Write-Host "Couldn't measure: $_" -ForegroundColor Red
    exit 2
}
finally {
    # Putting things back goes on past any hitch: reg.exe reports even success on stderr, which Windows PowerShell
    # would otherwise take for a failure.
    $ErrorActionPreference = 'Continue'
    if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id; $process.WaitForExit() }
    Remove-Item $key -Recurse -ErrorAction SilentlyContinue
    if ($hadKey) { & reg.exe import $backup 2>&1 | Out-Null; Remove-Item $backup -ErrorAction SilentlyContinue }
    if ($autostart) { Set-ItemProperty $run Snowfall $autostart } else { Remove-ItemProperty $run Snowfall -ErrorAction SilentlyContinue }
}

$over = $cpu -gt $MaxCpu -or $memory -gt $MaxMemoryMB
Write-Host ('CPU:    {0,6:N2}% of one core   (budget {1}%)' -f $cpu, $MaxCpu)
Write-Host ('Memory: {0,6:N1} MB peak working set while running (budget {1} MB); {2:N1} MB at most while starting' -f $memory, $MaxMemoryMB, $startup)
if ($over) {
    Write-Host 'Over budget.' -ForegroundColor Red
    exit 1
}
Write-Host 'Within budget.' -ForegroundColor Green
exit 0
