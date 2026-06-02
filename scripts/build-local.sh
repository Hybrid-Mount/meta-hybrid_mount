#!/usr/bin/env bash
# Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# Local build helper for Hybrid Mount test packages.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BUILD_MODE="debug"
BUILD_FLAVOR="full"
ARCH="arm64"
ALL_ARCH=false
SKIP_WEBUI=false
RUN_LINT=false
KASUMI_LKM_DIR="${HYBRID_MOUNT_KASUMI_LKM_DIR:-}"

usage() {
	cat <<'EOF'
Usage: ./scripts/build-local.sh [options]

Options:
  -r, --release               Build a release package
      --lite                  Build the lite package (no Kasumi frontend/backend/LKM)
      --nano                  Build the nano package (config-only, no WebUI/CLI/daemon)
  -a, --arch <arm64>
                              Build a single Android ABI (default: arm64)
      --all-arch              Build all supported Android ABIs (currently arm64 only)
      --skip-webui            Reuse the current WebUI assets
      --lint                  Run cargo xtask lint before building
      --kasumi-lkm-dir <DIR>  Stage .ko files from DIR into kasumi_lkm/
  -h, --help                  Show this help message

Examples:
  ./scripts/build-local.sh
  ./scripts/build-local.sh --release --arch arm64
  ./scripts/build-local.sh --kasumi-lkm-dir /path/to/kasumi-lkm
EOF
}

require_cmd() {
	if ! command -v "$1" >/dev/null 2>&1; then
		echo "error: required command not found: $1" >&2
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
	--lite)
		BUILD_FLAVOR="lite"
		shift
		;;
	--nano)
		BUILD_FLAVOR="nano"
		shift
		;;
	-a | --arch)
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
	--kasumi-lkm-dir)
		KASUMI_LKM_DIR="$2"
		shift 2
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

if [[ -n "$KASUMI_LKM_DIR" ]]; then
	if [[ ! -d "$KASUMI_LKM_DIR" ]]; then
		echo "error: Kasumi LKM directory not found: $KASUMI_LKM_DIR" >&2
		exit 1
	fi
	export HYBRID_MOUNT_KASUMI_LKM_DIR="$KASUMI_LKM_DIR"
fi

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
if [[ -n "${HYBRID_MOUNT_KASUMI_LKM_DIR:-}" ]]; then
	if [[ "$BUILD_FLAVOR" != "full" ]]; then
		echo "Kasumi LKM dir: ignored for $BUILD_FLAVOR build"
	else
		echo "Kasumi LKM dir: ${HYBRID_MOUNT_KASUMI_LKM_DIR}"
	fi
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
