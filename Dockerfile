# ============================================================
# Builder Stage
# ============================================================
FROM rust:1.88-slim AS builder

ARG DEBIAN_FRONTEND=noninteractive
WORKDIR /app

# Build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential pkg-config wget curl ca-certificates git xz-utils unzip \
    libssl-dev libjpeg-dev libpng-dev libtiff-dev libwebp-dev libfreetype6-dev \
    liblcms2-dev libxml2-dev libbz2-dev liblzma-dev libz-dev libltdl-dev \
    ocl-icd-opencl-dev clang libclang-dev llvm-dev \
    && rm -rf /var/lib/apt/lists/*

# Rust extra components via rustup
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Cache Rust deps
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release || true
RUN rm -rf src

# Build minimal ImageMagick
RUN mkdir -p /cache/imagemagick && cd /cache/imagemagick \
    && latest=$(curl -s https://download.imagemagick.org/archive/ | \
        grep -o 'ImageMagick-[0-9\.]\+-[0-9]\+\.tar\.gz' | sort -V | tail -1) \
    && curl -sSL "https://download.imagemagick.org/archive/$latest" -o ImageMagick.tar.gz \
    && mkdir -p /app/imagemagick \
    && tar -xzf ImageMagick.tar.gz -C /app/imagemagick --strip-components=1
WORKDIR /app/imagemagick
RUN ./configure --disable-dependency-tracking --enable-shared --without-docs --without-magick-plus-plus \
    && make -j$(nproc) && make install && ldconfig

# Symlink MagickWand.pc
RUN pcfile=$(find /usr/local/lib/pkgconfig -name "MagickWand-*.pc" | head -n1) \
    && [ -n "$pcfile" ] && ln -sf "$pcfile" /usr/local/lib/pkgconfig/MagickWand.pc

# Detect libclang
RUN CLANG_DIR=$(dirname $(find /usr/lib /usr/lib64 /usr/local/lib -name "libclang.so*" | head -n1)) \
    && [ -n "$CLANG_DIR" ] && echo "export LIBCLANG_PATH=$CLANG_DIR" >> /etc/profile.d/libclang.sh \
    && echo "LIBCLANG_PATH=$CLANG_DIR" >> /etc/environment

# Build project release
WORKDIR /app
COPY . .
RUN cargo build --release


# ============================================================
# Runtime Stage
# ============================================================
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Minimal runtime deps (no build tools)
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 libjpeg62-turbo libpng16-16 libtiff6 libwebp7 libwebpdemux2 libwebpmux3 libfreetype6 \
    liblcms2-2 libxml2 libbz2-1.0 liblzma5 libltdl7 libzstd1 \
    ffmpeg ca-certificates chromium chromium-sandbox curl xz-utils wget \
    && rm -rf /var/lib/apt/lists/*

# Copy ImageMagick and libclang from builder
COPY --from=builder /usr/local /usr/local
COPY --from=builder /etc/profile.d/libclang.sh /etc/profile.d/libclang.sh
COPY --from=builder /etc/environment /etc/environment

ENV PATH="/usr/local/bin:${PATH}" \
    LD_LIBRARY_PATH="/usr/local/lib:${LD_LIBRARY_PATH}" \
    PKG_CONFIG_PATH=/usr/local/lib/pkgconfig

# Non-root user
RUN useradd -ms /bin/bash appuser \
    && mkdir -p /app/workingdir/downloads \
    && chown -R appuser:appuser /app/workingdir

# Copy release binary
COPY --from=builder /app/target/release/vmbfcoreapi-imgproc /usr/local/bin/vmbfcoreapi-imgproc

# Strip binaries and shared objects
RUN strip /usr/local/bin/vmbfcoreapi-imgproc || true \
    && find /usr/local/lib -type f -name "*.so*" -exec strip --strip-unneeded {} + || true \
    && rm -rf /usr/share/doc/* /usr/share/man/* /usr/share/locale/*

RUN chown appuser:appuser /usr/local/bin/vmbfcoreapi-imgproc \
    && chmod +x /usr/local/bin/vmbfcoreapi-imgproc

USER appuser

ENV RUST_LOG=info \
    APP_ADDRESS=0.0.0.0 \
    APP_PORT=8180

EXPOSE 8180
CMD ["vmbfcoreapi-imgproc"]
