# Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

export KSU_HAS_METAMODULE="true"
export KSU_METAMODULE="hybrid-mount"
BASE_DIR="/data/adb/hybrid-mount"
MANAGED_PARTITIONS="odm product system_ext vendor apex mi_ext my_bigball my_carrier my_company my_engineering my_heytap my_manifest my_preload my_product my_region my_reserve my_stock oem optics prism"
MODE_MARKERS="overlay magic"
NANO_MODE=false

detect_nano_mode() {
  local script_dir="${0%/*}"
  if [ "$script_dir" != "$0" ] && [ -f "$script_dir/.nano" ]; then
    return 0
  fi
  if [ -f "/data/adb/modules/hybrid_mount/.nano" ]; then
    return 0
  fi
  if [ -f "/data/adb/modules_update/hybrid_mount/.nano" ]; then
    return 0
  fi
  return 1
}

read_default_mount_mode() {
  local default_mode="magic"
  if [ -f "$BASE_DIR/config.toml" ]; then
    local config_default_mode
    config_default_mode=$(grep -E '^[[:space:]]*default_mode[[:space:]]*=' "$BASE_DIR/config.toml" | head -n 1 | sed 's/.*=\s*"\([^"]*\)".*/\1/')
    case "$config_default_mode" in
    overlay | magic)
      default_mode="$config_default_mode"
      ;;
    esac
  fi
  printf '%s\n' "$default_mode"
}

mode_label() {
  case "$1" in
  overlay)
    printf '%s\n' "OverlayFS"
    ;;
  magic)
    printf '%s\n' "Magic Mount"
    ;;
  *)
    printf '%s\n' "$1"
    ;;
  esac
}

wait_volume_key_or_timeout() {
  local timeout_seconds=$1
  local start_time=$(date +%s)
  while true; do
    local current_time=$(date +%s)
    if [ $((current_time - start_time)) -ge "$timeout_seconds" ]; then
      printf 'timeout\n'
      return 0
    fi
    local key_event=$(timeout 0.5 getevent -l 2>/dev/null)
    if echo "$key_event" | grep -q "KEY_VOLUMEUP"; then
      printf 'up\n'
      return 0
    elif echo "$key_event" | grep -q "KEY_VOLUMEDOWN"; then
      printf 'down\n'
      return 0
    fi
  done
}

module_has_managed_partitions() {
  for partition in $MANAGED_PARTITIONS; do
    if [ -d "$MODPATH/system/$partition" ] || [ -d "$MODPATH/$partition" ]; then
      return 0
    fi
  done
  return 1
}

current_mount_mode_marker() {
  for marker in $MODE_MARKERS; do
    if [ -f "$MODPATH/$marker" ]; then
      printf '%s\n' "$marker"
      return 0
    fi
  done
  return 1
}

clear_mount_mode_markers() {
  for marker in $MODE_MARKERS; do
    rm -f "$MODPATH/$marker"
  done
  rm -f "$MODPATH/kasumi"
}

write_mount_mode_marker() {
  local mode="$1"
  clear_mount_mode_markers
  : >"$MODPATH/$mode"
}

prompt_module_mount_mode() {
  local default_mode default_label chosen_mode existing_mode

  existing_mode="$(current_mount_mode_marker || true)"
  if [ -n "$existing_mode" ]; then
    ui_print "- Existing module mount mode marker: $(mode_label "$existing_mode")"
    write_mount_mode_marker "$existing_mode"
    return 0
  fi

  default_mode="$(read_default_mount_mode)"
  default_label="$(mode_label "$default_mode")"
  ui_print " "
  ui_print "========================================"
  ui_print "      Select Module Mount Mode         "
  ui_print "========================================"
  ui_print "  Volume Up (+): OverlayFS"
  ui_print "  Volume Down (-): Magic Mount"
  ui_print " "
  ui_print "  Defaulting to ${default_label} in 10 seconds"
  ui_print "========================================"

  chosen_mode="$default_mode"
  case "$(wait_volume_key_or_timeout 10)" in
  up)
    chosen_mode="overlay"
    ui_print "- Key Detected: Selected OverlayFS"
    ;;
  down)
    chosen_mode="magic"
    ui_print "- Key Detected: Selected Magic Mount"
    ;;
  timeout)
    ui_print "- Timeout: Selected ${default_label}"
    ;;
  esac

  write_mount_mode_marker "$chosen_mode"
  ui_print "- Marker written: $chosen_mode"
}

handle_partition() {
  echo 0 >/dev/null
  true
}

hybrid_handle_partition() {
  partition="$1"

  if [ ! -d "$MODPATH/system/$partition" ]; then
    return
  fi

  if [ -d "/$partition" ] && [ -L "/system/$partition" ]; then
    ln -sf "./system/$partition" "$MODPATH/$partition"
    ui_print "- handled /$partition"
  fi
}

cleanup_empty_system_dir() {
  if [ -d "$MODPATH/system" ] && [ -z "$(ls -A "$MODPATH/system" 2>/dev/null)" ]; then
    rmdir "$MODPATH/system" 2>/dev/null
    ui_print "- Removed empty /system directory (Skip system mount)"
  fi
}

mark_replace() {
  replace_target="$1"
  mkdir -p "$replace_target"
  setfattr -n trusted.overlay.opaque -v y "$replace_target"
}

ui_print "- Using Hybrid Mount metainstall"

install_module

if detect_nano_mode; then
  NANO_MODE=true
  ui_print "- Flavor: Nano (config-only)"
fi

for partition in $MANAGED_PARTITIONS; do
  hybrid_handle_partition "$partition"
done

cleanup_empty_system_dir

if [ "$NANO_MODE" = "true" ] && module_has_managed_partitions; then
  prompt_module_mount_mode
fi

ui_print "- Installation complete"

metamodule_hot_install() {

  # Hot install is currently only supported on KernelSU.
  if [ ! "$KSU" = true ]; then
    return
  fi

  if [ -z "$MODID" ]; then
    return
  fi

  MODDIR_INTERNAL="/data/adb/modules/$MODID"
  MODPATH_INTERNAL="/data/adb/modules_update/$MODID"

  if [ ! -d "$MODDIR_INTERNAL" ] || [ ! -d "$MODPATH_INTERNAL" ]; then
    return
  fi

  # hot install
  busybox rm -rf "$MODDIR_INTERNAL"
  busybox mv "$MODPATH_INTERNAL" "$MODDIR_INTERNAL"

  # run script requested, blocking, just fork it yourselves if you want it on background
  if [ ! -z "$MODULE_HOT_RUN_SCRIPT" ]; then
    [ -f "$MODDIR_INTERNAL/$MODULE_HOT_RUN_SCRIPT" ] && sh "$MODDIR_INTERNAL/$MODULE_HOT_RUN_SCRIPT"
  fi

  # we do this dance to satisfy kernelsu's ensure_file_exists
  mkdir -p "$MODPATH_INTERNAL"
  cat "$MODDIR_INTERNAL/module.prop" > "$MODPATH_INTERNAL/module.prop"

  ( sleep 3 ; 
    rm -rf "$MODDIR_INTERNAL/update" ; 
    rm -rf "$MODPATH_INTERNAL"
  ) & # fork in background

  ui_print "- Module hot install requested!"
  ui_print "- Refresh module page after installation!"
  ui_print "- No need to reboot!"
}

if [ "$MODULE_HOT_INSTALL_REQUEST" = true ]; then
  metamodule_hot_install
fi
