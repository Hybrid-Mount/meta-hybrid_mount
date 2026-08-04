#!/usr/bin/env bash
# Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
#
# SPDX-License-Identifier: GPL-3.0-only

# Local build helper for Hybrid Mount test packages.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BUILD_MODE="debug"
BUILD_FLAVOR="lite"
ARCH="arm64"
ALL_ARCH=false
SKIP_WEBUI=false
RUN_LINT=false

usage() {
	cat <<'EOF'
Usage: ./scripts/build-local.sh [options]

Options:
  -r, --release               Build a release package
      --nano                  Build the nano package (config-only, no WebUI/CLI/daemon)
  -a, --arch <arm64>
                              Build a single Android ABI (default: arm64)
      --all-arch              Build all supported Android ABIs (currently arm64 only)
      --skip-webui            Reuse the current WebUI assets
      --lint                  Run cargo xtask lint before building
  -h, --help                  Show this help message

Examples:
  ./scripts/build-local.sh
  ./scripts/build-local.sh --release --arch arm64
EOF
}

require_cmd() {
	if ! command -v "$1" >/dev/null 2>&1; then
		echo "error: required command not found: $1" >&2
		exit 1
	fi
}

require_option_value() {
	local option="$1"
	local value="${2:-}"
	if [[ -z "$value" || "$value" == -* ]]; then
		echo "error: $option requires a value" >&2
		usage
		exit 1
	fi
}

detect_ndk_home() {
	local candidates=(
		"${ANDROID_NDK_HOME:-}"
		"${ANDROID_NDK_LATEST_HOME:-}"
		"${ANDROID_NDK_ROOT:-}"
		"${ANDROID_NDK:-}"
	)
	local candidate
	for candidate in "${candidates[@]}"; do
		if [[ -n "$candidate" && -d "$candidate" ]]; then
			echo "$candidate"
			return 0
		fi
	done
	return 1
}

while [[ $# -gt 0 ]]; do
	case "$1" in
	-r | --release)
		BUILD_MODE="release"
		shift
		;;
	--nano)
		BUILD_FLAVOR="nano"
		shift
		;;
	-a | --arch)
		require_option_value "$1" "${2:-}"
		ARCH="$2"
		shift 2
		;;
	--all-arch)
		ALL_ARCH=true
		shift
		;;
	--skip-webui)
		SKIP_WEBUI=true
		shift
		;;
	--lint)
		RUN_LINT=true
		shift
		;;
	-h | --help)
		usage
		exit 0
		;;
	*)
		echo "error: unknown option: $1" >&2
		usage
		exit 1
		;;
	esac
done

case "$ARCH" in
arm64) ;;
*)
	echo "error: unsupported arch: $ARCH" >&2
	exit 1
	;;
esac

require_cmd cargo

if ! cargo ndk --help >/dev/null 2>&1; then
	echo "error: cargo-ndk is required. Install it with: cargo install cargo-ndk" >&2
	exit 1
fi

if [[ "$BUILD_FLAVOR" != "nano" && "$SKIP_WEBUI" != "true" ]]; then
	require_cmd pnpm
fi

NDK_HOME="$(detect_ndk_home || true)"
if [[ -z "$NDK_HOME" ]]; then
	echo "error: Android NDK not found. Set ANDROID_NDK_HOME (or ANDROID_NDK_LATEST_HOME)." >&2
	exit 1
fi
export ANDROID_NDK_HOME="$NDK_HOME"

cd "$REPO_ROOT"

echo "== Hybrid Mount local build =="
echo "Mode: $BUILD_MODE"
echo "Flavor: $BUILD_FLAVOR"
if [[ "$ALL_ARCH" == "true" ]]; then
	echo "Arch: all"
else
	echo "Arch: $ARCH"
fi
echo "NDK: $ANDROID_NDK_HOME"
if [[ "$BUILD_FLAVOR" == "nano" ]]; then
	echo "WebUI: omitted"
elif [[ "$SKIP_WEBUI" == "true" ]]; then
	echo "WebUI: skip"
else
	echo "WebUI: build"
fi
echo

if [[ "$RUN_LINT" == "true" ]]; then
	echo ">>> Running lint"
	cargo run -p xtask -- lint
	echo
fi

build_args=(run -p xtask -- build)
if [[ "$BUILD_MODE" == "release" ]]; then
	build_args+=(--release)
fi
build_args+=(--flavor "$BUILD_FLAVOR")
if [[ "$SKIP_WEBUI" == "true" ]]; then
	build_args+=(--skip-webui)
fi
if [[ "$ALL_ARCH" != "true" ]]; then
	build_args+=(--arch "$ARCH")
fi

echo ">>> Building package"
cargo "${build_args[@]}"
echo
echo "Artifacts:"
ls -lh "$REPO_ROOT"/output/*.zip
