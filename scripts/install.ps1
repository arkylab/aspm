# aspm - AI Skill Package Manager Installer
# https://github.com/arkylab/aspm

param(
    [string]$Version = "",
    [string]$InstallDir = ""
)

$ErrorActionPreference = "Stop"

$Repo = "arkylab/aspm"
$BinaryName = "aspm.exe"
$DefaultInstallDir = "$env:USERPROFILE\.local\bin"

function Write-Info {
    param([string]$Message)
    Write-Host "[INFO] " -ForegroundColor Green -NoNewline
    Write-Host $Message
}

function Write-Warn {
    param([string]$Message)
    Write-Host "[WARN] " -ForegroundColor Yellow -NoNewline
    Write-Host $Message
}

function Write-Err {
    param([string]$Message)
    Write-Host "[ERROR] " -ForegroundColor Red -NoNewline
    Write-Host $Message
    exit 1
}

# Detect architecture
function Get-Architecture {
    # Try RuntimeInformation first (works on most systems)
    try {
        $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
        if ($arch) {
            switch ($arch.ToString()) {
                { $_ -in @("X64", "x64") } { return "x86_64" }
                { $_ -in @("Arm64", "arm64", "ARM64") } { return "aarch64" }
                { $_ -in @("X86", "x86") } { Write-Err "32-bit Windows is not supported" }
            }
        }
    } catch {}

    # Fallback: use environment variable
    $procArch = $env:PROCESSOR_ARCHITECTURE
    if ($procArch) {
        switch ($procArch) {
            "AMD64" { return "x86_64" }
            "ARM64" { return "aarch64" }
            "x86" { Write-Err "32-bit Windows is not supported" }
        }
    }

    # Last fallback: WMI
    try {
        $wmiArch = (Get-CimInstance -ClassName Win32_Processor).AddressWidth
        switch ($wmiArch) {
            64 { return "x86_64" }
            32 { Write-Err "32-bit Windows is not supported" }
        }
    } catch {}

    Write-Err "Could not detect architecture. Please report this issue."
}

# Get latest version from GitHub API
function Get-LatestVersion {
    try {
        $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing
        return $release.tag_name
    } catch {
        Write-Err "Failed to get latest version: $_"
    }
}

# Add to PATH (user level)
function Add-ToPath {
    param([string]$Path)

    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($currentPath -notlike "*$Path*") {
        Write-Info "Adding $Path to PATH..."
        [Environment]::SetEnvironmentVariable("Path", "$currentPath;$Path", "User")
        return $true
    }
    return $false
}

# Main installation
function Main {
    Write-Host ""
    Write-Host "  ___   ___  _   _ _  __"
    Write-Host " / _ \ / _ \| | | | |/ /"
    Write-Host "| |_| | (_) | |_| | ' / "
    Write-Host " \__\_\___/ \__,_|_|\_\"
    Write-Host "   AI Skill Package Manager"
    Write-Host ""

    # Detect platform
    $arch = Get-Architecture
    $target = "$arch-pc-windows-msvc"
    Write-Info "Detected platform: $target"

    # Get version
    if ([string]::IsNullOrEmpty($Version)) {
        $Version = $env:INSTALL_VERSION
    }
    if ([string]::IsNullOrEmpty($Version)) {
        $Version = Get-LatestVersion
    }
    if ([string]::IsNullOrEmpty($Version)) {
        Write-Err "Failed to determine version. Please specify with -Version parameter."
    }
    Write-Info "Installing version: $Version"

    # Set install directory
    if ([string]::IsNullOrEmpty($InstallDir)) {
        $InstallDir = $DefaultInstallDir
    }

    # Create install directory
    if (-not (Test-Path $InstallDir)) {
        Write-Info "Creating install directory: $InstallDir"
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    # Download archive
    $archiveName = "aspm-$target.zip"
    $downloadUrl = "https://github.com/$Repo/releases/download/$Version/$archiveName"
    $tempFile = Join-Path $env:TEMP $archiveName

    Write-Info "Downloading from $downloadUrl"
    try {
        Invoke-WebRequest -Uri $downloadUrl -OutFile $tempFile -UseBasicParsing
    } catch {
        Write-Err "Failed to download: $_"
    }

    # Extract
    Write-Info "Extracting..."
    try {
        Expand-Archive -Path $tempFile -DestinationPath $InstallDir -Force
    } catch {
        Write-Err "Failed to extract: $_"
    }

    # Cleanup
    Remove-Item $tempFile -Force -ErrorAction SilentlyContinue

    # Verify installation
    $binaryPath = Join-Path $InstallDir $BinaryName
    if (-not (Test-Path $binaryPath)) {
        Write-Err "Binary not found at $binaryPath"
    }

    # Add to PATH
    $pathAdded = Add-ToPath -Path $InstallDir

    Write-Host ""
    Write-Info "Installation successful!"
    Write-Info "aspm has been installed to: $binaryPath"

    if ($pathAdded) {
        Write-Info "PATH has been updated. Please restart your terminal to use aspm."
    } else {
        Write-Info "Run 'aspm' to start using it."
    }

    Write-Host ""
}

Main
