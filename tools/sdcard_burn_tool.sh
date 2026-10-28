#!/bin/bash

set -e -o pipefail

IMAGE=
DEVICE=
IMAGE_INSTALL_TYPE=

RED='\033[0;31m'
RESET='\033[m'

error_msg() {
	echo -e "$RED"ERROR:"$RESET" "$1"
}

usage() {
	echo -e "Usage:"
	echo -e "sdcard: $0 -d </dev/sdX> -i <path-to-image>"
}

require_cmd() {
	if ! command -v "$1" >/dev/null 2>&1; then
		error_msg "Missing required command: $1"
		exit 1
	fi
}

flash_sdcard() {
	pv -tpreb -- "$IMAGE" | sudo dd of="$DEVICE" conv=notrunc status=none
	echo "Sync..."
	sync
}

## Calculate time
## $1 - time in seconds
time_cal() {
	local minutes

	minutes=$(($1 / 60))

	echo "Time elapsed: $minutes minute(s)."
}

while getopts "d:i:Dh" flag; do
	case $flag in
		d)
		DEVICE="$OPTARG"
		;;
		i)
		IMAGE="$OPTARG"
		;;
		D)
		# Accepted for compatibility; no effect.
		;;
		h)
		usage
		exit 0
		;;
		*)
		usage
		exit 1
		;;
	esac
done

if [ -z "$IMAGE" ] || [ -z "$DEVICE" ]; then
	error_msg "Both -i <image> and -d <device> are required"
	usage
	exit 1
fi

if [ ! -f "$IMAGE" ]; then
	error_msg "Image '$IMAGE' doesn't exist!"
	usage
	exit 1
fi

require_cmd pv
require_cmd fdisk
require_cmd sudo

partition=$(fdisk -l "$IMAGE" | grep "Disklabel type" | awk -F ": " '{print $2}' || true)
if [ "$partition" == "dos" ]; then
	IMAGE_INSTALL_TYPE="SD-USB"
fi

start_time=$(date +%s)

if [ ! -b "$DEVICE" ]; then
	error_msg "'$DEVICE' is not a block device! Please make sure the device you choose is right."
	exit 1
fi

if [ "$IMAGE_INSTALL_TYPE" != "SD-USB" ]; then
	error_msg "Try to burn to SD/USB storage, but the image installation type is '$IMAGE_INSTALL_TYPE', please use a 'SD-USB' image!"
	exit 1
fi

echo "Burning image '$IMAGE' to SD/USB storage..."
flash_sdcard

end_time=$(date +%s)

time_cal "$((end_time - start_time))"

echo "Done!"
date
