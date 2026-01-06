#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." >/dev/null 2>&1 && pwd)"

show_help() {
  cat <<'EOF'
Usage:
  ./scripts/setup-backend-systemd.sh [--user|--system] [service-name]

Installs a systemd unit for running the backend with `cargo run --bin server`.

Defaults:
  --system (writes to /etc/systemd/system, requires sudo)

Options:
  --user    Install a per-user unit at ~/.config/systemd/user (no sudo).
  --system  Install a system unit at /etc/systemd/system (requires sudo).

Environment:
  RUN_USER               User to run the service as (system units only).
  BACKEND_PORT           Force a specific backend port (otherwise auto-picked).
  INCLUDE_USER_PATH=1    Capture login-shell PATH and write into unit.
  USE_NVM_IF_MISSING=1   If npx missing, source nvm and select default Node.
  EXTRA_PATH             PATH prefix to force-add (e.g. Node bin dir).
  ENV_FILE               Optional EnvironmentFile to include (default: ./.env if present).
EOF
}

SCOPE="${SCOPE:-system}"
SERVICE_NAME_DEFAULT="vibe-kanban-backend"

if [[ $# -gt 0 ]]; then
  case "${1:-}" in
    -h|--help)
      show_help
      exit 0
      ;;
    --user)
      SCOPE="user"
      shift
      ;;
    --system)
      SCOPE="system"
      shift
      ;;
  esac
fi

SERVICE_NAME="${SERVICE_NAME:-${1:-${SERVICE_NAME_DEFAULT}}}"

SYSTEMD_DIR=""
SERVICE_PATH=""

RUN_USER="${RUN_USER:-${SUDO_USER:-}}"
if [[ "${SCOPE}" == "user" ]]; then
  RUN_USER="${RUN_USER:-${USER}}"
  if [[ "${RUN_USER}" != "${USER}" ]]; then
    echo "For --user installs, run as the target user (RUN_USER must match \$USER)." >&2
    exit 1
  fi
fi

if [[ "${SCOPE}" == "system" ]]; then
  # When running without sudo, SUDO_USER is empty; default RUN_USER to the
  # current user so we can still generate a correct unit, but --system still
  # requires sudo to write into /etc/systemd/system.
  RUN_USER="${RUN_USER:-${USER:-}}"
fi

# systemd services do not inherit your interactive shell PATH by default.
# INCLUDE_USER_PATH=1 captures the login-shell PATH for RUN_USER and writes it
# into the unit so tools like `npx` are available.
INCLUDE_USER_PATH="${INCLUDE_USER_PATH:-1}"

# If `npx` is not available in the login-shell PATH, try to source nvm and use
# the default Node version to populate PATH. This avoids requiring users to
# manually add EXTRA_PATH when they use nvm.
USE_NVM_IF_MISSING="${USE_NVM_IF_MISSING:-1}"

# Optional extra PATH prefix (e.g. for NVM Node bin dir).
EXTRA_PATH="${EXTRA_PATH:-}"

if [[ -z "${RUN_USER}" ]]; then
  echo "Unable to determine which user should run the service. Set RUN_USER." >&2
  exit 1
fi

if [[ "${SCOPE}" == "system" && ${EUID} -ne 0 ]]; then
  echo "Please run this script with sudo for --system installs (writes to /etc/systemd/system), or use --user." >&2
  exit 1
fi

if [[ "${SCOPE}" == "system" ]]; then
  SYSTEMD_DIR="${SYSTEMD_DIR:-/etc/systemd/system}"
  SERVICE_PATH="${SYSTEMD_DIR}/${SERVICE_NAME}.service"
  CARGO_BIN="${CARGO_BIN:-$(sudo -iu "${RUN_USER}" bash -lc 'command -v cargo' 2>/dev/null || true)}"
else
  # Per-user unit
  USER_HOME="$(eval echo "~${RUN_USER}")"
  SYSTEMD_DIR="${SYSTEMD_DIR:-${USER_HOME}/.config/systemd/user}"
  SERVICE_PATH="${SYSTEMD_DIR}/${SERVICE_NAME}.service"
  CARGO_BIN="${CARGO_BIN:-$(bash -lc 'command -v cargo' 2>/dev/null || true)}"
