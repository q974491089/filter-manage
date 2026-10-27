param([switch]$Smoke)
# 契约测试：gamma-ramp-diag.bat 的 vcgt 解析必须"预测 app 会写什么"。
# 基准是 src-tauri/src/icc.rs 的 parse_vcgt —— 诊断脚本若与它不一致，
# 报告就会把用户和排查者一起引到错误方向（这正是旧版本 formula 分支的毛病）。
$ErrorActionPreference = 'Stop'
$bat = Join-Path $PSScriptRoot 'gamma-ramp-diag.bat'
function Assert($condition, [string]$message) {
    if (-not $condition) { throw "FAIL: $message" }
    Write-Host "PASS: $message"
}

Assert (Test-Path -LiteralPath $bat) 'diagnostic script exists'
$text = [IO.File]::ReadAllText($bat, [Text.Encoding]::UTF8)

# ── 结构契约 ──
# 按「行」找哨兵：bat 包装那行的 -notlike '#PS_START*' 里也含这个子串，
# 用子串 IndexOf 会把 bat 头部当成正文（包装脚本自己就是按行切的）
$bodyLines = $text -split "`r`n"
$sentinel = 0
while ($sentinel -lt $bodyLines.Count -and -not $bodyLines[$sentinel].StartsWith('#PS_START')) { $sentinel++ }
Assert ($sentinel -lt $bodyLines.Count) 'PS sentinel line present'
Assert ($text.Contains("`r`n") -and -not $text.Replace("`r`n", '').Contains("`n")) 'BAT uses CRLF throughout'

$match = [regex]::Match($text, "(?s)Add-Type -TypeDefinition @'\r?\n(.*?)\r?\n'@")
Assert $match.Success 'embedded C# can be extracted independently'
$native = $match.Groups[1].Value

# ── 新增代码不得引入任何"写"能力 ──
# 本脚本本身会写 gamma（第 5 节写入测试，有告警且自动恢复），那是既有的、用户知情的功能。
# 这里守的是：ICC / WCS / 显示拓扑这三类写入一个都不许有，拓扑只读不写。
Assert ($native -notmatch '\b(SetICMProfile|InstallColorProfile|WcsSet|WcsAssociate|DisplayConfigSetDeviceInfo|SetDisplayConfig)\w*\s*\(') 'native code has no profile/WCS/topology setters'
Assert ($native -match 'DisplayConfigGetDeviceInfo') 'topology probe reads via DisplayConfigGetDeviceInfo'
Assert ($text -notmatch '\b(Invoke-WebRequest|Invoke-RestMethod|Set-ItemProperty|New-ItemProperty|Start-Process)\b') 'no uploads, registry writes or process launching'

Add-Type -TypeDefinition $native

