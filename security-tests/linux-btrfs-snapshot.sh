#!/usr/bin/env bash
# Synthetic disposable-runner experiment; not a production snapshot service.
set -euo pipefail

scratch="$(mktemp -d "${RUNNER_TEMP:-/tmp}/gild-btrfs-race.XXXXXXXX")"
image="$scratch/test.img"
mount_path="$scratch/mount"
writer_pid=''
pass_message=''
cleanup() {
  local status=$? cleanup_failed=0 safe_to_remove=1 probe_status attachments
  trap - EXIT
  set +e
  if [[ -n "$writer_pid" ]]; then
    sudo -u nobody touch "$mount_path/source/stop" 2>/dev/null || cleanup_failed=1
    # The writer is itself wrapped in timeout; wait is bounded even on failure.
    wait "$writer_pid" || cleanup_failed=1
  fi
  mountpoint -q "$mount_path"
  probe_status=$?
  if [[ "$probe_status" -eq 0 ]]; then
    if [[ -d "$mount_path/snapshot" ]]; then sudo btrfs subvolume delete "$mount_path/snapshot" >/dev/null 2>&1 || cleanup_failed=1; fi
    if [[ -d "$mount_path/source" ]]; then sudo btrfs subvolume delete "$mount_path/source" >/dev/null 2>&1 || cleanup_failed=1; fi
    sudo umount "$mount_path" || cleanup_failed=1
  elif [[ "$probe_status" -ne 32 && -e "$mount_path" ]]; then
    echo 'ERROR: initial mount probe failed' >&2
    cleanup_failed=1
    safe_to_remove=0
  fi
  mountpoint -q "$mount_path"
  probe_status=$?
  if [[ "$probe_status" -eq 0 ]]; then
    echo "ERROR: mount remains; preserving $scratch for diagnosis" >&2
    cleanup_failed=1
    safe_to_remove=0
  elif [[ "$probe_status" -ne 32 && -e "$mount_path" ]]; then
    echo 'ERROR: final mount probe failed' >&2
    cleanup_failed=1
    safe_to_remove=0
  fi
  if [[ -e "$image" ]]; then
    attachments="$(sudo losetup -j "$image" 2>/dev/null)"
    probe_status=$?
    if [[ "$probe_status" -ne 0 || -n "$attachments" ]]; then
      echo "ERROR: image detachment unverified; preserving $scratch" >&2
      cleanup_failed=1
      safe_to_remove=0
    fi
  fi
  if [[ "$safe_to_remove" -eq 1 ]]; then
    rm -rf -- "$scratch" || cleanup_failed=1
    if [[ -e "$scratch" ]]; then cleanup_failed=1; fi
  fi
  if [[ "$cleanup_failed" -ne 0 ]]; then
    echo 'ERROR: cleanup did not verify' >&2
    exit 1
  fi
  if [[ "$status" -eq 0 && -n "$pass_message" ]]; then
    echo "$pass_message"
    echo 'LIMIT: no assertion that a particular write overlapped the snapshot syscall or that a multi-file generation is transactional'
    echo 'LIMIT: deleting a snapshot path does not prove already-open readers fail and is not reval failure injection'
  fi
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

test "$(id -u)" -ne 0
test "$(sudo -u nobody id -u)" -ne "$(id -u)"
if sudo -u nobody sudo -n true 2>/dev/null; then
  echo 'ERROR: writer has sudo; privilege boundary invalid' >&2
  exit 1
fi
sudo apt-get update -qq
sudo apt-get install -y -qq btrfs-progs
chmod 755 "$scratch"
mkdir "$mount_path"
truncate -s 512M "$image"
chmod 600 "$image"
# Format only the exclusively-created image, never an allocated block device.
sudo mkfs.btrfs -q -f "$image"
# mount manages its own auto-clearing loop; cleanup verifies this exact image.
sudo mount -t btrfs -o loop "$image" "$mount_path"
sudo chmod 755 "$mount_path"
sudo btrfs subvolume create "$mount_path/source"
sudo chown nobody:nogroup "$mount_path/source"

if sudo -u nobody test -r "$image"; then
  echo 'ERROR: writer can read backing image' >&2
  exit 1
fi
if sudo -u nobody touch "$mount_path/unauthorized" 2>/dev/null; then
  echo 'ERROR: writer can modify snapshot parent' >&2
  exit 1
fi

source_dir="$mount_path/source"
sudo -u nobody timeout --signal=TERM --kill-after=2s 60s bash -c '
  set -euo pipefail
  d="$1"
  printf "A\n" > "$d/value"
  : > "$d/ready"
  n=0
  while [[ ! -e "$d/stop" ]]; do
    printf "B-%s\n" "$n" > "$d/next"
    mv -f "$d/next" "$d/value"
    n=$((n+1))
    printf "%s\n" "$n" > "$d/count.next"
    mv -f "$d/count.next" "$d/count"
  done
' _ "$source_dir" &
writer_pid=$!

for _ in {1..200}; do
  if [[ -s "$source_dir/count" && -e "$source_dir/ready" ]]; then break; fi
  kill -0 "$writer_pid" 2>/dev/null || { echo 'ERROR: writer exited before ready' >&2; exit 1; }
  sleep 0.05
done
[[ -s "$source_dir/count" && -e "$source_dir/ready" ]] || { echo 'ERROR: writer readiness timeout' >&2; exit 1; }
count_before="$(cat "$source_dir/count")"
[[ "$count_before" =~ ^[0-9]+$ ]] || { echo 'ERROR: invalid writer count' >&2; exit 1; }
sudo btrfs subvolume snapshot -r "$source_dir" "$mount_path/snapshot"
snapshot_hash="$(sha256sum "$mount_path/snapshot/value" | cut -d' ' -f1)"
count_at_capture="$(cat "$source_dir/count")"
[[ "$count_at_capture" =~ ^[0-9]+$ ]] || { echo 'ERROR: invalid post-capture writer count' >&2; exit 1; }
for _ in {1..200}; do
  count_after="$(cat "$source_dir/count" 2>/dev/null || true)"
  if [[ "$count_after" =~ ^[0-9]+$ ]] && (( count_after > count_at_capture )); then break; fi
  sleep 0.05
done
[[ "${count_after:-}" =~ ^[0-9]+$ ]] && (( count_after > count_at_capture )) || { echo 'ERROR: writer did not progress after snapshot' >&2; exit 1; }
test "$(sha256sum "$mount_path/snapshot/value" | cut -d' ' -f1)" = "$snapshot_hash"
test "$(stat -c '%U' "$mount_path/snapshot/value")" = nobody
if sudo -u nobody bash -c 'printf "tamper\n" >> "$1"' _ "$mount_path/snapshot/value" 2>/dev/null; then
  echo 'ERROR: writer changed snapshot file' >&2
  exit 1
fi
if sudo -u nobody btrfs property set -ts "$mount_path/snapshot" ro false 2>/dev/null; then
  echo 'ERROR: writer altered snapshot read-only property' >&2
  exit 1
fi
sudo -u nobody touch "$source_dir/stop"
wait "$writer_pid"
writer_pid=''
sudo btrfs subvolume delete "$mount_path/snapshot"
if [[ -e "$mount_path/snapshot/value" ]]; then
  echo 'ERROR: deleted snapshot remains readable by pathname' >&2
  exit 1
fi
pass_message="PASS: snapshot bytes remained stable while a separate non-root writer progressed after capture (before=$count_before capture=$count_at_capture after=$count_after)"
