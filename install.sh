#!/bin/bash
set -euo pipefail

# KIAS Installer Script
# Usage: curl -fsSL https://raw.githubusercontent.com/Andy-ckm/KIAS/main/install.sh | sh

KIAS_VERSION="${KIAS_VERSION:-latest}"
INSTALL_DIR="${KIAS_INSTALL_DIR:-/usr/local/bin}"
GITHUB_REPO="Andy-ckm/KIAS"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

# Detect OS and architecture
detect_platform() {
    local os arch

    case "$(uname -s)" in
        Linux*)     os="linux" ;;
        Darwin*)    os="macos" ;;
        MINGW*|MSYS*|CYGWIN*) os="windows" ;;
        *)          error "Unsupported OS: $(uname -s)" ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64)   arch="x86_64" ;;
        aarch64|arm64)  arch="aarch64" ;;
        *)              error "Unsupported architecture: $(uname -m)" ;;
    esac

    echo "${os}-${arch}"
}

# Get latest version from GitHub
get_latest_version() {
    curl -sL "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" \
        | grep '"tag_name":' \
        | sed -E 's/.*"([^"]+)".*/\1/' \
        || echo "v0.1.0"
}

# Download and install
install_kias() {
    local platform="$1"
    local version="$2"
    local tmpdir

    tmpdir=$(mktemp -d)
    trap "rm -rf $tmpdir" EXIT

    local ext="tar.gz"
    if [[ "$platform" == *"windows"* ]]; then
        ext="zip"
    fi

    local filename="kias-${platform}.${ext}"
    local url="https://github.com/${GITHUB_REPO}/releases/download/${version}/${filename}"

    info "Downloading KIAS ${version} for ${platform}..."
    curl -sL "$url" -o "${tmpdir}/${filename}" || error "Download failed"

    info "Extracting..."
    if [[ "$ext" == "tar.gz" ]]; then
        tar -xzf "${tmpdir}/${filename}" -C "${tmpdir}"
    else
        unzip -q "${tmpdir}/${filename}" -d "${tmpdir}"
    fi

    info "Installing to ${INSTALL_DIR}..."
    mkdir -p "${INSTALL_DIR}"
    cp "${tmpdir}/kias" "${INSTALL_DIR}/kias" 2>/dev/null || \
    cp "${tmpdir}/kias-main" "${INSTALL_DIR}/kias" 2>/dev/null || \
    error "Binary not found"

    chmod +x "${INSTALL_DIR}/kias"

    # Verify installation
    if command -v kias &>/dev/null; then
        info "✓ KIAS installed successfully!"
        kias --version
    else
        warn "KIAS installed to ${INSTALL_DIR}"
        warn "Add to PATH: export PATH=\"${INSTALL_DIR}:\$PATH\""
    fi
}

# Main
main() {
    info "KIAS Installer"
    info "=============="

    local platform
    platform=$(detect_platform)
    info "Detected platform: ${platform}"

    local version
    if [[ "$KIAS_VERSION" == "latest" ]]; then
        version=$(get_latest_version)
    else
        version="$KIAS_VERSION"
    fi
    info "Version: ${version}"

    install_kias "$platform" "$version"

    echo ""
    info "Quick Start:"
    info "  kias config init    # Initialize configuration"
    info "  kias start          # Start KIAS server"
    info ""
    info "Documentation: https://github.com/${GITHUB_REPO}"
}

main "$@"
