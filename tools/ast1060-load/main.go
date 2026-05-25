// Package main implements ast1060-load: a standalone UART boot loader for
// AST1060 (and AST1030) firmware images over a serial port.
//
// The AST1060 ROM bootloader sends a ready indicator over UART5, then
// reads a 4-byte little-endian size header followed by that many bytes
// of payload.  This tool sends a pre-generated UART boot image (from
// gen-uart-image.sh) at a controlled rate.
//
// Usage:
//
//	ast1060-load -port /dev/ttyUSB0 uart_image.bin         # send image
//	ast1060-load -port /dev/ttyUSB0 -monitor uart_image.bin # send + read response
//	ast1060-load -port /dev/ttyUSB0 -probe                  # listen only (debug)
//	ast1060-load -port /dev/ttyUSB0 -probe -hex              # hex dump (debug)
//	ast1060-load -port /dev/ttyUSB0 -wait uart_image.bin     # wait for ROM ready
//	ast1060-load -port /dev/ttyUSB0 -raw firmware.bin        # send raw (no header)
package main

import (
	"encoding/binary"
	"encoding/hex"
	"flag"
	"fmt"
	"io"
	"log"
	"os"
	"sync"
	"time"

	"go.bug.st/serial"
)

func main() {
	log.SetFlags(log.Ltime | log.Lmicroseconds)

	portFlag := flag.String("port", "", "serial port device (e.g. /dev/ttyUSB0, COM3)")
	baudFlag := flag.Int("baud", 115200, "baud rate")
	chunkFlag := flag.Int("chunk", 128, "bytes per write")
	delayFlag := flag.Duration("delay", 0, "delay between chunks (0 = no delay)")
	timeoutFlag := flag.Duration("timeout", 0, "total transfer timeout (0 = no limit)")
	monitorFlag := flag.Bool("monitor", false, "show all target output during and after transfer (hex)")
	probeFlag := flag.Bool("probe", false, "listen only — print any bytes received (Ctrl-C to stop)")
	hexFlag := flag.Bool("hex", false, "hex dump mode for -probe")
	waitFlag := flag.Duration("wait", 0, "wait up to this duration for ROM ready byte before sending (e.g. -wait 30s)")
	rawFlag := flag.Bool("raw", false, "send file as-is without checking UART boot image header")
	verboseFlag := flag.Bool("v", false, "verbose: log every chunk write and RX event")
	flag.Parse()

	if *portFlag == "" {
		fmt.Fprintf(os.Stderr, "Usage: %s -port <device> [options] [uart_image.bin]\n\n", os.Args[0])
		flag.PrintDefaults()
		fmt.Fprintln(os.Stderr, "\nModes:")
		fmt.Fprintln(os.Stderr, "  (default)    Send UART boot image to target")
		fmt.Fprintln(os.Stderr, "  -raw         Send raw binary without header validation")
		fmt.Fprintln(os.Stderr, "  -probe       Listen only, print target output")
		fmt.Fprintln(os.Stderr, "  -monitor     Send image, print all RX as hex during+after")
		fmt.Fprintln(os.Stderr, "  -wait 30s    Wait for ROM ready byte, then send")
		fmt.Fprintln(os.Stderr, "\nDebugging workflow:")
		fmt.Fprintln(os.Stderr, "  1. Power off board")
		fmt.Fprintln(os.Stderr, "  2. Run: ast1060-load -port /dev/ttyUSB0 -probe -hex")
		fmt.Fprintln(os.Stderr, "  3. Power on board — observe ROM output")
		fmt.Fprintln(os.Stderr, "  4. Run: ast1060-load -port /dev/ttyUSB0 -wait 30s -monitor image.bin")
		fmt.Fprintln(os.Stderr, "  5. Power-cycle board — tool waits, sends, shows response")
		os.Exit(1)
	}

	// Open serial port.
	mode := &serial.Mode{
		BaudRate: *baudFlag,
		DataBits: 8,
		Parity:   serial.NoParity,
		StopBits: serial.OneStopBit,
	}
	port, err := serial.Open(*portFlag, mode)
	if err != nil {
		log.Fatalf("open %s: %v", *portFlag, err)
	}
	defer port.Close()

	fmt.Fprintf(os.Stderr, "Connected to %s @ %d baud 8N1\n", *portFlag, *baudFlag)

	// Probe mode.
	if *probeFlag {
		probeLoop(port, *hexFlag)
		return
	}

	if flag.NArg() != 1 {
		log.Fatal("no image file specified (use -probe for listen-only mode)")
	}

	imagePath := flag.Arg(0)
	data, err := os.ReadFile(imagePath)
	if err != nil {
		log.Fatalf("read %s: %v", imagePath, err)
	}

	if !*rawFlag {
		if len(data) < 8 {
			log.Fatalf("image too small (%d bytes): need at least 4-byte header + payload", len(data))
		}
		payloadSize := binary.LittleEndian.Uint32(data[:4])
		expectedTotal := int(payloadSize) + 4
		fmt.Fprintf(os.Stderr, "UART boot image: %s\n", imagePath)
		fmt.Fprintf(os.Stderr, "  File size:     %d bytes\n", len(data))
		fmt.Fprintf(os.Stderr, "  Header (LE32): %d (0x%08x) bytes payload\n", payloadSize, payloadSize)
		fmt.Fprintf(os.Stderr, "  Expected total: %d bytes (header + payload)\n", expectedTotal)
		fmt.Fprintf(os.Stderr, "  Header hex:    %s\n", hex.EncodeToString(data[:4]))
		if payloadSize > 1024*1024 {
			log.Fatalf("payload size %d (0x%x) looks wrong — did you pass the raw .bin instead of the UART boot image?\n"+
				"The raw binary's first 4 bytes are the initial SP, not a size header.\n"+
				"Run gen-uart-image.sh first, or use -raw to skip header validation.", payloadSize, payloadSize)
		}
		if len(data) != expectedTotal {
			fmt.Fprintf(os.Stderr, "  WARNING: file size %d != expected %d\n", len(data), expectedTotal)
		}
	} else {
		fmt.Fprintf(os.Stderr, "Raw binary: %s (%d bytes)\n", imagePath, len(data))
	}

	// Start background reader — always active in monitor mode, captures
	// everything the target sends including ROM responses.
	var rxMu sync.Mutex
	var rxBuf []byte
	rxStop := make(chan struct{})
	rxDone := make(chan struct{})

	startRxReader := func() {
		go func() {
			defer close(rxDone)
			buf := make([]byte, 4096)
			port.SetReadTimeout(100 * time.Millisecond)
			for {
				select {
				case <-rxStop:
					return
				default:
				}
				n, err := port.Read(buf)
				if n > 0 {
					rxMu.Lock()
					rxBuf = append(rxBuf, buf[:n]...)
					rxMu.Unlock()
					if *monitorFlag || *verboseFlag {
						fmt.Fprintf(os.Stderr, "[RX %d bytes] %s\n", n, hex.EncodeToString(buf[:n]))
					}
				}
				if err == io.EOF {
					return
				}
			}
		}()
	}

	if *monitorFlag || *waitFlag > 0 || *verboseFlag {
		startRxReader()
	}

	// Wait for ROM ready signal if requested.
	// The AST1060 ROM sends 0x55 ('U') when it enters UART boot mode.
	// Some boards emit stray bytes (e.g. 0x00) during reset before 'U'.
	// We must wait specifically for 0x55, not just any data.
	if *waitFlag > 0 {
		fmt.Fprintf(os.Stderr, "Waiting up to %s for ROM 'U' (0x55) byte (power-cycle/reset board now)...\n", *waitFlag)
		deadline := time.After(*waitFlag)
		gotReady := false
		for !gotReady {
			select {
			case <-deadline:
				rxMu.Lock()
				buf := append([]byte(nil), rxBuf...)
				rxMu.Unlock()
				if len(buf) > 0 {
					fmt.Fprintf(os.Stderr, "Wait timed out. Received %d bytes but no 'U' (0x55): %s\n",
						len(buf), hex.EncodeToString(buf))
					fmt.Fprintf(os.Stderr, "Board may not be in UART boot mode. Check OTP/strap config.\n")
				}
				log.Fatal("wait timed out: ROM ready byte 'U' (0x55) not received")
			default:
				rxMu.Lock()
				found := false
				for _, b := range rxBuf {
					if b == 0x55 {
						found = true
						break
					}
				}
				rxMu.Unlock()
				if found {
					fmt.Fprintf(os.Stderr, "ROM ready 'U' (0x55) detected. Sending image...\n")
					gotReady = true
				} else {
					time.Sleep(10 * time.Millisecond)
				}
			}
		}
		// Small delay after ROM ready to let it settle.
		time.Sleep(50 * time.Millisecond)
	} else {
		// Drain stale RX data.
		port.SetReadTimeout(100 * time.Millisecond)
		drain := make([]byte, 4096)
		for {
			n, _ := port.Read(drain)
			if n == 0 {
				break
			}
			if *verboseFlag {
				fmt.Fprintf(os.Stderr, "[DRAIN %d bytes] %s\n", n, hex.EncodeToString(drain[:n]))
			}
		}
	}

	// Send the image in chunks.
	var txDeadline <-chan time.Time
	if *timeoutFlag > 0 {
		txDeadline = time.After(*timeoutFlag)
	}

	sent := 0
	total := len(data)
	chunk := *chunkFlag
	start := time.Now()

	for sent < total {
		select {
		case <-txDeadline:
			log.Fatalf("transfer timed out after %s (%d/%d bytes sent)", *timeoutFlag, sent, total)
		default:
		}

		end := sent + chunk
		if end > total {
			end = total
		}

		n, err := port.Write(data[sent:end])
		if err != nil {
			log.Fatalf("write error at byte %d: %v", sent, err)
		}
		if *verboseFlag {
			fmt.Fprintf(os.Stderr, "[TX %d bytes @ offset %d]\n", n, sent)
		}
		sent += n

		pct := float64(sent) / float64(total) * 100
		elapsed := time.Since(start).Seconds()
		rate := float64(sent) / elapsed
		fmt.Fprintf(os.Stderr, "\rSending: %d/%d bytes (%.1f%%) %.0f B/s   ", sent, total, pct, rate)

		if *delayFlag > 0 {
			time.Sleep(*delayFlag)
		}
	}

	elapsed := time.Since(start)
	fmt.Fprintf(os.Stderr, "\nTransfer complete: %d bytes in %s (%.0f B/s)\n",
		sent, elapsed.Round(time.Millisecond), float64(sent)/elapsed.Seconds())

	// Post-transfer: wait for firmware response.
	if *monitorFlag {
		fmt.Fprintf(os.Stderr, "Waiting for target response (10s)...\n")
		time.Sleep(10 * time.Second)
		close(rxStop)
		<-rxDone

		rxMu.Lock()
		defer rxMu.Unlock()
		if len(rxBuf) > 0 {
			fmt.Fprintf(os.Stderr, "--- all RX data (%d bytes) ---\n", len(rxBuf))
			fmt.Fprint(os.Stderr, hex.Dump(rxBuf))
			// Also print as text (non-printable replaced with dots).
			fmt.Fprintf(os.Stderr, "--- as text ---\n")
			for _, b := range rxBuf {
				if b >= 0x20 && b < 0x7f {
					fmt.Fprintf(os.Stderr, "%c", b)
				} else if b == '\r' || b == '\n' {
					fmt.Fprintf(os.Stderr, "%c", b)
				} else {
					fmt.Fprintf(os.Stderr, ".")
				}
			}
			fmt.Fprintln(os.Stderr)
		} else {
			fmt.Fprintf(os.Stderr, "--- no response from target ---\n")
		}
	} else if *waitFlag > 0 || *verboseFlag {
		// Clean up background reader if it was started for -wait.
		close(rxStop)
		<-rxDone
	}
}

// probeLoop reads from the serial port and prints to stdout until interrupted.
func probeLoop(port serial.Port, hexDump bool) {
	port.SetReadTimeout(1 * time.Second)
	buf := make([]byte, 4096)
	fmt.Fprintf(os.Stderr, "Listening (Ctrl-C to stop)...\n")
	for {
		n, err := port.Read(buf)
		if n > 0 {
			ts := time.Now().Format("15:04:05.000")
			if hexDump {
				fmt.Fprintf(os.Stderr, "[%s RX %d bytes]\n", ts, n)
				fmt.Print(hex.Dump(buf[:n]))
			} else {
				os.Stdout.Write(buf[:n])
			}
		}
		if err == io.EOF {
			return
		}
	}
}
