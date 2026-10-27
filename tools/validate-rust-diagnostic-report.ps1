param(
    [string]$Path = (Join-Path ([Environment]::GetFolderPath("Desktop")) "diagnostic-report-v1.json")
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $Path)) {
    throw "Diagnostic report not found: $Path"
}

$report = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
if ($report.schema_version -ne "diagnostic-report-v1") {
    throw "Invalid schema_version: $($report.schema_version)"
}

foreach ($property in @("generated_at_ms", "tool", "os", "displays", "process_terms", "events", "probe_active")) {
    if ($null -eq $report.PSObject.Properties[$property]) {
        throw "Missing report field: $property"
    }
}

foreach ($display in @($report.displays)) {
    if ([string]::IsNullOrWhiteSpace([string]$display.id)) {
        throw "Display is missing id"
    }
    if ($null -ne $display.mode) {
        foreach ($property in @("width", "height", "refresh_hz", "bits_per_pixel")) {
            if ($null -eq $display.mode.PSObject.Properties[$property]) {
                throw "Display mode is missing field: $property"
            }
        }
    }
}

$events = @($report.events)
if ($events.Count -eq 0) {
    throw "Report has no events; start monitoring and reproduce the issue first"
}

$counts = @{}
foreach ($event in $events) {
    if ([string]::IsNullOrWhiteSpace([string]$event.kind)) {
        throw "Found an event without kind"
    }
    if ($null -eq $event.timestamp_ms) {
        throw "Event is missing timestamp_ms: $($event.kind)"
    }
    if ($null -ne $event.snapshot) {
        foreach ($property in @("timestamp_ms", "display_id", "gamma", "processes", "errors")) {
            if ($null -eq $event.snapshot.PSObject.Properties[$property]) {
                throw "Event snapshot is missing field: $property"
            }
        }
    }
    if (-not $counts.ContainsKey($event.kind)) {
        $counts[$event.kind] = 0
    }
    $counts[$event.kind]++
}

Write-Host "Report: $Path"
Write-Host "Event count: $($events.Count)"
Write-Host "Probe active: $($report.probe_active)"
foreach ($kind in ($counts.Keys | Sort-Object)) {
    Write-Host ("{0}: {1}" -f $kind, $counts[$kind])
}

$requiredBaseline = @("MONITOR_STARTED", "SNAPSHOT_INITIAL")
foreach ($kind in $requiredBaseline) {
    if (-not $counts.ContainsKey($kind)) {
        throw "Missing baseline event: $kind"
    }
}

Write-Host "Report structure validation passed."