fi

if [[ -z "${CARGO_BIN}" ]]; then
  echo "Could not locate cargo for ${RUN_USER}. Install Rust or set CARGO_BIN." >&2
  exit 1
fi

USER_PATH=""
if [[ "${INCLUDE_USER_PATH}" == "1" ]]; then
  if [[ "${SCOPE}" == "system" ]]; then
    USER_PATH="$(sudo -iu "${RUN_USER}" bash -lc 'printf "%s" "$PATH"' 2>/dev/null || true)"
  else
    USER_PATH="$(bash -lc 'printf "%s" "$PATH"' 2>/dev/null || true)"
  fi
  if [[ -n "${EXTRA_PATH}" ]]; then
    USER_PATH="${EXTRA_PATH}:${USER_PATH}"
  fi
fi

if [[ "${INCLUDE_USER_PATH}" == "1" && "${USE_NVM_IF_MISSING}" == "1" ]]; then
  npx_check_cmd="PATH='${USER_PATH}' command -v npx >/dev/null 2>&1"
  if [[ "${SCOPE}" == "system" ]]; then
    npx_check_ok=0
    sudo -iu "${RUN_USER}" bash -lc "${npx_check_cmd}" && npx_check_ok=1 || true
  else
    npx_check_ok=0
    bash -lc "${npx_check_cmd}" && npx_check_ok=1 || true
  fi

  if [[ "${npx_check_ok}" == "0" ]]; then
    if [[ "${SCOPE}" == "system" ]]; then
      NVM_PATH="$(sudo -iu "${RUN_USER}" bash -lc '
        export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
        if [[ -s "${NVM_DIR}/nvm.sh" ]]; then
          # shellcheck disable=SC1090
          . "${NVM_DIR}/nvm.sh"
          nvm use --silent default >/dev/null 2>&1 || nvm use --silent node >/dev/null 2>&1 || true
          printf "%s" "$PATH"
        fi
      ' 2>/dev/null || true)"
    else
      NVM_PATH="$(bash -lc '
        export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
        if [[ -s "${NVM_DIR}/nvm.sh" ]]; then
          # shellcheck disable=SC1090
          . "${NVM_DIR}/nvm.sh"
          nvm use --silent default >/dev/null 2>&1 || nvm use --silent node >/dev/null 2>&1 || true
          printf "%s" "$PATH"
        fi
      ' 2>/dev/null || true)"
    fi

    if [[ -n "${NVM_PATH}" ]]; then
      USER_PATH="${NVM_PATH}"
      if [[ -n "${EXTRA_PATH}" ]]; then
        USER_PATH="${EXTRA_PATH}:${USER_PATH}"
      fi
    fi
  fi
fi

port_is_listening() {
  local port="$1"
  if command -v ss >/dev/null 2>&1; then
    ss -ltnH "( sport = :${port} )" 2>/dev/null | grep -q .
    return $?
  fi
  if command -v netstat >/dev/null 2>&1; then
    netstat -ltn 2>/dev/null | awk '{print $4}' | grep -E "[:.]${port}\$" -q
    return $?
  fi
  return 1
}

pick_dedicated_backend_port() {
  local base="$1"
  local tries="$2"
  local port="${base}"
  local i=0
  while [[ $i -lt $tries ]]; do
    if ! port_is_listening "${port}"; then
      printf "%s" "${port}"
      return 0
    fi
    port=$((port + 1))
    i=$((i + 1))
  done
  return 1
}

env_file_has_backend_port() {
  local f="$1"
  [[ -f "${f}" ]] || return 1
  rg -n "^[[:space:]]*(BACKEND_PORT|PORT)[[:space:]]*=" "${f}" >/dev/null 2>&1
}

ENV_FILE_DEFAULT="${REPO_ROOT}/.env"
ENV_FILE="${ENV_FILE:-${ENV_FILE_DEFAULT}}"
ENV_LINE=""
if [[ -n "${ENV_FILE}" ]]; then
  if [[ -f "${ENV_FILE}" ]]; then
    ENV_LINE="EnvironmentFile=${ENV_FILE}"
  else
    echo "ENV_FILE '${ENV_FILE}' not found; continuing without EnvironmentFile entry." >&2
  fi
