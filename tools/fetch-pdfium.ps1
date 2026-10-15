<#
.SYNOPSIS
  PDF-Over NG - PDFium-Binaries (bblanchon/pdfium-binaries) laden, per SHA-256 pruefen und entpacken.

.DESCRIPTION
  Gegenstueck zu tools/fetch-pdfium.sh fuer Windows (PowerShell 5.1 und 7). Die erwarteten Hashes
  stehen in tools/pdfium.lock; eine Abweichung bricht ab und loescht das Archiv. Ziel ist
  tools/pdfium/<plattform>/ (nicht versioniert). Die Bibliothek liegt unter Windows in bin\pdfium.dll,
  sonst in lib/. Bereits entpackte Plattformen mit passendem Hash werden nicht neu geladen.

  Plattformen: mac-arm64 mac-x64 linux-x64 linux-arm64 win-x64 win-arm64 ios-device-arm64
               ios-simulator-arm64 ios-simulator-x64 android-arm64 android-arm android-x64

  Umgebung (optional): PDFIUM_DEST, PDFIUM_CACHE, PDFIUM_RELEASE (nur mit -UpdateLock), PDFIUM_BASE_URL.

  Die Datei ist bewusst reines ASCII: Windows PowerShell 5.1 liest Skripte ohne BOM als ANSI.

.EXAMPLE
  tools\fetch-pdfium.ps1                         # Plattform dieses Rechners
.EXAMPLE
  tools\fetch-pdfium.ps1 win-x64 win-arm64       # bestimmte Plattformen
.EXAMPLE
  tools\fetch-pdfium.ps1 -All                    # alle Plattformen aus tools/pdfium.lock
.EXAMPLE
  Invoke-Expression (tools\fetch-pdfium.ps1 -PrintEnv)   # $env:PDFOVER_PDFIUM_DIR setzen
.EXAMPLE
  tools\fetch-pdfium.ps1 -GitHubEnv              # CI: PDFOVER_PDFIUM_DIR nach $env:GITHUB_ENV
.EXAMPLE
  $env:PDFIUM_RELEASE = 'chromium/<n>'; tools\fetch-pdfium.ps1 -UpdateLock -All
#>
[CmdletBinding()]
param(
    [Parameter(Position = 0, ValueFromRemainingArguments = $true)]
    [string[]] $Platform,
    [switch] $All,
    [switch] $List,
    [switch] $PrintDir,
    [switch] $PrintEnv,
    [switch] $GitHubEnv,
    [switch] $UpdateLock,
    [switch] $Force,
    [string] $Dest
)

Set-StrictMode -Version 3
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'   # Invoke-WebRequest ist mit Fortschrittsbalken sehr langsam

$AllPlatforms = @(
    'mac-arm64', 'mac-x64', 'linux-x64', 'linux-arm64', 'win-x64', 'win-arm64', 'ios-device-arm64',
    'ios-simulator-arm64', 'ios-simulator-x64', 'android-arm64', 'android-arm', 'android-x64'
)
$Root = Split-Path -Parent $PSScriptRoot
$Lock = Join-Path $Root 'tools/pdfium.lock'

function Fail([string] $Message) {
    Write-Error "fetch-pdfium: $Message" -ErrorAction Continue
    exit 1
}
function Log([string] $Message) { Write-Host "fetch-pdfium: $Message" }

function Get-HostPlatform {
    $onWindows = ($env:OS -eq 'Windows_NT')
    if ($onWindows) {
        $arch = $env:PROCESSOR_ARCHITEW6432
        if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
        switch ($arch) {
            'AMD64' { return 'win-x64' }
            'ARM64' { return 'win-arm64' }
            default { Fail "unbekannte Architektur $arch; Plattform bitte angeben" }
        }
    }
    $osArch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    $arch = switch ($osArch) { 'Arm64' { 'arm64' } 'X64' { 'x64' } default { Fail "unbekannte Architektur $osArch" } }
    if ((Get-Variable -Name IsMacOS -ErrorAction SilentlyContinue) -and $IsMacOS) { return "mac-$arch" }
    if ((Get-Variable -Name IsLinux -ErrorAction SilentlyContinue) -and $IsLinux) { return "linux-$arch" }
    Fail 'unbekanntes Betriebssystem; Plattform bitte angeben'
}

function Get-LibSubdir([string] $Plat) { if ($Plat -like 'win-*') { 'bin' } else { 'lib' } }

