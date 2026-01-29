# Install JetBrainsMono Nerd Font
# Run as Administrator for system-wide installation

$FontName = "JetBrainsMono"
$url = "https://github.com/ryanoasis/nerd-fonts/releases/latest/download/$FontName.zip"
$zip = "$env:TEMP\$FontName.zip"
$extract = "$env:TEMP\$FontName"

Write-Host "Downloading $FontName Nerd Font..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $url -OutFile $zip

Write-Host "Extracting fonts..." -ForegroundColor Cyan
if (Test-Path $extract) {
    Remove-Item -Path $extract -Recurse -Force
}
Expand-Archive -Path $zip -DestinationPath $extract -Force

Write-Host "Installing fonts..." -ForegroundColor Cyan
$fonts = (New-Object -ComObject Shell.Application).Namespace(0x14)
$fontFiles = Get-ChildItem -Path $extract -Filter "*.ttf"

foreach ($font in $fontFiles) {
    Write-Host "  Installing $($font.Name)..." -ForegroundColor Gray
    $fonts.CopyHere($font.FullName, 0x10)
}

Write-Host "Cleaning up..." -ForegroundColor Cyan
Remove-Item -Path $zip -Force
Remove-Item -Path $extract -Recurse -Force

Write-Host "Done! $FontName Nerd Font installed successfully." -ForegroundColor Green
Write-Host "You may need to restart applications to see the new font." -ForegroundColor Yellow
