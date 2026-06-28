#!/usr/bin/env bash
# Load KEY=VALUE lines from a deploy config file into the current shell.

load_deploy_config() {
  local file="$1"
  if [[ ! -f "$file" ]]; then
    echo "Config not found: $file" >&2
    return 1
  fi
  while IFS= read -r line || [[ -n "$line" ]]; do
    line="${line%%#*}"
    line="${line#"${line%%[![:space:]]*}"}"
    line="${line%"${line##*[![:space:]]}"}"
    [[ -z "$line" ]] && continue
    if [[ "$line" =~ ^[A-Za-z_][A-Za-z0-9_]*= ]]; then
      # shellcheck disable=SC2163
      export "$line"
    fi
  done <"$file"
}