# ── ICC 构造辅助 ──
function BE32([byte[]]$data, [int]$offset, [uint32]$value) {
    $data[$offset] = ($value -shr 24) -band 255
    $data[$offset + 1] = ($value -shr 16) -band 255
    $data[$offset + 2] = ($value -shr 8) -band 255
    $data[$offset + 3] = $value -band 255
}
function BE16([byte[]]$data, [int]$offset, [int]$value) {
    $data[$offset] = ($value -shr 8) -band 255
    $data[$offset + 1] = $value -band 255
}
function Profile([byte[]]$tag, [int]$declaredSize = -1) {
    $size = if ($tag.Length) { 144 + $tag.Length } else { 132 }
    $d = New-Object byte[] $size
    BE32 $d 0 $size
    [Text.Encoding]::ASCII.GetBytes('mntr').CopyTo($d, 12)
    [Text.Encoding]::ASCII.GetBytes('RGB ').CopyTo($d, 16)
    [Text.Encoding]::ASCII.GetBytes('acsp').CopyTo($d, 36)
    if ($tag.Length) {
        BE32 $d 128 1
        [Text.Encoding]::ASCII.GetBytes('vcgt').CopyTo($d, 132)
        BE32 $d 136 144
        BE32 $d 140 $(if ($declaredSize -ge 0) { $declaredSize } else { $tag.Length })
        $tag.CopyTo($d, 144)
    }
    return , $d
}
# 16 位表：第 c 通道第 j 项由 $value(c,j) 给出
function Table16([int]$channels, [int]$entries, [scriptblock]$value) {
    $t = New-Object byte[] (18 + $channels * $entries * 2)
    [Text.Encoding]::ASCII.GetBytes('vcgt').CopyTo($t, 0)
    BE16 $t 12 $channels; BE16 $t 14 $entries; BE16 $t 16 2
    for ($c = 0; $c -lt $channels; $c++) {
        for ($j = 0; $j -lt $entries; $j++) {
            BE16 $t (18 + $c * $entries * 2 + $j * 2) ([int](& $value $c $j))
        }
    }
    return , $t
}
function Table8([int]$channels, [int]$entries, [scriptblock]$value) {
    $t = New-Object byte[] (18 + $channels * $entries)
    [Text.Encoding]::ASCII.GetBytes('vcgt').CopyTo($t, 0)
    BE16 $t 12 $channels; BE16 $t 14 $entries; BE16 $t 16 1
    for ($c = 0; $c -lt $channels; $c++) {
        for ($j = 0; $j -lt $entries; $j++) {
            $t[18 + $c * $entries + $j] = [byte](& $value $c $j)
        }
    }
    return , $t
}
# formula：交错布局 gamma/min/max（每通道 3 个 s15Fixed16）
function Formula([double[]]$g, [double[]]$mn, [double[]]$mx) {
    $t = New-Object byte[] 48
    [Text.Encoding]::ASCII.GetBytes('vcgt').CopyTo($t, 0)
    BE32 $t 8 1
    for ($c = 0; $c -lt 3; $c++) {
        BE32 $t (12 + $c * 12) ([uint32][int][Math]::Round($g[$c] * 65536))
        BE32 $t (16 + $c * 12) ([uint32][int][Math]::Round($mn[$c] * 65536))
        BE32 $t (20 + $c * 12) ([uint32][int][Math]::Round($mx[$c] * 65536))
    }
    return , $t
}

# ── 无 vcgt：合法，但 app 不会碰 LUT ──
$p = [GammaDiag]::ParseVcgt((Profile @()))
Assert ($p.Valid -and $p.Kind -eq 'absent' -and $null -eq $p.Ramp) 'profile without vcgt parses as absent'
Assert ([GammaDiag]::AppVerdict((Profile @()), $p) -match '注册') 'absent vcgt verdict says WCS-registration only (no LUT write)'

# ── 表类型：app 用最近邻 i*(n-1)/255，不是线性插值 ──
$t = Table16 3 2 { param($c, $j) if ($j -eq 0) { 0 } else { 65535 } }
$p = [GammaDiag]::ParseVcgt((Profile $t))
Assert ($p.Valid -and $p.Kind -eq 'table' -and $p.Ramp.Length -eq 768) '2-entry 16-bit table parses'
Assert ($p.Ramp[254] -eq 0 -and $p.Ramp[255] -eq 65535) 'table resampling is nearest-neighbour (i*(n-1)/255), not interpolation'
Assert ([GammaDiag]::AppVerdict((Profile $t), $p) -match '写入') 'valid RGB table verdict says LUT will be written'

# ── 256 项线性表 = 恒等曲线：app 会"成功写入"但画面不会有任何变化 ──
$t = Table16 3 256 { param($c, $j) [Math]::Round($j * 65535 / 255) }
$p = [GammaDiag]::ParseVcgt((Profile $t))
Assert ($p.Ramp[128] -eq 32896) '256-entry linear table maps identity (i*257)'
Assert ([GammaDiag]::IdentityVerdict($p.Ramp) -match '恒等') 'identity curve is reported as identity'

