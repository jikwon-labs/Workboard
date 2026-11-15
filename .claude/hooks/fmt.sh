#!/bin/bash
# PostToolUse: rustfmt a Rust file Claude just edited, but only when the
# committed version of that file was already rustfmt-clean (or the file is
# new). The whole workspace is formatted (scripts/wt verify checks it); the
# check keeps vendored code under third_party/ from being reformatted by a
# small edit. Formats only this file (stdin mode does not follow `mod x;`).
# Never blocks.
file=$(jq -r '.tool_input.file_path // empty')
case $file in *.rs) ;; *) exit 0 ;; esac
[ -f "$file" ] || exit 0
fmt=("$HOME/.cargo/bin/rustfmt" --edition 2021)
"${fmt[@]}" --version >/dev/null 2>&1 || exit 0

dir=$(dirname "$file")
root=$(git -C "$dir" rev-parse --show-toplevel 2>/dev/null) || exit 0
rel=${file#"$root"/}
tmp=$(mktemp) || exit 0
trap 'rm -f "$tmp"' EXIT
# (rustfmt --check always exits 0 on stdin, so compare the output instead)
if git -C "$root" cat-file -e "HEAD:$rel" 2>/dev/null; then
  git -C "$root" show "HEAD:$rel" | "${fmt[@]}" >"$tmp" 2>/dev/null || exit 0
  git -C "$root" show "HEAD:$rel" | cmp -s - "$tmp" || exit 0
fi

if "${fmt[@]}" <"$file" >"$tmp" 2>/dev/null && ! cmp -s "$tmp" "$file"; then
  cat "$tmp" >"$file"
fi
exit 0