function Get-Sha256([string] $Path) { (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant() }

function Get-LockLines {
    if (-not (Test-Path -LiteralPath $Lock)) { Fail "$Lock fehlt" }
    Get-Content -LiteralPath $Lock -Encoding UTF8
}

function Get-LockRelease {
    foreach ($line in Get-LockLines) {
        if ($line -match '^release\s+(\S+)\s*$') { return $Matches[1] }
    }
    return $null
}

function Get-LockSha([string] $Plat) {
    $name = "pdfium-$Plat.tgz"
    foreach ($line in Get-LockLines) {
        if ($line -match '^([0-9a-f]{64})\s+(\S+)\s*$' -and $Matches[2] -eq $name) { return $Matches[1] }
    }
    return $null
}

function Invoke-Download([string] $Url, [string] $OutFile) {
    if ($PSVersionTable.PSVersion.Major -lt 6) {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    }
    for ($i = 1; $i -le 3; $i++) {
        try {
            Invoke-WebRequest -Uri $Url -OutFile $OutFile -UseBasicParsing -MaximumRedirection 10
            return
        } catch {
            if ($i -eq 3) { throw }
            Start-Sleep -Seconds (2 * $i)
        }
    }
}

# Unter Windows das bsdtar des Systems verwenden: GNU tar (Git for Windows) hielte "D:\..." fuer
# einen entfernten Host.
function Get-Tar {
    if ($env:OS -eq 'Windows_NT' -and $env:SystemRoot) {
        $systemTar = Join-Path $env:SystemRoot 'System32\tar.exe'
        if (Test-Path -LiteralPath $systemTar) { return $systemTar }
    }
    return 'tar'
}
$Tar = Get-Tar

function Test-ArchiveMembers([string] $Path) {
    $members = & $Tar -tzf $Path
    if ($LASTEXITCODE -ne 0) { Fail "$Path ist kein lesbares tar.gz-Archiv" }
    foreach ($m in $members) {
        if ($m -match '^(/|\\|[A-Za-z]:)' -or $m -match '(^|[\\/])\.\.([\\/]|$)') { Fail "${Path}: unzulaessiger Pfad im Archiv: $m" }
    }
}

# --- Argumente ---------------------------------------------------------------------------------
if ($List) { $AllPlatforms; exit 0 }
$platforms = @()
if ($All) { $platforms = $AllPlatforms }
foreach ($p in @($Platform | Where-Object { $_ })) {
    if ($AllPlatforms -notcontains $p) { Fail "unbekannte Plattform '$p' (-List zeigt alle)" }
    $platforms += $p
}
if ($platforms.Count -eq 0) { $platforms = @(Get-HostPlatform) }

if (-not $Dest) { $Dest = $env:PDFIUM_DEST }
if (-not $Dest) { $Dest = Join-Path $Root 'tools/pdfium' }
New-Item -ItemType Directory -Force -Path $Dest | Out-Null
$Dest = (Resolve-Path -LiteralPath $Dest).ProviderPath

$release = Get-LockRelease
if ($UpdateLock -and $env:PDFIUM_RELEASE) { $release = $env:PDFIUM_RELEASE }
elseif ($env:PDFIUM_RELEASE -and $env:PDFIUM_RELEASE -ne $release) {
    Fail "PDFIUM_RELEASE=$($env:PDFIUM_RELEASE) weicht von tools/pdfium.lock ($release) ab; nur mit -UpdateLock"
}
if (-not $release -or $release -notmatch '^chromium/[0-9]+$') { Fail "ungueltige oder fehlende Release '$release' in $Lock" }
$baseUrl = $env:PDFIUM_BASE_URL
if (-not $baseUrl) { $baseUrl = 'https://github.com/bblanchon/pdfium-binaries/releases/download' }
# Nur HTTPS (wie curl --proto '=https' in fetch-pdfium.sh): -UpdateLock uebernimmt Hashes ungeprueft.
if ($baseUrl -notmatch '^https://') { Fail "PDFIUM_BASE_URL muss mit https:// beginnen: $baseUrl" }
$baseUrl = "$baseUrl/$release"
$cacheRoot = $env:PDFIUM_CACHE
if (-not $cacheRoot) { $cacheRoot = Join-Path $Dest '.downloads' }
$cache = Join-Path $cacheRoot ($release -replace '/', '-')

# --- Laden, pruefen, entpacken ------------------------------------------------------------------
$newHashes = @{}
foreach ($plat in $platforms) {
    $archive = "pdfium-$plat.tgz"
    $target = Join-Path $Dest $plat
    $libdir = Join-Path $target (Get-LibSubdir $plat)
    $marker = Join-Path $target '.pdfium-archive.sha256'
    $expected = $null
    if (-not $UpdateLock) {
        $expected = Get-LockSha $plat
        if (-not $expected) { Fail "$archive fehlt in tools/pdfium.lock" }
        if (-not $Force -and (Test-Path -LiteralPath $marker) -and (Test-Path -LiteralPath $libdir) -and
            ((Get-Content -LiteralPath $marker -Raw).Trim() -eq $expected)) {
            Log "${plat}: aktuell ($release)"
            continue
        }
    }

    New-Item -ItemType Directory -Force -Path $cache | Out-Null
    $file = Join-Path $cache $archive
    if ((Test-Path -LiteralPath $file) -and ($Force -or ($expected -and (Get-Sha256 $file) -ne $expected))) {
        Remove-Item -LiteralPath $file -Force
    }
    if (-not (Test-Path -LiteralPath $file)) {
        Log "${plat}: lade $baseUrl/$archive"
        $part = "$file.part"
        if (Test-Path -LiteralPath $part) { Remove-Item -LiteralPath $part -Force }
        try { Invoke-Download "$baseUrl/$archive" $part }
        catch {
            if (Test-Path -LiteralPath $part) { Remove-Item -LiteralPath $part -Force }
            Fail "${plat}: Download fehlgeschlagen: $($_.Exception.Message)"
        }
        Move-Item -LiteralPath $part -Destination $file -Force
    }

    $actual = Get-Sha256 $file
    if ($UpdateLock) { $newHashes[$plat] = $actual }
    elseif ($actual -ne $expected) {
        Remove-Item -LiteralPath $file -Force
        Fail "${plat}: SHA-256 stimmt nicht (erwartet $expected, erhalten $actual) - Archiv geloescht"
    }

    Test-ArchiveMembers $file
    $tmp = Join-Path $Dest ".tmp-$plat-$PID"
    if (Test-Path -LiteralPath $tmp) { Remove-Item -LiteralPath $tmp -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $tmp | Out-Null
    & $Tar -xzf $file -C $tmp
    if ($LASTEXITCODE -ne 0) { Remove-Item -LiteralPath $tmp -Recurse -Force; Fail "${plat}: Entpacken fehlgeschlagen" }
    if (-not (Test-Path -LiteralPath (Join-Path $tmp (Get-LibSubdir $plat)))) {
        Remove-Item -LiteralPath $tmp -Recurse -Force
        Fail "${plat}: $(Get-LibSubdir $plat)/ fehlt im Archiv"
    }
    Set-Content -LiteralPath (Join-Path $tmp '.pdfium-archive.sha256') -Value $actual -Encoding ASCII
    if (Test-Path -LiteralPath $target) { Remove-Item -LiteralPath $target -Recurse -Force }
    Move-Item -LiteralPath $tmp -Destination $target
    $build = (Get-Content -LiteralPath (Join-Path $target 'VERSION') | Where-Object { $_ -like 'BUILD=*' }) -replace '^BUILD=', ''
    Log "${plat}: entpackt nach $target (Build $build)"
}

if ($UpdateLock) {
    $header = @()
    foreach ($line in Get-LockLines) { if ($line -like '#*') { $header += $line } else { break } }
    $out = @($header) + @("release $release")
    foreach ($p in $AllPlatforms) {
        if ($newHashes.ContainsKey($p)) { $out += "$($newHashes[$p])  pdfium-$p.tgz" }
        else { $old = Get-LockSha $p; if ($old) { $out += "$old  pdfium-$p.tgz" } }
    }
    # LF-Zeilenenden und UTF-8 ohne BOM wie im Repository (.gitattributes).
    [System.IO.File]::WriteAllText($Lock, (($out -join "`n") + "`n"), (New-Object System.Text.UTF8Encoding $false))
    Log "tools/pdfium.lock aktualisiert ($release) - Hashes gegen die Release-Seite pruefen und Kopfkommentar anpassen"
}

# --- Ausgabe -----------------------------------------------------------------------------------
$first = $platforms[0]
$firstDir = Join-Path (Join-Path $Dest $first) (Get-LibSubdir $first)
if ($PrintDir) { Write-Output $firstDir }
if ($PrintEnv) { Write-Output ("`$env:PDFOVER_PDFIUM_DIR = '{0}'" -f ($firstDir -replace "'", "''")) }
if ($GitHubEnv) {
    if (-not $env:GITHUB_ENV) { Fail '-GitHubEnv: GITHUB_ENV ist nicht gesetzt' }
    # UTF-8 ohne BOM, auch unter Windows PowerShell 5.1 (dort schreibt Out-File -Encoding utf8 ein BOM,
    # das den ersten Variablennamen in GITHUB_ENV verfaelschen wuerde).
    [System.IO.File]::AppendAllText($env:GITHUB_ENV, "PDFOVER_PDFIUM_DIR=$firstDir`n", (New-Object System.Text.UTF8Encoding $false))
    Log "PDFOVER_PDFIUM_DIR=$firstDir -> GITHUB_ENV"
}
