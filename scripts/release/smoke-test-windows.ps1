<#
.SYNOPSIS
  Test de démarrage d'Augur : installe la version construite, la lance et
  vérifie qu'elle tient debout.

.DESCRIPTION
  Garde-fou contre un crash au lancement comme celui de la v0.1.36
  (« state() called before manage() », corrigé en v0.1.37) : les tests
  unitaires passaient, mais l'application s'arrêtait dès son ouverture.

  Étapes :
    1. installe l'installeur NSIS en silencieux (/S, installation utilisateur) ;
    2. lance l'exécutable installé ;
    3. pendant -Seconds secondes, échoue si le processus s'arrête ou si
       augur.log contient une ligne ERREUR (une panne y est écrite par le
       panic hook avant l'arrêt) ;
    4. exige la ligne « démarrage d'Augur vX.Y.Z » dans augur.log, preuve
       que le setup a tourné et que le journal est branché ;
    5. ferme Augur.

  Seules les lignes ajoutées au journal pendant le test sont lues : rien
  n'est supprimé, le script peut aussi être lancé sur un PC de développement.

.PARAMETER Installer
  Chemin de l'installeur NSIS (*-setup.exe).

.PARAMETER Version
  Version attendue dans le journal (ex. 0.1.38). Facultative.

.PARAMETER Seconds
  Durée pendant laquelle Augur doit rester vivant (20 s par défaut).
#>
param(
  [Parameter(Mandatory = $true)]
  [string] $Installer,
  [string] $Version = '',
  [int] $Seconds = 20
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$identifier = 'com.shigenotv.augur'
# app_log_dir() de Tauri sous Windows : %LOCALAPPDATA%\<identifier>\logs
$logFile = Join-Path $env:LOCALAPPDATA "$identifier\logs\augur.log"

function Fail([string] $message) {
  Write-Host "::error::Test de démarrage : $message"
  exit 1
}

function Get-NewLogLines([long] $offset) {
  if (-not (Test-Path -LiteralPath $logFile)) { return @() }
  # Augur garde augur.log ouvert en écriture : lecture en partage ReadWrite.
  $stream = [System.IO.File]::Open($logFile, 'Open', 'Read', 'ReadWrite, Delete')
  try {
    $bytes = New-Object byte[] $stream.Length
    $read = 0
    while ($read -lt $bytes.Length) {
      $n = $stream.Read($bytes, $read, $bytes.Length - $read)
      if ($n -le 0) { break }
      $read += $n
    }
  } finally { $stream.Dispose() }
  if ($bytes.Length -le $offset) { return @() }
  $text = [System.Text.Encoding]::UTF8.GetString($bytes, [int] $offset, $bytes.Length - [int] $offset)
  return @($text -split "`r?`n" | Where-Object { $_ })
}

function Find-InstalledExe {
  # L'installeur NSIS de Tauri déclare l'exécutable dans DisplayIcon.
  $keys = Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' -ErrorAction SilentlyContinue
  foreach ($key in $keys) {
    $props = Get-ItemProperty $key.PSPath
    if ($props.PSObject.Properties['DisplayName'] -and $props.DisplayName -eq 'Augur' -and $props.PSObject.Properties['DisplayIcon']) {
      $exe = $props.DisplayIcon.Trim('"') -replace ',\d+$', ''
      if (Test-Path -LiteralPath $exe) { return $exe }
    }
  }
  # Repli : dossier d'installation par défaut (installMode currentUser).
  $dir = Join-Path $env:LOCALAPPDATA 'Augur'
  $found = Get-ChildItem -LiteralPath $dir -Filter *.exe -ErrorAction SilentlyContinue |
    Where-Object Name -notmatch '^uninstall'
  if ($found) { return @($found)[0].FullName }
  return $null
}

if (-not (Test-Path -LiteralPath $Installer)) { Fail "installeur introuvable : $Installer" }

# 1. Installation silencieuse
Write-Host "Installation de $Installer"
$setup = Start-Process -FilePath $Installer -ArgumentList '/S' -Wait -PassThru
if ($setup.ExitCode -ne 0) { Fail "l'installeur a échoué (code $($setup.ExitCode))" }
$exe = Find-InstalledExe
if (-not $exe) { Fail "exécutable d'Augur introuvable après l'installation" }
Write-Host "Exécutable installé : $exe"

# 2. Lancement
$offset = if (Test-Path -LiteralPath $logFile) { (Get-Item -LiteralPath $logFile).Length } else { 0 }
$app = Start-Process -FilePath $exe -PassThru
Write-Host "Augur lancé (PID $($app.Id)), observation pendant $Seconds s"

# 3. Observation
$failure = $null
$deadline = (Get-Date).AddSeconds($Seconds)
while ((Get-Date) -lt $deadline) {
  Start-Sleep -Milliseconds 500
  if ($app.HasExited) {
    $failure = "Augur s'est arrêté au bout de quelques secondes (code $($app.ExitCode))"
    break
  }
  if (Get-NewLogLines $offset | Where-Object { $_ -match ' ERREUR ' }) {
    $failure = 'erreur écrite dans augur.log'
    break
  }
}

$lines = Get-NewLogLines $offset
Write-Host "--- augur.log (lignes du test) ---"
$lines | ForEach-Object { Write-Host $_ }
Write-Host "----------------------------------"
$lines | Where-Object { $_ -match ' ALERTE ' } | ForEach-Object { Write-Host "::warning::augur.log : $_" }

# 4. Preuve que le setup a tourné
if (-not $failure) {
  $attendu = if ($Version) { "Augur v$Version" } else { 'Augur v' }
  if (-not ($lines | Where-Object { $_ -match ' info ' -and $_.Contains($attendu) })) {
    $failure = "ligne de démarrage « $attendu » absente de augur.log ($logFile)"
  }
}

if (-not $failure) {
  $app.Refresh()
  if ($app.MainWindowHandle -eq 0) {
    Write-Host '::warning::Aucune fenêtre principale détectée (session sans bureau ?), le processus reste vivant.'
  }
}

# 5. Fermeture (Augur et ses processus WebView2)
if (-not $app.HasExited) {
  & taskkill.exe /PID $app.Id /T /F | Out-Null
}

if ($failure) { Fail $failure }
Write-Host "Test de démarrage réussi : Augur est resté vivant $Seconds s sans erreur."
