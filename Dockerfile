# ============================================================
# Builder Stage
# ============================================================
FROM ubuntu:22.04 AS builder

ARG DEBIAN_FRONTEND=noninteractive
WORKDIR /app

# ------------------------------
# Install build dependencies
# ------------------------------
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential pkg-config wget curl ca-certificates git xz-utils jq unzip \
    libssl-dev libjpeg-dev libpng-dev libtiff-dev libwebp-dev libfreetype6-dev \
    liblcms2-dev libxml2-dev libbz2-dev liblzma-dev libz-dev libltdl-dev \
    ocl-icd-opencl-dev clang libclang-dev llvm-dev \
    && rm -rf /var/lib/apt/lists/*

# ------------------------------
# Install Rust
# ------------------------------
RUN mkdir -p /root/.cargo \
    && curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# ------------------------------
# Cache Rust deps before copying source
# ------------------------------
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release || true
RUN rm -rf src

# ------------------------------
# Download Google Chrome per architecture
# ------------------------------
RUN ARCH=$(dpkg --print-architecture) && \
    if [ "$ARCH" = "amd64" ]; then \
      CHROME_URL="https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb"; \
    elif [ "$ARCH" = "arm64" ]; then \
      CHROME_URL="https://dl.google.com/linux/direct/google-chrome-stable_current_arm64.deb"; \
    else \
      echo "Unsupported architecture: $ARCH" && exit 1; \
    fi && \
    curl -sSL "$CHROME_URL" -o /cache-chrome.deb

# ------------------------------
# Build ImageMagick from source
# ------------------------------
RUN mkdir -p /cache/imagemagick
WORKDIR /cache/imagemagick
RUN latest=$(curl -s https://download.imagemagick.org/archive/ | \
        grep -o 'ImageMagick-[0-9\.]\+-[0-9]\+\.tar\.gz' | sort -V | tail -1) && \
    curl -sSL "https://download.imagemagick.org/archive/$latest" -o ImageMagick.tar.gz && \
    mkdir -p /app/imagemagick && tar -xzf ImageMagick.tar.gz -C /app/imagemagick --strip-components=1
WORKDIR /app/imagemagick
RUN ./configure --disable-dependency-tracking --enable-shared && \
    make -j$(nproc) && make install && ldconfig

# Symlink MagickWand.pc automatically
RUN pcfile=$(find /usr/local/lib/pkgconfig -name "MagickWand-*.pc" | head -n1) \
    && if [ -n "$pcfile" ]; then ln -sf "$pcfile" /usr/local/lib/pkgconfig/MagickWand.pc; fi

# ------------------------------
# Autodetect libclang path
# ------------------------------
RUN CLANG_DIR=$(dirname $(find /usr/lib /usr/lib64 /usr/local/lib -name "libclang.so*" | head -n1)) \
    && if [ -z "$CLANG_DIR" ]; then echo "libclang.so not found" && exit 1; fi \
    && echo "export LIBCLANG_PATH=$CLANG_DIR" >> /etc/profile.d/libclang.sh \
    && echo "LIBCLANG_PATH=$CLANG_DIR" >> /etc/environment

# ------------------------------
# Copy project and build release
# ------------------------------
WORKDIR /app
COPY . .
RUN cargo build --release

# ============================================================
# Runtime Stage
# ============================================================
FROM ubuntu:22.04 AS runtime

ARG DEBIAN_FRONTEND=noninteractive
WORKDIR /app

# ------------------------------
# Install minimal runtime dependencies
# ------------------------------
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 libjpeg-turbo8 libpng16-16 libtiff5 libwebp7 libwebpdemux2 libwebpmux3 libfreetype6 \
    liblcms2-2 libxml2 libbz2-1.0 liblzma5 libltdl7 libzstd1 \
    ffmpeg exiftool ca-certificates curl xz-utils \
    && rm -rf /var/lib/apt/lists/*

# ------------------------------
# Install Google Chrome per architecture
# ------------------------------
COPY --from=builder /cache-chrome.deb /tmp/google-chrome.deb
RUN apt-get update && apt-get install -y ./tmp/google-chrome.deb || apt-get -f install -y \
    && rm -rf /var/lib/apt/lists/* /tmp/google-chrome.deb

# Ensure Chrome binary is accessible
RUN ln -sf /usr/bin/google-chrome /usr/bin/chromium-browser

# ------------------------------
# Copy ImageMagick libraries from builder
# ------------------------------
COPY --from=builder /usr/local /usr/local
COPY --from=builder /etc/profile.d/libclang.sh /etc/profile.d/libclang.sh
COPY --from=builder /etc/environment /etc/environment

ENV PATH="/usr/local/bin:${PATH}"
ENV LD_LIBRARY_PATH="/usr/local/lib:${LD_LIBRARY_PATH}"
ENV PKG_CONFIG_PATH=/usr/local/lib/pkgconfig

# ------------------------------
# Create non-root user
# ------------------------------
RUN useradd -ms /bin/bash appuser
RUN mkdir -p /app/workingdir/downloads && chown -R appuser:appuser /app/workingdir

# ------------------------------
# Copy release binary
# ------------------------------
COPY --from=builder /app/target/release/vmbfcoreapi-imgproc /usr/local/bin/vmbfcoreapi-imgproc

# Strip binaries and shared objects
RUN strip /usr/local/bin/vmbfcoreapi-imgproc || true && \
    find /usr/local/lib -type f -name "*.so*" -exec strip --strip-unneeded {} + || true && \
    rm -rf /usr/share/doc/* /usr/share/man/* /usr/share/locale/*

RUN chown appuser:appuser /usr/local/bin/vmbfcoreapi-imgproc \
    && chmod +x /usr/local/bin/vmbfcoreapi-imgproc

# ------------------------------
# Switch to non-root
# ------------------------------
USER appuser

ENV RUST_LOG=info \
    APP_ADDRESS=0.0.0.0 \
    APP_PORT=8180 \
    PUPPETEER_EXECUTABLE_PATH=/usr/bin/chromium-browser

EXPOSE 8180
WORKDIR /usr/local/bin

CMD ["./vmbfcoreapi-imgproc"]
