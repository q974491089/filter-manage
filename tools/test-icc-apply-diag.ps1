param([switch]$Smoke)
# 契约测试：icc-apply-diag.bat 必须 (a) 全程只读，(b) 正确预测 app 会往 LUT 写什么。
# (b) 的基准是 src-tauri/src/icc.rs 的 parse_vcgt —— 对不上就会把用户和排查者
# 一起引向错误方向（vcgt formula 的交错布局就是踩过的坑）。
$ErrorActionPreference = 'Stop'
$bat = Join-Path $PSScriptRoot 'icc-apply-diag.bat'
function Assert($condition, [string]$message) {
    if (-not $condition) { throw "FAIL: $message" }
    Write-Host "PASS: $message"
}

Assert (Test-Path -LiteralPath $bat) 'standalone diagnostic exists'
$text = [IO.File]::ReadAllText($bat, [Text.Encoding]::UTF8)

# ── 结构契约 ──
# 按「行」找哨兵：bat 包装那行的 -notlike '#PS_START*' 里也含这个子串
$bodyLines = $text -split "`r`n"
$sentinel = 0
while ($sentinel -lt $bodyLines.Count -and -not $bodyLines[$sentinel].StartsWith('#PS_START')) { $sentinel++ }
Assert ($sentinel -lt $bodyLines.Count) 'PS sentinel line present'
Assert ($text.Contains("`r`n") -and -not $text.Replace("`r`n", '').Contains("`n")) 'BAT uses CRLF throughout'
$batHeader = $bodyLines[0..($sentinel - 1)]
Assert (($batHeader | Where-Object { $_ -match '[^\x00-\x7F]' }).Count -eq 0) 'BAT header is ASCII-only (cmd parses it with the OEM codepage)'

$match = [regex]::Match($text, "(?s)Add-Type -TypeDefinition @'\r?\n(.*?)\r?\n'@")
Assert $match.Success 'embedded C# can be extracted independently'
$native = $match.Groups[1].Value

# ── 只读契约（这个脚本的核心承诺）──
Assert ($native -notmatch '\b(SetDeviceGammaRamp|SetRamp|SetICMProfile|InstallColorProfile|WcsSet|WcsAssociate|DisplayConfigSetDeviceInfo|SetDisplayConfig)\w*\s*\(') 'native code declares no setter of any kind'
Assert ($native -match '\bGetDeviceGammaRamp\b') 'gamma ramp is only read back'
Assert ($native -match '\bGetICMProfileW\b') 'ICC association is only read back'
Assert ($native -match 'DisplayConfigGetDeviceInfo') 'topology is only read back'
Assert ($native -notmatch '\b(Registry|Microsoft\.Win32\.Registry)\b') 'native code never touches the registry'
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

# ── 无 vcgt：profile 完好，但 app 不碰 LUT ──
$p = [IccDiag]::ParseVcgt((Profile @()))
Assert ($p.Valid -and $p.Kind -eq 'absent' -and $null -eq $p.Ramp) 'profile without vcgt parses as absent-and-valid'
Assert ([IccDiag]::AppVerdict((Profile @()), $p) -match '注册') 'absent vcgt verdict says WCS-registration only (no LUT write)'

# ── 表类型：app 用最近邻 i*(n-1)/255，不是线性插值 ──
$t = Table16 3 2 { param($c, $j) if ($j -eq 0) { 0 } else { 65535 } }
$p = [IccDiag]::ParseVcgt((Profile $t))
Assert ($p.Valid -and $p.Kind -eq 'table' -and $p.Ramp.Length -eq 768) '2-entry 16-bit table parses'
Assert ($p.Ramp[254] -eq 0 -and $p.Ramp[255] -eq 65535) 'table resampling is nearest-neighbour (i*(n-1)/255), not interpolation'
Assert ([IccDiag]::AppVerdict((Profile $t), $p) -match '写入') 'valid RGB table verdict says LUT will be written'

