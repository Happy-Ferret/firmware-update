DISK="/dev/disk/by-id/usb-SanDisk_Cruzer_Blade_4C531001600616122302-0:0"

if [ ! -b "$DISK" ]
then
	echo "$DISK not present" >&2
	exit 1
fi

pv build/efi.img | sudo dd bs=1M of="$DISK" oflag=sync
