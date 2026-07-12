#!/usr/bin/env bash
# Download all UD English treebanks used by Stanza's en_combined model.
# Usage: ./download_data.sh

set -euo pipefail

DATA_DIR="./data"
mkdir -p "$DATA_DIR"

TREEBANKS=(
    "UD_English-EWT:https://raw.githubusercontent.com/UniversalDependencies/UD_English-EWT/master"
    "UD_English-GUM:https://raw.githubusercontent.com/UniversalDependencies/UD_English-GUM/master"
    "UD_English-LinES:https://raw.githubusercontent.com/UniversalDependencies/UD_English-LinES/master"
    "UD_English-CHILDES:https://raw.githubusercontent.com/UniversalDependencies/UD_English-CHILDES/master"
)

for tb in "${TREEBANKS[@]}"; do
    name="${tb%%:*}"
    base="${tb#*:}"
    short=$(echo "$name" | sed 's/UD_English-//' | tr '[:upper:]' '[:lower:]')
    prefix="en_${short}"

    echo "=== $name ==="
    for split in train dev test; do
        filename="${prefix}-ud-${split}.conllu"
        url="${base}/${filename}"
        dest="$DATA_DIR/$filename"
        if [ -f "$dest" ]; then
            echo "  $filename already exists, skipping"
        else
            echo "  Downloading $filename..."
            curl -sSL -o "$dest" "$url"
        fi
    done
done

echo ""
echo "Done. Files in $DATA_DIR:"
ls -1 "$DATA_DIR"/*.conllu 2>/dev/null | sed 's/^/  /'
