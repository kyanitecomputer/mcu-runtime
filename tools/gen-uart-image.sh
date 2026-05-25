#!/bin/bash
# Generate a UART boot image for AST10x0 from a raw firmware binary.
#
# Usage: gen-uart-image.sh <input.bin> <output.bin>
#
# Format: 4-byte little-endian payload size (aligned) + payload + zero padding
# The ROM bootloader reads the size header, then receives the payload over UART5.
set -euo pipefail

if [ $# -ne 2 ]; then
    echo "Usage: $0 <input.bin> <output.bin>" >&2
    exit 1
fi

input="$1"
output="$2"

if [ ! -f "$input" ]; then
    echo "Error: $input not found" >&2
    exit 1
fi

# Get input size and round up to 4-byte alignment.
input_sz=$(stat -c%s "$input")
aligned_sz=$(( (input_sz + 3) / 4 * 4 ))

# Write 4-byte little-endian size header.
printf '%08x' "$aligned_sz" | sed 's/\(..\)\(..\)\(..\)\(..\)/\4\3\2\1/' | xxd -r -p > "$output"

# Append the raw firmware.
dd if="$input" of="$output" bs=1 seek=4 2>/dev/null

# Pad with zeros to 4-byte alignment.
if [ "$aligned_sz" -gt "$input_sz" ]; then
    dd if=/dev/zero bs=1 count="$((aligned_sz - input_sz))" >> "$output" 2>/dev/null
fi

echo "UART boot image: $output (header: 4 bytes, payload: $input_sz bytes, aligned: $aligned_sz bytes, total: $((aligned_sz + 4)) bytes)"
