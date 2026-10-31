<#
.SYNOPSIS
  Signe un binaire Windows (Authenticode) pour la release d'Augur.

.DESCRIPTION
  Appelé par `tauri build` via bundle.windows.signCommand pour CHAQUE fichier
  à signer (exécutable de l'application, puis installeurs NSIS et MSI).
  Le mode est choisi par la variable AUGUR_SIGNING_MODE :

    azure     Azure Artifact Signing (ex-« Trusted Signing ») via trusted-signing-cli.
              Variables : AZURE_TENANT_ID, AZURE_CLIENT_ID, AZURE_CLIENT_SECRET,
                          AZURE_SIGNING_ENDPOINT, AZURE_SIGNING_ACCOUNT,
                          AZURE_SIGNING_PROFILE.
    signtool  Certificat installé dans le magasin Windows du runner, par exemple
              via le KSP d'un HSM cloud (DigiCert KeyLocker, SSL.com eSigner…).
              Variables : AUGUR_SIGNING_THUMBPRINT, AUGUR_TIMESTAMP_URL (optionnelle).
    none      Aucune signature (builds de test). Un avertissement est émis.

  Aucun secret n'est écrit sur disque ni affiché dans les journaux.

.PARAMETER File
  Chemin du fichier à signer (le %1 de signCommand).
#>
param(
  [Parameter(Mandatory = $true, Position = 0)]
  [string] $File
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$mode = if ($env:AUGUR_SIGNING_MODE) { $env:AUGUR_SIGNING_MODE } else { 'none' }
$description = 'Augur'
$timestampUrl = if ($env:AUGUR_TIMESTAMP_URL) { $env:AUGUR_TIMESTAMP_URL } else { 'http://timestamp.acs.microsoft.com' }

if (-not (Test-Path -LiteralPath $File)) {
  throw "Fichier à signer introuvable : $File"
}

function Require-Env([string[]] $names) {
  foreach ($n in $names) {
    if (-not [Environment]::GetEnvironmentVariable($n)) {
      throw "Variable d'environnement manquante pour le mode '$mode' : $n"
    }
  }
}

function Find-SignTool {
  $cmd = Get-Command signtool.exe -ErrorAction SilentlyContinue
  if ($cmd) { return $cmd.Source }
  $kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
  $found = Get-ChildItem -Path $kits -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match '\\x64\\' } |
    Sort-Object FullName -Descending |
    Select-Object -First 1
  if (-not $found) { throw 'signtool.exe introuvable (Windows SDK requis).' }
  return $found.FullName
}

# Les serveurs d'horodatage échouent parfois de façon transitoire : on réessaie.
function Invoke-WithRetry([scriptblock] $action, [int] $attempts = 3) {
  for ($i = 1; $i -le $attempts; $i++) {
    & $action
    if ($LASTEXITCODE -eq 0) { return }
    if ($i -lt $attempts) {
      $delay = [math]::Pow(2, $i) * 5
      Write-Warning "Échec de signature (tentative $i/$attempts), nouvel essai dans $delay s."
      Start-Sleep -Seconds $delay
    }
  }
  throw "Signature impossible après $attempts tentatives : $File"
}

switch ($mode) {
  'azure' {
    Require-Env @('AZURE_TENANT_ID', 'AZURE_CLIENT_ID', 'AZURE_CLIENT_SECRET',
                  'AZURE_SIGNING_ENDPOINT', 'AZURE_SIGNING_ACCOUNT', 'AZURE_SIGNING_PROFILE')
    if (-not (Get-Command trusted-signing-cli -ErrorAction SilentlyContinue)) {
      throw 'trusted-signing-cli introuvable (cargo install trusted-signing-cli).'
    }
    Invoke-WithRetry {
      trusted-signing-cli `
        -e $env:AZURE_SIGNING_ENDPOINT `
        -a $env:AZURE_SIGNING_ACCOUNT `
        -c $env:AZURE_SIGNING_PROFILE `
        -d $description `
        $File
    }
  }
  'signtool' {
    Require-Env @('AUGUR_SIGNING_THUMBPRINT')
    $signtool = Find-SignTool
    Invoke-WithRetry {
      & $signtool sign /sha1 $env:AUGUR_SIGNING_THUMBPRINT /fd sha256 `
        /tr $timestampUrl /td sha256 /d $description $File
    }
  }
  'none' {
    Write-Warning "AUGUR_SIGNING_MODE=none : $File n'est PAS signé (SmartScreen avertira l'utilisateur)."
    exit 0
  }
  default {
    throw "AUGUR_SIGNING_MODE inconnu : '$mode' (attendu : azure, signtool, none)"
  }
}

$sig = Get-AuthenticodeSignature -LiteralPath $File
if ($sig.Status -ne 'Valid') {
  throw "Signature non valide après signature de $File : $($sig.Status) $($sig.StatusMessage)"
}
Write-Host "Signé : $(Split-Path -Leaf $File) ($($sig.SignerCertificate.Subject))"
