#!/usr/bin/env bash
# Install cgrep from GitHub release assets.
#
# Usage:
#   scripts/install_release.sh
#   scripts/install_release.sh --version v1.5.1 --bin-dir ~/.local/bin

set -euo pipefail

REPO="${CGREP_REPO:-meghendra6/cgrep}"
VERSION="latest"
BIN_DIR="${HOME}/.local/bin"
FORCE=0

usage() {
  cat <<'EOF'
Install cgrep release binary.

Options:
  -v, --version <tag>   Release tag (default: latest)
  -b, --bin-dir <dir>   Install directory (default: ~/.local/bin)
  -f, --force           Overwrite existing binary
  -h, --help            Show this help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -v|--version)
      VERSION="${2:-}"
      shift 2
      ;;
    -b|--bin-dir)
      BIN_DIR="${2:-}"
      shift 2
      ;;
    -f|--force)
      FORCE=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

resolve_latest_tag() {
  if command -v gh >/dev/null 2>&1; then
    local tag
    if tag="$(gh release view --repo "${REPO}" --json tagName --jq .tagName 2>/dev/null)" && [[ -n "${tag}" ]]; then
      echo "${tag}"
      return
    fi
  fi

  curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' \
    | head -n 1
}

version_lt() {
  local lhs="$1"
  local rhs="$2"
  [[ "$(printf '%s\n%s\n' "${lhs}" "${rhs}" | sort -V | head -n 1)" != "${rhs}" ]]
}

detect_glibc_version() {
  if command -v getconf >/dev/null 2>&1; then
    local v
    v="$(getconf GNU_LIBC_VERSION 2>/dev/null | awk '{print $2}')"
    if [[ -n "${v}" ]]; then
      echo "${v}"
      return
    fi
  fi

  if command -v ldd >/dev/null 2>&1; then
    ldd --version 2>/dev/null \
      | sed -n '1s/.* \([0-9][0-9.]*\)$/\1/p'
  fi
}

host_os="$(uname -s | tr '[:upper:]' '[:lower:]')"
host_arch="$(uname -m)"

case "${host_os}" in
  darwin)
    case "${host_arch}" in
      arm64|aarch64) target="aarch64-apple-darwin" ;;
      x86_64) target="x86_64-apple-darwin" ;;
      *)
        echo "Unsupported macOS architecture: ${host_arch}" >&2
        exit 1
        ;;
    esac
    ;;
  linux)
    case "${host_arch}" in
      x86_64|amd64) target="x86_64-unknown-linux-gnu" ;;
      *)
        echo "Unsupported Linux architecture: ${host_arch}" >&2
        exit 1
        ;;
    esac
    ;;
  *)
    echo "Unsupported OS: ${host_os}" >&2
    echo "Use release assets directly for this platform." >&2
    exit 1
    ;;
esac

if [[ "${VERSION}" == "latest" ]]; then
  VERSION="$(resolve_latest_tag)"
  if [[ -z "${VERSION}" ]]; then
    echo "Failed to resolve latest release tag." >&2
    exit 1
  fi
fi

archive="cgrep-${VERSION}-${target}.tar.gz"
checksum="${archive}.sha256"
base_url="https://github.com/${REPO}/releases/download/${VERSION}"

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "${tmp_dir}"
}
trap cleanup EXIT

echo "Installing ${archive} ..."
curl -fL "${base_url}/${archive}" -o "${tmp_dir}/${archive}"
curl -fL "${base_url}/${checksum}" -o "${tmp_dir}/${checksum}"

if command -v shasum >/dev/null 2>&1; then
  actual_hash="$(shasum -a 256 "${tmp_dir}/${archive}" | awk '{print $1}')"
else
  actual_hash="$(sha256sum "${tmp_dir}/${archive}" | awk '{print $1}')"
fi
expected_hash="$(awk '{print $1}' "${tmp_dir}/${checksum}")"

if [[ -z "${expected_hash}" || "${actual_hash}" != "${expected_hash}" ]]; then
  echo "Checksum verification failed for ${archive}" >&2
  exit 1
fi

tar -xzf "${tmp_dir}/${archive}" -C "${tmp_dir}"
bin_src="$(find "${tmp_dir}" -type f -name cgrep | head -n 1)"
if [[ -z "${bin_src}" ]]; then
  echo "Could not find cgrep binary in extracted archive." >&2
  exit 1
fi

if [[ "${host_os}" == "linux" ]] && command -v strings >/dev/null 2>&1; then
  host_glibc="$(detect_glibc_version || true)"
  required_glibc="$(strings "${bin_src}" | sed -n 's/^GLIBC_//p' | sort -V | tail -n 1)"
  if [[ -n "${host_glibc}" && -n "${required_glibc}" ]] && version_lt "${host_glibc}" "${required_glibc}"; then
    echo "Incompatible Linux release binary: requires glibc >= ${required_glibc}, found ${host_glibc}." >&2
    echo "Try a newer release artifact or install from source: cargo install --path ." >&2
    exit 1
  fi
fi

mkdir -p "${BIN_DIR}"
dst="${BIN_DIR}/cgrep"
if [[ -e "${dst}" && "${FORCE}" -ne 1 ]]; then
  echo "Destination exists: ${dst}" >&2
  echo "Use --force to overwrite." >&2
  exit 1
fi

install -m 755 "${bin_src}" "${dst}"

if [[ "${host_os}" == "darwin" ]] && command -v xattr >/dev/null 2>&1; then
  xattr -d com.apple.quarantine "${dst}" 2>/dev/null || true
fi

echo "Installed to ${dst}"
"${dst}" --version