# ── 256 项线性表 = 恒等：app「成功写入」但画面不会有任何变化 ──
$t = Table16 3 256 { param($c, $j) [Math]::Round($j * 65535 / 255) }
$p = [IccDiag]::ParseVcgt((Profile $t))
Assert ($p.Ramp[128] -eq 32896) '256-entry linear table maps identity (i*257)'
Assert ([IccDiag]::IdentityVerdict($p.Ramp) -match '恒等') 'identity curve is reported as identity'

$t = Table16 3 256 { param($c, $j) [Math]::Round(65535 * [Math]::Pow($j / 255.0, 2.2)) }
$p = [IccDiag]::ParseVcgt((Profile $t))
Assert ([IccDiag]::IdentityVerdict($p.Ramp) -notmatch '恒等') 'gamma 2.2 table is not reported as identity'
Assert ([IccDiag]::IdentityVerdict($p.Ramp) -match '\d') 'non-identity verdict quantifies the deviation'

# ── 8 位表项：app 做 v<<8（→65280），不是 v*257 ──
$t = Table8 3 256 { param($c, $j) $j }
$p = [IccDiag]::ParseVcgt((Profile $t))
Assert ($p.Ramp[255] -eq 65280) '8-bit table entries are scaled as v<<8, mirroring the app'

# ── 单通道：ICC 规范允许，但 app 要求 channels>=3 → 直接放弃 ──
$t = Table8 1 256 { param($c, $j) $j }
$p = [IccDiag]::ParseVcgt((Profile $t))
Assert ($p.Valid -and $p.Ramp[0] -eq $p.Ramp[256]) 'single-channel vcgt still parses spec-wise (grayscale replicates)'
Assert ([IccDiag]::AppVerdict((Profile $t), $p) -match '放弃') 'single-channel vcgt verdict says the app gives up (channels < 3)'

$t = Table16 2 4 { param($c, $j) 100 }
$p = [IccDiag]::ParseVcgt((Profile $t))
Assert (-not [IccDiag]::AppVerdict((Profile $t), $p).StartsWith('会写入')) 'non-RGB channel count is not reported as a writable curve'

# ── formula：交错布局 gamma/min/max ──
$t = Formula @(1, 1, 1) @(0, 0, 0) @(1, 1, 1)
$p = [IccDiag]::ParseVcgt((Profile $t))
Assert ($p.Valid -and $p.Kind -eq 'formula' -and $p.Ramp[128] -eq 32896) 'formula uses interleaved gamma/min/max fixed16 values'
Assert ($p.Gamma[0] -eq 1 -and $p.Min[0] -eq 0 -and $p.Max[0] -eq 1) 'formula channel 0 parameters read from the interleaved layout'
Assert ([IccDiag]::IdentityVerdict($p.Ramp) -match '恒等') 'formula gamma=1 min=0 max=1 is an identity curve'

$t = Formula @(1.8, 2.0, 2.4) @(0.02, 0.01, 0.0) @(0.98, 1.0, 1.0)
$p = [IccDiag]::ParseVcgt((Profile $t))
# s15Fixed16 存不下 2.4 这类值（157286/65536 = 2.39999…），只能按容差比
Assert ([Math]::Abs($p.Gamma[2] - 2.4) -lt 0.0001) 'channel 2 gamma reads from its own interleaved slot'
Assert ([Math]::Abs($p.Min[1] - 0.01) -lt 0.0001) 'channel 1 min reads from its own interleaved slot'
Assert ([Math]::Abs($p.Max[0] - 0.98) -lt 0.0001) 'channel 0 max reads from its own interleaved slot'
Assert ([IccDiag]::IdentityVerdict($p.Ramp) -notmatch '恒等') 'calibration-style formula is not identity'

# ── formula 非法参数：gamma<=0 是规范层面的错；max<=min 规范允许但 app 会放弃 ──
$t = Formula @(0, 1, 1) @(0, 0, 0) @(1, 1, 1)
$p = [IccDiag]::ParseVcgt((Profile $t))
Assert (-not $p.Valid -and $p.Error -match 'formula') 'zero formula gamma is rejected'
Assert ([IccDiag]::AppVerdict((Profile $t), $p) -match '放弃') 'zero formula gamma verdict says the app gives up'

