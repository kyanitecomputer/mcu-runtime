#!/bin/bash
# Generate a padded SPI flash image from a raw firmware binary.
#
# Usage: gen-flash-image.sh <input.bin> <output.bin> [size]
#
# The output is padded with zeros to the target size (default 1 MB).
# Suitable for programming via flashrom, openbmc-utils, or QEMU -drive.
set -euo pipefail

if [ $# -lt 2 ]; then
    echo "Usage: $0 <input.bin> <output.bin> [size_bytes]" >&2
    echo "  Default size: 1048576 (1 MB)" >&2
    exit 1
fi

input="$1"
output="$2"
size="${3:-1048576}"

if [ ! -f "$input" ]; then
    echo "Error: $input not found" >&2
    exit 1
fi

input_sz=$(stat -c%s "$input")
if [ "$input_sz" -gt "$size" ]; then
    echo "Error: input ($input_sz bytes) exceeds target size ($size bytes)" >&2
    exit 1
fi

# Create zero-filled output, then write the input at offset 0.
dd if=/dev/zero bs=1 count="$size" of="$output" 2>/dev/null
dd if="$input" of="$output" conv=notrunc 2>/dev/null

echo "Flash image: $output ($size bytes, firmware: $input_sz bytes, padding: $((size - input_sz)) bytes)"
