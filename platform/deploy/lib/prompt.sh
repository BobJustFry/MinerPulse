#!/usr/bin/env bash
# Shared prompt helpers for MinerPulse deploy scripts.

prompt() {
  local var_name="$1"
  local message="$2"
  local default_value="${3:-}"
  local input
  if [[ -n "$default_value" ]]; then
    read -r -p "$message [$default_value]: " input
    input="${input:-$default_value}"
  else
    read -r -p "$message: " input
  fi
  printf -v "$var_name" '%s' "$input"
}

prompt_secret() {
  local var_name="$1"
  local message="$2"
  local input
  read -r -s -p "$message: " input
  echo
  printf -v "$var_name" '%s' "$input"
}

confirm() {
  local message="$1"
  local reply
  read -r -p "$message [y/N]: " reply
  [[ "$reply" =~ ^[Yy]$ ]]
}

gen_password() {
  openssl rand -base64 24 | tr -d '/+=' | head -c 24
}

one_line_pem() {
  awk '{printf "%s\\n", $0}' "$1"
}
