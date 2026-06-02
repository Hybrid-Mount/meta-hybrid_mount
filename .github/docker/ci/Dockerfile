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

FROM ubuntu:24.04

ARG NDK_VERSION=r27
ARG NODE_MAJOR=22
ARG PNPM_VERSION=10
ARG RUST_TOOLCHAIN=nightly

ENV DEBIAN_FRONTEND=noninteractive
ENV ANDROID_NDK_HOME=/opt/android-ndk-${NDK_VERSION}
ENV ANDROID_NDK_ROOT=/opt/android-ndk-${NDK_VERSION}
ENV ANDROID_NDK_LATEST_HOME=/opt/android-ndk-${NDK_VERSION}
ENV CARGO_HOME=/root/.cargo
ENV RUSTUP_HOME=/root/.rustup
ENV PNPM_HOME=/root/.local/share/pnpm
ENV PATH=/root/.cargo/bin:/root/.local/share/pnpm:/opt/android-ndk-${NDK_VERSION}/toolchains/llvm/prebuilt/linux-x86_64/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    bash \
    build-essential \
    ca-certificates \
    clang \
    cmake \
    curl \
    file \
    gcc-multilib \
    gh \
    git \
    jq \
    libclang-dev \
    libssl-dev \
    llvm \
    make \
    perl \
    pkg-config \
    python3 \
    unzip \
    xz-utils \
    zip \
    zstd \
  && curl -fsSL "https://deb.nodesource.com/setup_${NODE_MAJOR}.x" | bash - \
  && apt-get install -y --no-install-recommends nodejs \
  && npm install -g "pnpm@${PNPM_VERSION}" \
  && rm -rf /var/lib/apt/lists/*

RUN curl -fsSLo /tmp/android-ndk.zip "https://dl.google.com/android/repository/android-ndk-${NDK_VERSION}-linux.zip" \
  && unzip -q /tmp/android-ndk.zip -d /opt \
  && rm /tmp/android-ndk.zip \
  && test -d "${ANDROID_NDK_HOME}"

RUN curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain "${RUST_TOOLCHAIN}" \
  && rustup target add aarch64-linux-android --toolchain "${RUST_TOOLCHAIN}" \
  && rustup component add rust-src clippy rustfmt --toolchain "${RUST_TOOLCHAIN}" \
  && cargo install cargo-ndk --locked \
  && cargo ndk --help >/dev/null

RUN rustc +${RUST_TOOLCHAIN} --version \
	&& gh --version \
  && cargo +${RUST_TOOLCHAIN} --version \
  && cargo ndk --version \
  && jq --version \
  && node --version \
  && pnpm --version \
  && "${ANDROID_NDK_HOME}/ndk-build" --version
