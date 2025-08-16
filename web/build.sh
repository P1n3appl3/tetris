#!/usr/bin/env bash
set -e

dir=$(dirname "$0")
output=$(pwd)/.site

mkdir -p "$output"

wasm-pack build --target web --out-dir "$output" --out-name tetris "$dir" "$@"
rm "$output"/{*.ts,package.json}

minify -o "$output" "$dir"/assets/{*.html,*.css,*.svg}

cp "$INTER" "$output/"
chmod +w "$output/$(basename "$INTER")"
cp "$dir/assets/icon.png" "$output/"

# TODO: entr (or minify -w)
echo -e "\nbundle size: $(du -sh .site)"
echo -e "now run:\n\n    static-web-server -d $output -p 8080\n"
echo "...and then open http://localhost:8080 in a browser"
