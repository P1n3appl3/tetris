#!/usr/bin/env bash

dir=$(dirname "$0")

wasm-pack build --target web "$dir" "$@"

# TODO: entr, concat separate js file and copy assets into clean dir
echo -e "now run:\n\n    static-web-server -d $dir -p 8080\n"
echo "...and then open http://localhost:8080 in a browser"