$t = Table16 3 256 { param($c, $j) [Math]::Round(65535 * [Math]::Pow($j / 255.0, 2.2)) }
$p = [GammaDiag]::ParseVcgt((Profile $t))
Assert ([GammaDiag]::IdentityVerdict($p.Ramp) -notmatch '恒等') 'gamma 2.2 table is not reported as identity'
Assert ([GammaDiag]::IdentityVerdict($p.Ramp) -match '\d') 'non-identity verdict quantifies the deviation'

# ── 8 位表项：app 做 v<<8（→65280），不是 v*257 ──
$t = Table8 3 256 { param($c, $j) $j }
$p = [GammaDiag]::ParseVcgt((Profile $t))
Assert ($p.Ramp[255] -eq 65280) '8-bit table entries are scaled as v<<8, mirroring the app'

# ── 单通道：ICC 规范允许，但 app 要求 channels>=3 → 直接放弃 ──
$t = Table8 1 256 { param($c, $j) $j }
$p = [GammaDiag]::ParseVcgt((Profile $t))
Assert ($p.Valid -and $p.Ramp[0] -eq ($p.Ramp[256])) 'single-channel vcgt still parses spec-wise (grayscale replicates)'
Assert ([GammaDiag]::AppVerdict((Profile $t), $p) -match '放弃') 'single-channel vcgt verdict says the app gives up (channels < 3)'

# ── 表头非 3 通道之外的非法值仍要拒绝 ──
$t = Table16 2 4 { param($c, $j) 100 }
$p = [GammaDiag]::ParseVcgt((Profile $t))
Assert (-not [GammaDiag]::AppVerdict((Profile $t), $p).StartsWith('会写入')) 'non-RGB channel count is not reported as a writable curve'

# ── formula：交错布局 gamma/min/max ──
$t = Formula @(1, 1, 1) @(0, 0, 0) @(1, 1, 1)
$p = [GammaDiag]::ParseVcgt((Profile $t))
Assert ($p.Valid -and $p.Kind -eq 'formula' -and $p.Ramp[128] -eq 32896) 'formula uses interleaved gamma/min/max fixed16 values'
Assert ($p.Gamma[0] -eq 1 -and $p.Min[0] -eq 0 -and $p.Max[0] -eq 1) 'formula channel 0 parameters read from the interleaved layout'
Assert ([GammaDiag]::IdentityVerdict($p.Ramp) -match '恒等') 'formula gamma=1 min=0 max=1 is an identity curve'

$t = Formula @(1.8, 2.0, 2.4) @(0.02, 0.01, 0.0) @(0.98, 1.0, 1.0)
$p = [GammaDiag]::ParseVcgt((Profile $t))
# s15Fixed16 存不下 2.4 这类值（157286/65536 = 2.39999…），只能按容差比
Assert ([Math]::Abs($p.Gamma[2] - 2.4) -lt 0.0001) 'channel 2 gamma reads from its own interleaved slot'
Assert ([Math]::Abs($p.Min[1] - 0.01) -lt 0.0001) 'channel 1 min reads from its own interleaved slot'
Assert ([Math]::Abs($p.Max[0] - 0.98) -lt 0.0001) 'channel 0 max reads from its own interleaved slot'
Assert ([GammaDiag]::IdentityVerdict($p.Ramp) -notmatch '恒等') 'calibration-style formula is not identity'

# ── formula 的非法参数：gamma<=0 是规范层面的错误，max<=min 是规范允许但 app 会放弃 ──
$t = Formula @(0, 1, 1) @(0, 0, 0) @(1, 1, 1)
$p = [GammaDiag]::ParseVcgt((Profile $t))
Assert (-not $p.Valid -and $p.Error -match 'formula') 'zero formula gamma is rejected'
Assert ([GammaDiag]::AppVerdict((Profile $t), $p) -match '放弃') 'zero formula gamma verdict says the app gives up'

