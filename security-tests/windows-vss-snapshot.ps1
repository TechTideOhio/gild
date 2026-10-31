# Synthetic disposable-runner experiment; not a production snapshot service.
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or !$env:RUNNER_TEMP) {
    throw 'Run only on an explicitly disposable GitHub-hosted runner.'
}

$scratch = Join-Path $env:RUNNER_TEMP ('gild-vss-race-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $scratch | Out-Null
$vhd = Join-Path $scratch 'test.vhdx'
$userName = 'gild' + [guid]::NewGuid().ToString('N').Substring(0, 12)
$user = $null
$writer = $null
$shadowId = $null
$passMessage = $null
$cleanupProblems = [Collections.Generic.List[string]]::new()

try {
    if (Get-PSDrive -Name Z -ErrorAction SilentlyContinue) { throw 'Test drive letter Z is already in use.' }
    $diskpart = Join-Path $scratch 'create.txt'
    @("create vdisk file=`"$vhd`" maximum=512 type=expandable", "select vdisk file=`"$vhd`"", 'attach vdisk', 'create partition primary', 'format fs=ntfs quick label=GILDTEST', 'assign letter=Z') | Set-Content -Path $diskpart -Encoding ascii
    & diskpart.exe /s $diskpart
    if ($LASTEXITCODE -ne 0 -or !(Test-Path 'Z:\')) { throw 'Could not create the isolated test VHD.' }
    $imageDisk = Get-DiskImage -ImagePath $vhd | Get-Disk
    $driveDisk = Get-Partition -DriveLetter Z | Get-Disk
    if (!$imageDisk -or !$driveDisk -or $imageDisk.Number -ne $driveDisk.Number -or $imageDisk.UniqueId -ne $driveDisk.UniqueId) {
        throw 'Assigned drive does not belong to the newly created VHD.'
    }

    $password = ConvertTo-SecureString -String ('Aa1!' + [guid]::NewGuid().ToString('N')) -AsPlainText -Force
    $user = New-LocalUser -Name $userName -Password $password -PasswordNeverExpires
    $source = 'Z:\source'
    New-Item -ItemType Directory -Path $source | Out-Null
    & icacls.exe $source /grant "${userName}:(OI)(CI)M"
    if ($LASTEXITCODE -ne 0) { throw 'Could not grant writer access to synthetic source.' }
    $writerScript = Join-Path $source 'writer.ps1'
    @'
param([string] $Directory)
$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { exit 10 }
[IO.File]::WriteAllText((Join-Path $Directory 'identity.txt'), $identity.User.Value)
[IO.File]::WriteAllText((Join-Path $Directory 'value.txt'), 'initial')
[IO.File]::WriteAllText((Join-Path $Directory 'ready.txt'), 'ready')
$n = 0
while (!(Test-Path (Join-Path $Directory 'stop.txt'))) {
    [IO.File]::WriteAllText((Join-Path $Directory 'next.txt'), "value-$n")
    Move-Item -LiteralPath (Join-Path $Directory 'next.txt') -Destination (Join-Path $Directory 'value.txt') -Force
    $n++
    [IO.File]::WriteAllText((Join-Path $Directory 'count.next'), [string]$n)
    Move-Item -LiteralPath (Join-Path $Directory 'count.next') -Destination (Join-Path $Directory 'count.txt') -Force
}
'@ | Set-Content -Path $writerScript -Encoding utf8
    $credential = [Management.Automation.PSCredential]::new(".\$userName", $password)
    $writer = Start-Process -FilePath 'pwsh.exe' -ArgumentList @('-NoProfile', '-NonInteractive', '-File', $writerScript, $source) -Credential $credential -PassThru
    $ready = Join-Path $source 'ready.txt'
    $countPath = Join-Path $source 'count.txt'
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while ((!(Test-Path $ready) -or !(Test-Path $countPath)) -and [DateTime]::UtcNow -lt $deadline) {
        $writer.Refresh()
        if ($writer.HasExited) { throw "Writer exited before readiness: $($writer.ExitCode)" }
        Start-Sleep -Milliseconds 100
    }
    if (!(Test-Path $ready) -or !(Test-Path $countPath)) { throw 'Writer readiness timeout.' }
    if ((Get-Content -LiteralPath (Join-Path $source 'identity.txt') -Raw).Trim() -ne $user.SID.Value) { throw 'Writer identity was not the restricted test user.' }
    $countBefore = 0
    for ($i = 0; $i -lt 100; $i++) {
        if ([int]::TryParse((Get-Content -LiteralPath $countPath -Raw -ErrorAction SilentlyContinue), [ref]$countBefore)) { break }
        Start-Sleep -Milliseconds 20
    }
    if ($countBefore -lt 1) { throw 'Could not read a positive writer counter.' }

    $created = Invoke-CimMethod -ClassName Win32_ShadowCopy -MethodName Create -Arguments @{ Volume = 'Z:\'; Context = 'ClientAccessible' }
    if ($created.ReturnValue -ne 0 -or !$created.ShadowID) { throw "VSS creation failed: $($created.ReturnValue)" }
    $shadowId = $created.ShadowID
    $shadow = Get-CimInstance -ClassName Win32_ShadowCopy | Where-Object ID -eq $shadowId
    if (!$shadow -or !$shadow.DeviceObject) { throw 'Created shadow copy was not found.' }
    $snapshotFile = $shadow.DeviceObject + '\source\value.txt'
    $firstHash = (Get-FileHash -LiteralPath $snapshotFile -Algorithm SHA256).Hash
    $countAtCapture = 0
    for ($i = 0; $i -lt 100; $i++) {
        if ([int]::TryParse((Get-Content -LiteralPath $countPath -Raw -ErrorAction SilentlyContinue), [ref]$countAtCapture)) { break }
        Start-Sleep -Milliseconds 20
    }
    if ($countAtCapture -lt 1) { throw 'Could not read a positive post-capture writer counter.' }
    $countAfter = 0
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while ([DateTime]::UtcNow -lt $deadline) {
        $candidate = 0
        if ([int]::TryParse((Get-Content -LiteralPath $countPath -Raw -ErrorAction SilentlyContinue), [ref]$candidate) -and $candidate -gt $countAtCapture) { $countAfter = $candidate; break }
        Start-Sleep -Milliseconds 100
    }
    if ($countAfter -le $countAtCapture) { throw 'Writer did not progress after snapshot.' }
    if ((Get-FileHash -LiteralPath $snapshotFile -Algorithm SHA256).Hash -ne $firstHash) { throw 'Snapshot bytes changed after capture.' }
    [IO.File]::WriteAllText((Join-Path $source 'stop.txt'), 'stop')
    if (!$writer.WaitForExit(30000)) { throw 'Writer did not stop.' }
    if ($writer.ExitCode -ne 0) { throw "Writer failed: $($writer.ExitCode)" }
    $writer = $null

    $shadow | Remove-CimInstance
    if (Get-CimInstance -ClassName Win32_ShadowCopy | Where-Object ID -eq $shadowId) { throw 'Shadow copy still exists after deletion.' }
    $shadowId = $null
    if (Test-Path -LiteralPath $snapshotFile) { throw 'Deleted snapshot pathname is still readable.' }
    $passMessage = "PASS: snapshot bytes stayed stable while a non-admin writer progressed after capture ($countBefore -> $countAtCapture -> $countAfter)"
} finally {
    if ($writer) {
        try {
            $writer.Refresh()
            if (!$writer.HasExited) { Stop-Process -Id $writer.Id -Force -ErrorAction Stop }
            $writer.WaitForExit()
        } catch { $cleanupProblems.Add("writer: $($_.Exception.Message)") }
    }
    if ($shadowId) {
        try {
            $shadow = Get-CimInstance -ClassName Win32_ShadowCopy | Where-Object ID -eq $shadowId
            if ($shadow) { $shadow | Remove-CimInstance }
            if (Get-CimInstance -ClassName Win32_ShadowCopy | Where-Object ID -eq $shadowId) { throw 'shadow still exists' }
        } catch { $cleanupProblems.Add("shadow: $($_.Exception.Message)") }
    }
    if ($user) {
        try {
            Remove-LocalUser -SID $user.SID
            if (Get-LocalUser -SID $user.SID -ErrorAction SilentlyContinue) { throw 'test user still exists' }
        } catch { $cleanupProblems.Add("user: $($_.Exception.Message)") }
    }
    if (Test-Path $vhd) {
        try {
            $diskImage = Get-DiskImage -ImagePath $vhd
            if ($diskImage.Attached) {
                $detach = Join-Path $scratch 'detach.txt'
                @("select vdisk file=`"$vhd`"", 'detach vdisk') | Set-Content -Path $detach -Encoding ascii
                & diskpart.exe /s $detach
                if ($LASTEXITCODE -ne 0 -or (Get-DiskImage -ImagePath $vhd).Attached) { throw 'test disk remains attached' }
            }
        } catch { $cleanupProblems.Add("disk: $($_.Exception.Message)") }
    }
    try { Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction Stop } catch { $cleanupProblems.Add("scratch: $($_.Exception.Message)") }
    if ($cleanupProblems.Count) { throw "Cleanup failed: $($cleanupProblems -join '; ')" }
}
if ($passMessage) {
    Write-Output $passMessage
    Write-Output 'LIMIT: not proof of write overlap inside VSS, transaction consistency, a restricted reader, reparse-safe opens, or reval failure handling.'
}