$t = Formula @(1, 1, 1) @(1, 1, 1) @(0, 0, 0)
$p = [IccDiag]::ParseVcgt((Profile $t))
Assert ($p.Valid) 'inverted curve (min > max) is legal per spec and still parses'
Assert ([IccDiag]::AppVerdict((Profile $t), $p) -match '放弃') 'inverted curve verdict says the app gives up (max <= min)'

# ── 边界：声明长度、越界、不溢出 ──
$t = Table16 3 4 { param($c, $j) 100 }
$p = [IccDiag]::ParseVcgt((Profile $t 18))
Assert (-not $p.Valid -and $p.Error -match 'table') 'vcgt table cannot read beyond its declared tag size'
Assert ([IccDiag]::AppVerdict((Profile $t 18), $p) -match '不校验') 'app leniency about tag size is called out, not blamed on the app'

$t = Table16 3 4 { param($c, $j) 100 }
BE16 $t 14 256                       # 谎报 256 项，文件里只有 4 项的数据
$d = Profile $t 0                    # 声明长度 0：app 不看这个字段，按文件剩余内容解析
$p = [IccDiag]::ParseVcgt($d)
Assert (-not $p.Valid -and $p.Error -match 'table') 'entry count larger than the actual data is rejected'
Assert ([IccDiag]::AppVerdict($d, $p) -match '放弃') 'app also refuses a table that outruns the file'

$d = Profile (Table16 3 4 { param($c, $j) 100 }); BE32 $d 136 ([uint32]::MaxValue)
Assert (-not [IccDiag]::ParseVcgt($d).Valid) 'huge tag offset rejected without overflow'
$d = Profile @(); BE32 $d 128 ([uint32]::MaxValue)
Assert (-not [IccDiag]::ParseVcgt($d).Valid) 'huge tag count rejected without overflow'
$d = Profile (Table16 3 4 { param($c, $j) 100 }); BE32 $d 0 ($d.Length + 16)
Assert (-not [IccDiag]::ParseVcgt($d).Valid) 'declared file length beyond EOF rejected'
$d = Profile @(); $d[36] = 0
Assert (-not [IccDiag]::ParseVcgt($d).Valid) 'bad acsp signature rejected'
Assert (-not [IccDiag]::ParseVcgt((New-Object byte[] 12)).Valid) 'truncated header rejected'
Assert (-not [IccDiag]::ParseVcgt($null).Valid) 'null input rejected rather than crashing'

# ── ramp 比对：只看高字节 ──
$a = [IccDiag]::ParseVcgt((Profile (Table16 3 256 { param($c, $j) [Math]::Round($j * 65535 / 255) }))).Ramp
$b = $a.Clone(); $b[128] = $b[128] -band 65280
Assert ([IccDiag]::MaxHighByteDiff($a, $b) -eq 0) 'low-byte quantization is ignored for comparison'
$b[128] = 0
Assert ([IccDiag]::MaxHighByteDiff($a, $b) -eq 128) 'high-byte curve change is measured'
Assert ([IccDiag]::MaxHighByteDiff($null, $b) -eq -1) 'missing ramp is unknown rather than unchanged'

Assert ([IccDiag]::LayoutsValid()) 'Win32 structure sizes match the SDK ABI'

$body = ($bodyLines[($sentinel + 1)..($bodyLines.Count - 1)]) -join "`r`n"
$tokens = $null; $parseErrors = $null
$tree = [Management.Automation.Language.Parser]::ParseInput($body, [ref]$tokens, [ref]$parseErrors)
Assert ($parseErrors.Count -eq 0) 'PowerShell body parses'

# 只加载函数定义、不跑主流程 —— 这样采集/分类逻辑能被功能测试覆盖，
# 而不是只能做「文本里有没有这句话」这种假断言
foreach ($fn in $tree.FindAll({ param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] }, $true)) {
    . ([scriptblock]::Create($fn.Extent.Text))
}

