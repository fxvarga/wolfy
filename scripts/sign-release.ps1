<#
.SYNOPSIS
    Signs the wolfy release binary with a code signing certificate.
.DESCRIPTION
    Creates a self-signed code signing certificate if one doesn't exist,
    then signs the release binary. Run this after each cargo build --release.
.NOTES
    The certificate is created once and reused for all future builds.
    Note: Creating a new certificate may require running as Administrator.
#>

param(
    [string]$BinaryPath = "$PSScriptRoot\..\target\release\wolfy.exe",
    [string]$CertSubject = "CN=Wolfy Dev",
    [string]$TimestampServer = "http://timestamp.digicert.com"
)

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "Wolfy Code Signing Script" -ForegroundColor Cyan
Write-Host "========================="

# Resolve full path
$BinaryPath = Resolve-Path $BinaryPath -ErrorAction SilentlyContinue
if (-not $BinaryPath -or -not (Test-Path $BinaryPath)) {
    Write-Host "ERROR: Binary not found. Run 'cargo build --release' first." -ForegroundColor Red
    exit 1
}

Write-Host "Binary: $BinaryPath"

# Check for existing certificate
$cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Where-Object { $_.Subject -eq $CertSubject } | Select-Object -First 1

if ($cert) {
    $thumbprint = $cert.Thumbprint.Substring(0,8)
    Write-Host "Found existing certificate: $thumbprint..." -ForegroundColor Green

    # Check expiration
    if ($cert.NotAfter -lt (Get-Date)) {
        Write-Host "Certificate expired. Creating new one..." -ForegroundColor Yellow
        $cert = $null
    }
}

if (-not $cert) {
    Write-Host "Creating new code signing certificate..." -ForegroundColor Yellow

    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject $CertSubject -CertStoreLocation Cert:\CurrentUser\My -NotAfter (Get-Date).AddYears(5) -KeyUsage DigitalSignature -KeyAlgorithm RSA -KeyLength 2048

    Write-Host "Certificate created: $($cert.Thumbprint)" -ForegroundColor Green
    $validUntil = $cert.NotAfter.ToString("yyyy-MM-dd")
    Write-Host "Valid until: $validUntil" -ForegroundColor Gray
}

# Sign the binary
Write-Host ""
Write-Host "Signing binary..." -ForegroundColor Cyan

try {
    $result = Set-AuthenticodeSignature -FilePath $BinaryPath -Certificate $cert -TimestampServer $TimestampServer -HashAlgorithm SHA256

    if ($result.Status -eq "Valid") {
        Write-Host "Binary signed successfully!" -ForegroundColor Green
    } else {
        Write-Host "Signature status: $($result.Status)" -ForegroundColor Yellow
        Write-Host "Message: $($result.StatusMessage)" -ForegroundColor Gray
    }
} catch {
    Write-Host "Signing failed: $_" -ForegroundColor Red
    exit 1
}

# Verify signature
Write-Host ""
Write-Host "Verifying signature..." -ForegroundColor Cyan
$sig = Get-AuthenticodeSignature $BinaryPath
Write-Host "Status: $($sig.Status)"
Write-Host "Signer: $($sig.SignerCertificate.Subject)"
if ($sig.TimeStamperCertificate) {
    Write-Host "Timestamp: $($sig.TimeStamperCertificate.Subject)"
}

# Copy to dist if it exists
$distPath = "$PSScriptRoot\..\dist\wolfy.exe"
$distDir = Split-Path $distPath
if (Test-Path $distDir) {
    Copy-Item $BinaryPath $distPath -Force
    Write-Host ""
    Write-Host "Copied signed binary to dist\" -ForegroundColor Green
}

Write-Host ""
Write-Host "Done!" -ForegroundColor Cyan