$t = Formula @(1, 1, 1) @(1, 1, 1) @(0, 0, 0)
$p = [GammaDiag]::ParseVcgt((Profile $t))
Assert ($p.Valid) 'inverted curve (min > max) is legal per spec and still parses'
Assert ([GammaDiag]::AppVerdict((Profile $t), $p) -match '放弃') 'inverted curve verdict says the app gives up (max <= min)'

# ── 边界：声明长度、越界、溢出 ──
$t = Table16 3 4 { param($c, $j) 100 }
$p = [GammaDiag]::ParseVcgt((Profile $t 18))
Assert (-not $p.Valid -and $p.Error -match 'table') 'vcgt table cannot read beyond its declared tag size'
Assert ([GammaDiag]::AppVerdict((Profile $t 18), $p) -match '不校验') 'app leniency about tag size is called out, not blamed on the app'

$d = Profile (Table16 3 4 { param($c, $j) 100 }); BE32 $d 136 ([uint32]::MaxValue)
Assert (-not [GammaDiag]::ParseVcgt($d).Valid) 'huge tag offset rejected without overflow'
$d = Profile @(); BE32 $d 128 ([uint32]::MaxValue)
Assert (-not [GammaDiag]::ParseVcgt($d).Valid) 'huge tag count rejected without overflow'
$d = Profile (Table16 3 4 { param($c, $j) 100 }); BE32 $d 0 ($d.Length + 16)
Assert (-not [GammaDiag]::ParseVcgt($d).Valid) 'declared file length beyond EOF rejected'
$d = Profile @(); $d[36] = 0
Assert (-not [GammaDiag]::ParseVcgt($d).Valid) 'bad acsp signature rejected'
Assert (-not [GammaDiag]::ParseVcgt((New-Object byte[] 12)).Valid) 'truncated header rejected'
$t = Table16 3 4 { param($c, $j) 100 }
BE16 $t 14 256                       # 谎报 256 项，文件里只有 4 项的数据
$d = Profile $t 0                    # 声明长度 0：app 不看这个字段，按文件剩余内容解析
$p = [GammaDiag]::ParseVcgt($d)
Assert (-not $p.Valid -and $p.Error -match 'table') 'entry count larger than the actual data is rejected'
Assert ([GammaDiag]::AppVerdict($d, $p) -match '放弃') 'app also refuses a table that outruns the file'

# ── ramp 比对：只看高字节（驱动会把 16 位 LUT 量化到 8 位）──
$a = [GammaDiag]::ParseVcgt((Profile (Table16 3 256 { param($c, $j) [Math]::Round($j * 65535 / 255) }))).Ramp
$b = $a.Clone(); $b[128] = $b[128] -band 65280
Assert ([GammaDiag]::MaxHighByteDiff($a, $b) -eq 0) 'low-byte quantization is ignored for comparison'
$b[128] = 0
Assert ([GammaDiag]::MaxHighByteDiff($a, $b) -eq 128) 'high-byte curve change is measured'
Assert ([GammaDiag]::MaxHighByteDiff($null, $b) -eq -1) 'missing ramp is unknown rather than unchanged'

Assert ([GammaDiag]::LayoutsValid()) 'Win32 structure sizes match the SDK ABI'

$body = ($bodyLines[($sentinel + 1)..($bodyLines.Count - 1)]) -join "`r`n"
$tokens = $null; $parseErrors = $null
[void][Management.Automation.Language.Parser]::ParseInput($body, [ref]$tokens, [ref]$parseErrors)
Assert ($parseErrors.Count -eq 0) 'PowerShell body parses'

if ($Smoke) {
    Assert ([GammaDiag]::DisplayTopology().Length -gt 0) 'live topology probe returns a result'
    Assert ([GammaDiag]::AppMonitorList().Length -gt 0) 'live app-enumeration replica returns a result'
    Write-Host ([GammaDiag]::DisplayTopology())
    Write-Host ([GammaDiag]::AppMonitorList())
}
Write-Host 'All vcgt diagnostic tests passed.'
