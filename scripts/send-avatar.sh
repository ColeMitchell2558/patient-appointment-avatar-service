#!/bin/sh
set -eu

avatar_path="${1:?usage: scripts/send-avatar.sh PATH_TO_AVATAR}"
avatar_bytes=$(od -An -vtu1 "$avatar_path" | tr -s ' ' ',' | tr -d '\n' | sed 's/^,//')

curl --fail-with-body http://127.0.0.1:8080/appointment-avatar \
  -H 'Content-Type: application/json' \
  --data "{\"user_id\":\"patient_42\",\"appointment_id\":\"appointment_2026_09_27\",\"appointment_state\":\"scheduled\",\"request_id\":\"appointment_2026_09_27_avatar_v1\",\"filename\":\"patient-avatar.png\",\"avatar\":[${avatar_bytes}]}"
