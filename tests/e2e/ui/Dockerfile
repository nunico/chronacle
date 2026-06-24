# Linux container to run the tauri-driver UI E2E from a macOS host (or any host).
#
# tauri-driver needs WebKitWebDriver, which exists only on Linux/Windows — this
# image provides the Linux side. Build context is the repo root:
#
#   docker build -f tests/e2e/ui/Dockerfile -t chronacle-e2e .
#   docker run --rm chronacle-e2e                 # runs all UI specs
#   docker run --rm chronacle-e2e <spec>          # e.g. settings-toggle.e2e.mjs
FROM rust:1-bookworm

# WebKitGTK + driver, GTK, headless X, and a session bus (WebKitGTK needs DBus).
RUN apt-get update && apt-get install -y --no-install-recommends \
      libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
      libayatana-appindicator3-dev webkit2gtk-driver \
      xvfb dbus dbus-x11 curl ca-certificates \
  && rm -rf /var/lib/apt/lists/*

# Node 22 (matches CI's lts/*) + the pnpm version pinned in package.json.
# pnpm 11 requires Node >= 22, and the repo's pnpm-workspace.yaml requires
# pnpm >= 10.
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
  && apt-get install -y --no-install-recommends nodejs \
  && npm install -g pnpm@11.5.1 \
  && rm -rf /var/lib/apt/lists/*

# WebDriver proxy (compiles; cached as its own layer).
RUN cargo install tauri-driver --locked

WORKDIR /app
COPY . .

RUN pnpm install --config.minimum-release-age=0
RUN pnpm build                                  # frontend → dist (embedded)

# libclang is needed by bindgen for surrealdb-librocksdb-sys (GitHub runners
# ship it; the slim rust image does not).
RUN apt-get update && apt-get install -y --no-install-recommends \
      clang libclang-dev \
  && rm -rf /var/lib/apt/lists/*
RUN cargo build --release --bin chronacle       # also downloads linux libpdfium.so

# Run under a private DBus session + virtual X display. Args (a spec glob)
# override the default of running every spec.
ENTRYPOINT ["tests/e2e/ui/run-in-container.sh"]
