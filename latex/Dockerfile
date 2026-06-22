# ── Stage 1: Build ────────────────────────────────────────────────────────────
FROM rust:1.79-slim AS builder

WORKDIR /app

# Cache dependencies separately from source for faster rebuilds.
COPY Cargo.toml Cargo.lock* ./
# Create a dummy main so `cargo build` can resolve deps without the real source.
RUN mkdir src && echo 'fn main(){}' > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Now copy the real source and rebuild only the binary.
COPY src ./src
# Touch main.rs so Cargo knows to recompile.
RUN touch src/main.rs
RUN cargo build --release

# ── Stage 2: Runtime ──────────────────────────────────────────────────────────
# texlive/texlive provides pdflatex, so --compile-pdf works out of the box.
FROM texlive/texlive:latest

# Copy the compiled binary from the build stage.
COPY --from=builder /app/target/release/docx2tex /usr/local/bin/docx2tex

# Work in /data so users can mount their files here:
#   docker run --rm -v $(pwd):/data docx2tex convert /data/my_doc.docx --output /data/out
WORKDIR /data

ENTRYPOINT ["docx2tex"]
