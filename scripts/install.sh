#!/bin/sh
# aspm - AI Skill Package Manager Installer
# https://github.com/arkylab/aspm

set -e

REPO="arkylab/aspm"
INSTALL_DIR="$HOME/.local/bin"
BINARY_NAME="aspm"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() {
    echo "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo "${RED}[ERROR]${NC} $1"
    exit 1
}

# Detect operating system
detect_os() {
    OS=$(uname -s | tr '[:upper:]' '[:lower:]')
    case "$OS" in
        linux) OS="unknown-linux-gnu" ;;
        darwin) OS="apple-darwin" ;;
        *) error "Unsupported operating system: $OS" ;;
    esac
    echo "$OS"
}

# Detect architecture
detect_arch() {
    ARCH=$(uname -m)
    case "$ARCH" in
        x86_64|amd64) ARCH="x86_64" ;;
        aarch64|arm64) ARCH="aarch64" ;;
        *) error "Unsupported architecture: $ARCH" ;;
    esac
    echo "$ARCH"
}

# Get latest version from GitHub API
get_latest_version() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
    else
        error "Neither curl nor wget is available. Please install one of them."
    fi
}

# Download file
download() {
    url="$1"
    output="$2"

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "$output"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$url" -O "$output"
    fi
}

# Main installation
main() {
    echo ""
    echo "  ___   ___  _   _ _  __"
    echo " / _ \\ / _ \\| | | | |/ /"
    echo "| |_| | (_) | |_| | ' / "
    echo " \\__\\_\\\\___/ \\__,_|_|\\_\\"
    echo "   AI Skill Package Manager"
    echo ""

    # Detect platform
    OS=$(detect_os)
    ARCH=$(detect_arch)
    TARGET="$ARCH-$OS"

    info "Detected platform: $TARGET"

    # Get version
    VERSION="${INSTALL_VERSION:-$(get_latest_version)}"
    if [ -z "$VERSION" ]; then
        error "Failed to determine version. Please specify with INSTALL_VERSION environment variable."
    fi
    info "Installing version: $VERSION"

    # Create temporary directory
    TEMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TEMP_DIR"' EXIT

    # Download archive
    ARCHIVE_NAME="aspm-$TARGET.tar.gz"
    DOWNLOAD_URL="https://github.com/$REPO/releases/download/$VERSION/$ARCHIVE_NAME"

    info "Downloading from $DOWNLOAD_URL"
    download "$DOWNLOAD_URL" "$TEMP_DIR/$ARCHIVE_NAME"

    if [ ! -f "$TEMP_DIR/$ARCHIVE_NAME" ]; then
        error "Failed to download $ARCHIVE_NAME"
    fi

    # Extract
    info "Extracting..."
    cd "$TEMP_DIR"
    tar -xzf "$ARCHIVE_NAME"

    if [ ! -f "$BINARY_NAME" ]; then
        error "Binary not found in archive"
    fi

    # Install
    info "Installing to $INSTALL_DIR"
    mkdir -p "$INSTALL_DIR"
    mv "$BINARY_NAME" "$INSTALL_DIR/"
    chmod +x "$INSTALL_DIR/$BINARY_NAME"

    # Check PATH - both current session and shell config
    PATH_ALREADY_SET=false

    # Check current PATH
    if echo "$PATH" | grep -q "$INSTALL_DIR"; then
        PATH_ALREADY_SET=true
    fi

    # Check shell config files (idempotent check)
    for config_file in "$HOME/.bashrc" "$HOME/.bash_profile" "$HOME/.zshrc" "$HOME/.profile"; do
        if [ -f "$config_file" ] && grep -q "$INSTALL_DIR" "$config_file" 2>/dev/null; then
            PATH_ALREADY_SET=true
            break
        fi
    done

    if [ "$PATH_ALREADY_SET" = false ]; then
        info "Adding $INSTALL_DIR to PATH"

        # Detect shell config file
        SHELL_CONFIG=""
        case "$SHELL" in
            */bash)
                if [ -f "$HOME/.bashrc" ]; then
                    SHELL_CONFIG="$HOME/.bashrc"
                elif [ -f "$HOME/.bash_profile" ]; then
                    SHELL_CONFIG="$HOME/.bash_profile"
                fi
                ;;
            */zsh)
                if [ -f "$HOME/.zshrc" ]; then
                    SHELL_CONFIG="$HOME/.zshrc"
                fi
                ;;
            */fish)
                SHELL_CONFIG="$HOME/.config/fish/config.fish"
                mkdir -p "$(dirname "$SHELL_CONFIG")"
                ;;
        esac

        if [ -n "$SHELL_CONFIG" ]; then
            # Check if the exact line already exists (idempotent)
            MARKER="# Added by aspm installer"
            if ! grep -q "$MARKER" "$SHELL_CONFIG" 2>/dev/null; then
                echo "" >> "$SHELL_CONFIG"
                echo "$MARKER" >> "$SHELL_CONFIG"
                if [ "$SHELL" = "*/fish" ]; then
                    echo "set -gx PATH \$PATH $INSTALL_DIR" >> "$SHELL_CONFIG"
                else
                    echo "export PATH=\"\$PATH:$INSTALL_DIR\"" >> "$SHELL_CONFIG"
                fi
                info "Added to $SHELL_CONFIG"
            fi
        else
            warn "Could not detect shell config file. Please add the following to your shell config:"
            echo ""
            echo "    export PATH=\"\$PATH:$INSTALL_DIR\""
            echo ""
        fi
    else
        info "PATH already configured"
    fi

    echo ""
    info "Installation successful!"
    info "aspm has been installed to: $INSTALL_DIR/$BINARY_NAME"
    info "Run 'source ~/.bashrc' (or restart your terminal) to use aspm"
    echo ""
}

main "$@"
