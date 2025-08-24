# ===== 1. Builder Stage =====
FROM rust:1.88-slim AS builder

# Set the working directory
WORKDIR /app

# Install dependencies needed for building (OpenSSL, pkg-config, etc.)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    build-essential \
 && rm -rf /var/lib/apt/lists/*

# Copy manifest files for dependency caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to allow dependency-only build
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Pre-build dependencies (so they are cached)
RUN cargo build --release && rm -rf src

# Clean /app before copying real source
RUN rm -rf /app/*

# Copy actual source code
COPY src ./src
COPY . .

# Build the actual release binary
RUN cargo build --release \
 && test -f target/release/vmbfcoreapi-imgproc

# ===== 2. Runtime Stage =====
FROM debian:bookworm-slim AS runtime

# Install dependencies needed for your app and Chrome
RUN apt-get update && apt-get install -y --no-install-recommends \
    wget \
    gnupg \
    ca-certificates \
    fonts-liberation \
    libasound2 \
    libatk-bridge2.0-0 \
    libatk1.0-0 \
    libc6 \
    libcairo2 \
    libcups2 \
    libdbus-1-3 \
    libexpat1 \
    libfontconfig1 \
    libgbm1 \
    libgcc-s1 \
    libglib2.0-0 \
    libgtk-3-0 \
    libnspr4 \
    libnss3 \
    libpango-1.0-0 \
    libx11-6 \
    libx11-xcb1 \
    libxcb1 \
    libxcomposite1 \
    libxdamage1 \
    libxext6 \
    libxfixes3 \
    libxrender1 \
    libxshmfence1 \
    libxtst6 \
    lsb-release \
    xdg-utils \
    --no-install-recommends

# Download and install Google Chrome .deb, fix deps if needed
RUN wget -q -O /tmp/chrome.deb https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb \
 && apt-get install -y /tmp/chrome.deb || apt-get install -f -y \
 && rm /tmp/chrome.deb \
 && rm -rf /var/lib/apt/lists/*




# Install only runtime dependencies (minimal size)
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 \
    ca-certificates \
 && rm -rf /var/lib/apt/lists/*


# Install dependencies and ExifTool
RUN apt-get update && apt-get install -y --no-install-recommends \
    libimage-exiftool-perl \
 && rm -rf /var/lib/apt/lists/*

# Verify installation
RUN exiftool -ver


# Create a non-root user for security
RUN useradd -ms /bin/bash appuser

# Create working directory with correct ownership
RUN mkdir -p /app/workingdir/downloads && chown -R appuser:appuser /app/workingdir

# Copy the compiled binary from builder
COPY --from=builder /app/target/release/vmbfcoreapi-imgproc /usr/local/bin/vmbfcoreapi-imgproc

# Change ownership to the non-root user
RUN chown appuser:appuser /usr/local/bin/vmbfcoreapi-imgproc


# Switch to non-root user
USER appuser

# Environment variables for Actix
ENV RUST_LOG=info \
    APP_ADDRESS=0.0.0.0 \
    APP_PORT=8180

# Expose the port your Actix server listens on
EXPOSE 8180

RUN chmod +x /usr/local/bin/vmbfcoreapi-imgproc

WORKDIR /usr/local/bin

# Command to run the binary
CMD ["./vmbfcoreapi-imgproc"]
