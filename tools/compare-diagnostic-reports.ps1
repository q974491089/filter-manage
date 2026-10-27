param(
    [Parameter(Mandatory = $true)]
    [string]$RustPath,

    [Parameter(Mandatory = $true)]
    [string]$PythonPath
)

$ErrorActionPreference = "Stop"

function Read-Report([string]$Path, [string]$Label) {
    if (-not (Test-Path -LiteralPath $Path)) {
        throw "$Label report not found: $Path"
    }
    $report = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($report.schema_version -ne "diagnostic-report-v1") {
        throw "$Label report has unsupported schema_version: $($report.schema_version)"
    }
    return $report
}

function Get-ModeText($Mode) {
    if ($null -eq $Mode) {
        return "unavailable"
    }
    return "{0}x{1}@{2}Hz/{3}bpp" -f $Mode.width, $Mode.height, $Mode.refresh_hz, $Mode.bits_per_pixel
}

function Get-DisplayMap($Report) {
    $map = @{}
    foreach ($display in @($Report.displays)) {
        $map[[string]$display.id] = $display
    }
    return $map
}

function Get-EventCounts($Report) {
    $counts = @{}
    foreach ($event in @($Report.events)) {
        $kind = [string]$event.kind
        if (-not $counts.ContainsKey($kind)) {
            $counts[$kind] = 0
        }
        $counts[$kind]++
    }
    return $counts
}

function Get-LatestSnapshot($Report) {
    $withSnapshots = @($Report.events | Where-Object { $null -ne $_.snapshot })
    if ($withSnapshots.Count -eq 0) {
        return $null
    }
    return $withSnapshots[$withSnapshots.Count - 1].snapshot
}

$rust = Read-Report $RustPath "Rust"
$python = Read-Report $PythonPath "Python"
$errors = New-Object System.Collections.Generic.List[string]
$warnings = New-Object System.Collections.Generic.List[string]

$rustTarget = [string]$rust.display_id
$pythonTarget = [string]$python.display_id
if ($rustTarget -ne $pythonTarget) {
    $errors.Add("Target display differs: Rust='$rustTarget' Python='$pythonTarget'")
}

$rustDisplays = Get-DisplayMap $rust
$pythonDisplays = Get-DisplayMap $python
$allDisplayIds = @($rustDisplays.Keys + $pythonDisplays.Keys | Sort-Object -Unique)
foreach ($displayId in $allDisplayIds) {
    $hasRust = $rustDisplays.ContainsKey($displayId)
    $hasPython = $pythonDisplays.ContainsKey($displayId)
    if (-not $hasRust -or -not $hasPython) {
        $errors.Add("Display exists in only one report: $displayId")
        continue
    }
    $rustMode = Get-ModeText $rustDisplays[$displayId].mode
    $pythonMode = Get-ModeText $pythonDisplays[$displayId].mode
    if ($rustMode -ne $pythonMode) {
        $errors.Add("Display mode differs for $displayId`: Rust='$rustMode' Python='$pythonMode'")
    }
}

$rustSnapshot = Get-LatestSnapshot $rust
$pythonSnapshot = Get-LatestSnapshot $python
if ($null -ne $rustSnapshot -and $null -ne $pythonSnapshot) {
    if ([string]$rustSnapshot.display_id -ne [string]$pythonSnapshot.display_id) {
        $errors.Add("Latest snapshot display differs")
    }
    if ((Get-ModeText $rustSnapshot.mode) -ne (Get-ModeText $pythonSnapshot.mode)) {
        $warnings.Add("Latest snapshot mode differs; the two captures may be from different moments")
    }
    if ([string]$rustSnapshot.gamma.shape -ne [string]$pythonSnapshot.gamma.shape) {
        $warnings.Add("Latest Gamma shape differs: Rust='$($rustSnapshot.gamma.shape)' Python='$($pythonSnapshot.gamma.shape)'")
    }
} elseif ($null -eq $rustSnapshot -and $null -eq $pythonSnapshot) {
    $warnings.Add("Neither report contains an event snapshot")
} else {
    $warnings.Add("Only one report contains an event snapshot")
}

$rustCounts = Get-EventCounts $rust
$pythonCounts = Get-EventCounts $python
$allKinds = @($rustCounts.Keys + $pythonCounts.Keys | Sort-Object -Unique)
foreach ($kind in $allKinds) {
    $rustCount = if ($rustCounts.ContainsKey($kind)) { $rustCounts[$kind] } else { 0 }
    $pythonCount = if ($pythonCounts.ContainsKey($kind)) { $pythonCounts[$kind] } else { 0 }
    if ($rustCount -ne $pythonCount) {
        $warnings.Add("Event count differs for $kind`: Rust=$rustCount Python=$pythonCount")
    }
}

Write-Host "Rust report:   $RustPath"
Write-Host "Python report: $PythonPath"
Write-Host "Target display: Rust='$rustTarget' Python='$pythonTarget'"
Write-Host "Rust events: $(@($rust.events).Count); Python events: $(@($python.events).Count)"

foreach ($warning in $warnings) {
    Write-Warning $warning
}
if ($errors.Count -gt 0) {
    throw (($errors | ForEach-Object { "Structural mismatch: $_" }) -join [Environment]::NewLine)
}

Write-Host "Structural report comparison passed. Dynamic timing differences are warnings, not schema failures."