fi

BACKEND_PORT_LINE=""
if [[ -n "${BACKEND_PORT:-}" ]]; then
  BACKEND_PORT_LINE="Environment=BACKEND_PORT=${BACKEND_PORT}"
else
  # Only inject BACKEND_PORT if the user isn't already providing it via ENV_FILE.
  need_port=1
  if [[ -n "${ENV_FILE:-}" && -f "${ENV_FILE}" ]] && env_file_has_backend_port "${ENV_FILE}"; then
    need_port=0
  fi
  if [[ "${need_port}" == "1" ]]; then
    # Prefer the backend default port (3000) when it's free, otherwise pick a
    # stable per-repo port in 3000–3999 (probing forward for a free slot).
    if ! port_is_listening 3000; then
      picked="3000"
    else
      hash_src="${REPO_ROOT}|${SERVICE_NAME}"
      hash_hex="$(printf "%s" "${hash_src}" | sha1sum | awk '{print $1}')"
      hash_dec="$((16#${hash_hex:0:6}))"
      base_port="$((3000 + (hash_dec % 1000)))"
      picked="$(pick_dedicated_backend_port "${base_port}" 200 || true)"
    fi
    if [[ -n "${picked}" ]]; then
      BACKEND_PORT_LINE="Environment=BACKEND_PORT=${picked}"
    else
      echo "Warning: could not find a free port near ${base_port}; not setting BACKEND_PORT." >&2
    fi
  fi
fi

PATH_LINE=""
if [[ -n "${USER_PATH}" ]]; then
  PATH_LINE="Environment=PATH=${USER_PATH}"
fi

USER_LINE=""
if [[ "${SCOPE}" == "system" ]]; then
  USER_LINE="User=${RUN_USER}"
fi

mkdir -p "${SYSTEMD_DIR}"

cat >"${SERVICE_PATH}" <<EOF
[Unit]
Description=Vibe Kanban backend dev server
After=network.target

[Service]
Type=simple
${USER_LINE}
WorkingDirectory=${REPO_ROOT}
ExecStart=${CARGO_BIN} run --bin server
Restart=on-failure
RestartSec=5
${BACKEND_PORT_LINE}
${PATH_LINE}
${ENV_LINE}

[Install]
WantedBy=multi-user.target
EOF

if [[ "${INCLUDE_USER_PATH}" == "1" ]]; then
  if [[ "${SCOPE}" == "system" ]]; then
    npx_found=0
    sudo -iu "${RUN_USER}" bash -lc "PATH='${USER_PATH}' command -v npx >/dev/null 2>&1" && npx_found=1 || true
  else
    npx_found=0
    bash -lc "PATH='${USER_PATH}' command -v npx >/dev/null 2>&1" && npx_found=1 || true
  fi

  if [[ "${npx_found}" == "0" ]]; then
    cat >&2 <<'EOW'
Warning: `npx` was not found for the service user.
  - If you use an npx-based executor (Codex/Claude/Gemini/etc), install Node.js or ensure npx is on PATH.
  - If you use nvm, make sure your login shell initializes it, or set EXTRA_PATH to your Node bin dir.
EOW
  fi
fi

if [[ "${SCOPE}" == "system" ]]; then
  # system unit
  systemctl daemon-reload
  systemctl enable --now "${SERVICE_NAME}"
else
  # user unit
  systemctl --user daemon-reload
  systemctl --user enable --now "${SERVICE_NAME}"
fi

cat <<EON
Systemd service installed:
  Service file: ${SERVICE_PATH}
  Service name: ${SERVICE_NAME}

Manage it with:
$(if [[ "${SCOPE}" == "system" ]]; then
  cat <<EOF2
  sudo systemctl status ${SERVICE_NAME}
  sudo journalctl -u ${SERVICE_NAME} -f
EOF2
else
  cat <<EOF2
  systemctl --user status ${SERVICE_NAME}
  journalctl --user -u ${SERVICE_NAME} -f
EOF2
fi)
EON
