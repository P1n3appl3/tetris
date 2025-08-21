#!/usr/bin/env bash
set -e

dir=$(dirname "$0")
output=$(pwd)/.site

mkdir -p "$output"

wasm-pack build --target web --out-dir "$output" --out-name tetris "$dir" "$@"
rm -f "$output"/{*.ts,package.json}

minify -o "$output" "$dir"/assets/{*.html,*.css,*.svg} "$output/tetris.js"
cp "$dir/assets/icon.png" "$output/"

if [ "${INTER##*.}" = "ttf" ]; then
    cp "$INTER" "$output/Inter.ttf"
    woff2_compress "$output/Inter.ttf"
elif [ "${INTER##*.}" = "woff2" ]; then
    cp "$INTER" "$output/Inter.woff2"
else
    echo "Font not recognized:"
    echo "set $$INTER to the path of a file in either the ttf or woff2 format"
fi
chmod +w "$output/Inter.woff2"

# TODO: entr (or minify -w)
echo -e "\nbundle size: $(du -sh .site)"
echo -e "now run:\n\n    static-web-server -d $output -p 8080\n"
echo "...and then open http://localhost:8080 in a browser"