# 幽灵设备（未连接）绝不能被报成故障：每台机器都有一堆「以前插过、现在不在」的旧记录
# （本机实测 1 个旧显卡 + 6 个旧显示器）。报成故障 = 满屏假警报 + 把人引向错误方向。
Assert ((Get-ErrCodeText 45) -match '无害') 'code 45 (not connected / ghost device) is labelled benign'
Assert ((Get-ErrCodeText 24) -match '无害') 'code 24 (not present / ghost device) is labelled benign'
Assert ((Get-ErrCodeText 28) -match '驱动未安装') 'code 28 (no driver installed) is still reported as a real fault'
Assert ((Get-ErrCodeText 31) -match '无法加载') 'code 31 (cannot load drivers) is still reported as a real fault'
Assert ((Get-ErrCodeText 45) -notmatch '\*\*') 'benign codes carry no alarm marker'
# 第一份真实报告就栽在这：用户双击运行、没拖文件、Q1 留空，结果「点的是哪个 ICC」无从得知。
# 报告缺了这一项基本没用，所以脚本必须反复要，不能静默放过。
Assert ($body -match 'Test-Path -LiteralPath \$iccPath') 'ICC path is validated instead of silently skipped'
# 两次快照分不清「没写」和「写了又被抢回去」——必须覆盖点击那一整段时间
Assert ($body -match 'Burst\(') 'the click window is sampled continuously, not just at two points'
# 驱动健康必须是机器可读地采出来，而不是让用户去设备管理器截图
# （DisplayCAL 有先例：SetDeviceGammaRamp 全盘失败，根因是显示适配器驱动没装好）
Assert ($body -match 'function Get-MachineProfile') 'chassis / driver-health / LUT-contender probe is defined'
Assert ($body -match 'Add-Text \(Get-MachineProfile\)') 'and it is actually called in the report flow'
Assert ($body -match 'ConfigManagerErrorCode|Get-PnpDevice -Class Display') 'display-adapter health is read machine-readably'
Assert ($body -match 'Get-Service') 'service-based LUT contenders are scanned (a tray exit does not stop a service)'
# 对"必然常驻"的组件（NVIDIA 自己的容器服务）也喊"请关掉"，等于每份报告都带这句 = 没有信号
Assert ($body -match 'loud\s*=\s*\$false') 'contenders are tiered so always-present driver components cannot raise the alarm'

if ($Smoke) {
    Assert ([IccDiag]::AppMonitors().Length -gt 0) 'live app-enumeration replica returns a result'
    Assert ([IccDiag]::Topology().Length -gt 0) 'live topology probe returns a result'
    Assert ([IccDiag]::Snapshot().Length -gt 0) 'live read-only snapshot returns a result'
    Assert ([IccDiag]::Luttakers().Length -gt 0) 'live LUT-contender scan returns a result'
    $real = Join-Path $env:SystemRoot 'System32\spool\drivers\color'
    $one = Get-ChildItem $real -Filter *.icc -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($one) { Write-Host ([IccDiag]::AnalyzeIcc($one.FullName)) }
    Assert ([IccDiag]::IccAlreadyActive().Length -gt 0) 'live already-active check returns a result'
    Assert ([IccDiag]::Burst(1).Length -gt 0) 'live burst sampling returns a result'
    $mp = Get-MachineProfile
    Assert ($mp -match '显示适配器') 'live chassis / driver-health profile is collected'
    Assert ($mp -match '历史遗留|驱动状态均正常') 'ghost device records are summarised, not alarmed as faults'
    Assert ($mp -notmatch '(?s)NVIDIA 驱动组件.*请全部退出') 'always-present driver components are not escalated to "close these and re-run"'
    Write-Host $mp
    Write-Host ([IccDiag]::AppMonitors())
    Write-Host ([IccDiag]::Topology())
    Write-Host ([IccDiag]::Snapshot())
    Write-Host ([IccDiag]::IccAlreadyActive())
}
Write-Host 'All ICC apply-diagnostic tests passed.'
