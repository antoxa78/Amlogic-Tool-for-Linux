#!/bin/bash
# ADNL-based Amlogic flashing script
# Replaces the legacy update binary for ADNL-capable devices

set -o pipefail

msg() { echo "$@"; }
err() { echo "ERROR: $@" >&2; }

TOOL_PATH=$(cd "$(dirname "$0")" && pwd)
ADNL="$TOOL_PATH/adnl/adnl"

# Parse args
img=""
parts=""
soc=""
reset="y"
wipe=0
destroy=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --img=*) img="${1#*=}" ;;
        --parts=*) parts="${1#*=}" ;;
        --soc=*) soc="${1#*=}" ;;
        --reset=*) reset="${1#*=}" ;;
        --wipe) wipe=1 ;;
        --destroy) destroy=1 ;;
        *) err "Unknown arg: $1"; exit 1 ;;
    esac
    shift
done

if [[ -z "$img" ]]; then
    err "No image path specified"
    exit 1
fi

if [[ ! -x "$ADNL" ]]; then
    err "ADNL tool not found or not executable: $ADNL"
    exit 1
fi

# The ADNL command-line tool has no flash-erase command.  Do not silently
# ignore these options: the GUI exposes them, so fail with a clear message.
if [[ "$wipe" == 1 || "$destroy" == 1 ]]; then
    err "Erase Flash / Erase Bootloader are not supported by the ADNL engine; uncheck them and retry"
    exit 1
fi

TMP_DIR=""
cleanup() {
    if [[ -n "$TMP_DIR" && "$TMP_DIR" != "$img" ]]; then
        rm -rf "$TMP_DIR"
    fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP

# If img is a directory (unpacked), use it directly.
# If it's a file, it's a package that needs unpacking first.
if [[ -d "$img" ]]; then
    TMP_DIR="$img"
else
    case "$(uname -m)" in
        aarch64|arm64|armv7l|armv8l) TOOL_ARCH="linux-arm" ;;
        *) TOOL_ARCH="linux-x86" ;;
    esac

    PACKER="$TOOL_PATH/$TOOL_ARCH/aml_image_v2_packer"
    if [[ ! -x "$PACKER" && -x "$TOOL_PATH/$TOOL_ARCH/aml_image_v2_packer_khadas" ]]; then
        PACKER="$TOOL_PATH/$TOOL_ARCH/aml_image_v2_packer_khadas"
    fi
    if [[ ! -x "$PACKER" ]]; then
        err "Image packer not found: $PACKER"
        exit 1
    fi

    if ! TMP_DIR=$(mktemp -d /tmp/aml-adnl-XXXXXX); then
        err "Failed to create temporary directory"
        exit 1
    fi
    if ! "$PACKER" -d "$img" "$TMP_DIR" 2>/dev/null; then
        err "Failed to extract image"
        exit 1
    fi
fi

CFG="$TMP_DIR/image.cfg"
if [[ ! -f "$CFG" ]]; then
    err "image.cfg not found in $TMP_DIR"
    exit 1
fi

msg "Using ADNL tool: $ADNL"

# Check device
msg "Looking for ADNL device..."
DEVICES=$("$ADNL" devices 2>/dev/null)
DEV_SERIAL=$(echo "$DEVICES" | awk 'NF {print $1; exit}')
if [[ -z "$DEV_SERIAL" ]]; then
    err "No ADNL device found"
    exit 1
fi
msg "Found device: $DEV_SERIAL"

# Identify
msg "Identifying device..."
if ! IDENT=$("$ADNL" -s "$DEV_SERIAL" getvar identify 2>&1); then
    err "Identify failed"
    exit 1
fi
msg "$IDENT"

# Read partition list from image.cfg
declare -a PART_NAMES=()
declare -a PART_FILES=()
declare -a PART_TYPES=()

while IFS= read -r line; do
    case "$line" in
        *main_type=\"PARTITION\"*)
            name=$(echo "$line" | sed -n 's/.*sub_type="\([^"]*\)".*/\1/p')
            file=$(echo "$line" | sed -n 's/.*file="\([^"]*\)".*/\1/p')
            ftype=$(echo "$line" | sed -n 's/.*file_type="\([^"]*\)".*/\1/p')
            if [[ -n "$name" && -n "$file" ]]; then
                PART_NAMES+=("$name")
                PART_FILES+=("$file")
                PART_TYPES+=("${ftype:-normal}")
            fi
            ;;
    esac
done < <(grep 'main_type="PARTITION"' "$CFG")

msg "Found ${#PART_NAMES[@]} partitions in image"

# Determine which partitions to burn
BURN_ALL=false
BURN_LIST=()
if [[ "$parts" == "all" ]]; then
    BURN_ALL=true
elif [[ "$parts" == "none" ]]; then
    msg "No partitions selected (--parts=none)"
elif [[ -n "$parts" ]]; then
    IFS=',' read -ra BURN_LIST <<< "$parts"
fi

# Resolve the partitions that will actually be written, so progress and the
# final success message reflect only those files.
BURN_INDICES=()
for i in "${!PART_NAMES[@]}"; do
    pname="${PART_NAMES[$i]}"
    pfile="${PART_FILES[$i]}"
    pfilepath="$TMP_DIR/$pfile"

    if [[ "$BURN_ALL" == false ]]; then
        in_list=false
        for selected in "${BURN_LIST[@]}"; do
            if [[ "$selected" == "$pname" ]]; then
                in_list=true
                break
            fi
        done
        [[ "$in_list" == false ]] && continue
    fi

    if [[ ! -f "$pfilepath" ]]; then
        err "Partition file not found: $pfilepath"
        exit 1
    fi

    BURN_INDICES+=("$i")
done

total=${#BURN_INDICES[@]}
if [[ "$total" -eq 0 ]]; then
    err "No partitions to write"
    exit 1
fi

# Device is already in TPL mode (DDR initialized). Skip bl2_boot.
# Try writing partitions directly from TPL mode.
msg "Device is in TPL mode, attempting partition write directly..."
sleep 1

# Write partitions
msg "Writing partitions..."
count=0

for i in "${BURN_INDICES[@]}"; do
    pname="${PART_NAMES[$i]}"
    pfile="${PART_FILES[$i]}"
    ptype="${PART_TYPES[$i]}"
    pfilepath="$TMP_DIR/$pfile"

    msg "  Writing $pname ($pfile, $ptype)..."
    ARGS=(-s "$DEV_SERIAL" partition -p "$pname" -f "$pfilepath")
    if [[ "$ptype" == "sparse" ]]; then
        ARGS+=(-t sparse)
    fi
    if ! "$ADNL" "${ARGS[@]}" 2>&1; then
        err "Failed to write $pname"
        exit 1
    fi
    count=$((count + 1))
    pct=$((count * 100 / total))
    msg "$pct%"
    msg "  OK $pname"
done

msg "[OK] Partitions written successfully"

# Reboot
if [[ "$reset" == "y" ]]; then
    msg "Rebooting device..."
    "$ADNL" -s "$DEV_SERIAL" reboot 2>&1 || err "Reboot command failed"
fi

msg "[OK] All done"
exit 0
