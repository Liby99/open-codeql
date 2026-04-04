FROM --platform=linux/amd64 ubuntu:24.04

# Prevent interactive prompts during package installation
ENV DEBIAN_FRONTEND=noninteractive
ENV TZ=UTC

# ================================================================
# Core system utilities
# ================================================================
RUN apt-get update && apt-get install -y \
    vim tmux git curl wget unzip zip \
    build-essential pkg-config libssl-dev \
    ca-certificates gnupg lsb-release \
    software-properties-common apt-transport-https \
    jq ripgrep fd-find bat \
    locales \
    && locale-gen en_US.UTF-8 \
    && rm -rf /var/lib/apt/lists/*

ENV LANG=en_US.UTF-8
ENV LC_ALL=en_US.UTF-8

# ================================================================
# C / C++ toolchain
# ================================================================
RUN apt-get update && apt-get install -y \
    gcc g++ clang clang-format clang-tidy \
    cmake make ninja-build \
    gdb valgrind \
    && rm -rf /var/lib/apt/lists/*

# ================================================================
# Java (OpenJDK 17 — also needed by CodeQL CLI itself)
# ================================================================
RUN apt-get update && apt-get install -y \
    openjdk-17-jdk maven gradle \
    && rm -rf /var/lib/apt/lists/*
ENV JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64

# ================================================================
# Python 3
# ================================================================
RUN apt-get update && apt-get install -y \
    python3 python3-pip python3-venv python3-dev \
    && rm -rf /var/lib/apt/lists/*

# ================================================================
# Node.js 20 LTS (for JavaScript / TypeScript)
# ================================================================
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# ================================================================
# Go (official binary, not Ubuntu's outdated package)
# ================================================================
RUN curl -fsSL https://go.dev/dl/go1.23.4.linux-amd64.tar.gz | tar -C /usr/local -xz
ENV PATH="/usr/local/go/bin:${PATH}"
ENV GOPATH="/root/go"
ENV PATH="${GOPATH}/bin:${PATH}"

# ================================================================
# Ruby
# ================================================================
RUN apt-get update && apt-get install -y \
    ruby ruby-dev \
    && rm -rf /var/lib/apt/lists/*

# ================================================================
# C# / .NET SDK 8
# ================================================================
RUN wget -q https://packages.microsoft.com/config/ubuntu/24.04/packages-microsoft-prod.deb -O /tmp/packages-microsoft-prod.deb \
    && dpkg -i /tmp/packages-microsoft-prod.deb \
    && rm /tmp/packages-microsoft-prod.deb \
    && apt-get update && apt-get install -y dotnet-sdk-8.0 \
    && rm -rf /var/lib/apt/lists/*

# ================================================================
# Swift (official Linux toolchain)
# ================================================================
RUN apt-get update && apt-get install -y \
    binutils libc6-dev libcurl4-openssl-dev libedit2 \
    libgcc-13-dev libncurses-dev libpython3-dev libsqlite3-0 \
    libstdc++-13-dev libxml2-dev libz3-dev zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*
RUN SWIFT_URL="https://download.swift.org/swift-6.0.3-release/ubuntu2404/swift-6.0.3-RELEASE/swift-6.0.3-RELEASE-ubuntu24.04.tar.gz" \
    && curl -fsSL "$SWIFT_URL" | tar -C /usr/local --strip-components=2 -xz
ENV PATH="/usr/local/bin:${PATH}"

# ================================================================
# Rust toolchain (nightly — needed for edition 2024)
# ================================================================
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    --default-toolchain nightly-2025-12-19
ENV PATH="/root/.cargo/bin:${PATH}"

# ================================================================
# CodeQL CLI (Linux x86_64)
# ================================================================
# Place codeql-linux64.zip in vendor/ before building:
#   wget https://github.com/github/codeql-cli-binaries/releases/download/v2.25.1/codeql-linux64.zip \
#     -O vendor/codeql-linux64.zip
COPY vendor/codeql-linux64.zip /tmp/codeql-linux64.zip
RUN unzip -q /tmp/codeql-linux64.zip -d /opt \
    && rm /tmp/codeql-linux64.zip \
    && chmod +x /opt/codeql/codeql
ENV PATH="/opt/codeql:${PATH}"
# Verify CodeQL works
RUN codeql version

# ================================================================
# Workspace setup
# ================================================================
WORKDIR /workspace
COPY . .

# Build open-codeql in release mode
RUN cargo build --release \
    && cargo install --path crates/ocodeql --locked
# Verify ocodeql works
RUN ocodeql version

# Shell config for convenience
RUN echo 'alias ll="ls -la"' >> /root/.bashrc \
    && echo 'alias gs="git status"' >> /root/.bashrc \
    && echo 'export PS1="\[\033[1;32m\]open-codeql\[\033[0m\]:\[\033[1;34m\]\w\[\033[0m\]\$ "' >> /root/.bashrc

CMD ["/bin/bash"]
