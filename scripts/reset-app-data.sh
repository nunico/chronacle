#!/usr/bin/env bash
#
# Purge Chronacle's local app data so the app starts from a clean slate.
#
# Removes from the app data dir (~/Library/Application Support/dev.tea-driven.chronacle.desktop
# on macOS, $XDG_DATA_HOME or ~/.local/share/dev.tea-driven.chronacle.desktop on Linux):
#   - chronacle.db   (SurrealDB / RocksDB: campaigns, sources, chunks,
#                     settings incl. the encrypted API key, chat history)
#   - pdfs/          (ingested PDF blobs)
#
# The embedding model cache (embedding_model/) is KEPT by default because it
# is a large download; pass --all to remove it too.
#
# Also clears the Tauri WebView cache for dev.tea-driven.chronacle.desktop
# on macOS (localStorage etc.).
#
# Usage: scripts/reset-app-data.sh [--all] [--yes]
#   --all   also delete the cached embedding model
#   --yes   skip the confirmation prompt

set -euo pipefail

DATA_DIR_NAME="dev.tea-driven.chronacle.desktop"
BUNDLE_ID="dev.tea-driven.chronacle.desktop"

wipe_all=false
assume_yes=false
for arg in "$@"; do
    case "$arg" in
        --all) wipe_all=true ;;
        --yes|-y) assume_yes=true ;;
        *) echo "Unknown option: $arg" >&2; exit 2 ;;
    esac
done

case "$(uname -s)" in
    Darwin)
        data_dir="$HOME/Library/Application Support/$DATA_DIR_NAME"
        webview_dirs=(
            "$HOME/Library/Caches/$BUNDLE_ID"
            "$HOME/Library/WebKit/$BUNDLE_ID"
        )
        ;;
    Linux)
        data_dir="${XDG_DATA_HOME:-$HOME/.local/share}/$DATA_DIR_NAME"
        webview_dirs=(
            "${XDG_CACHE_HOME:-$HOME/.cache}/$BUNDLE_ID"
        )
        ;;
    *)
        echo "Unsupported platform: $(uname -s)" >&2
        exit 1
        ;;
esac

if pgrep -f "[Cc]hronacle" >/dev/null 2>&1; then
    echo "Chronacle appears to be running — quit it first (RocksDB holds a lock)." >&2
    exit 1
fi

targets=(
    "$data_dir/chronacle.db"
    "$data_dir/pdfs"
)
$wipe_all && targets+=("$data_dir/embedding_model")
targets+=("${webview_dirs[@]}")

existing=()
for t in "${targets[@]}"; do
    [ -e "$t" ] && existing+=("$t")
done

if [ ${#existing[@]} -eq 0 ]; then
    echo "Nothing to delete — app data is already clean."
    exit 0
fi

echo "Will delete:"
for t in "${existing[@]}"; do
    echo "  $t"
done

if ! $assume_yes; then
    printf "Proceed? [y/N] "
    read -r answer
    case "$answer" in
        y|Y|yes|YES) ;;
        *) echo "Aborted."; exit 1 ;;
    esac
fi

for t in "${existing[@]}"; do
    rm -rf "$t"
done

echo "Done. Chronacle will start fresh on next launch."
