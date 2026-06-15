// Package main implements aspeed-uart-load: a standalone UART boot loader
// for ASPEED MCU firmware images over a serial port.
//
// Supports:
//   - AST1060/AST1030: ROM sends 'U' (0x55) when ready for UART boot
//   - AST2700 BootMCU stage 1: ROM sends "U1" (0x55 0x31) — ready for Caliptra FW
//   - AST2700 BootMCU stage 2: ROM sends "U2" (0x55 0x32) — Caliptra done, ready for app
//
// AST2700 two-stage protocol:
//
//	Power-on → ROM prints status → "Fc U1 " (stage-1 ready) → receive caliptra-fw.bin →
//	ROM programs Caliptra → "Ff U2" (stage-2 ready) → receive app binary → execute
//
// Single-command two-stage usage (recommended for AST2700):
//
//	aspeed-uart-load -port /dev/ttyUSB0 -soc ast2700 -wait 30s \
//	    -stage2 app_uart.bin -monitor -monitor-wait 30s caliptra-fw.bin
//
// Manual single-stage usage:
//
//	aspeed-uart-load -port /dev/ttyUSB0 uart_image.bin                         # AST1060
//	aspeed-uart-load -port /dev/ttyUSB0 -soc ast2700 -wait 30s caliptra-fw.bin # AST2700 stage-1 only
//	aspeed-uart-load -port /dev/ttyUSB0 -soc ast2700-app -wait 60s app.bin     # AST2700 stage-2 only
//	aspeed-uart-load -port /dev/ttyUSB0 -probe                                 # listen only
//	aspeed-uart-load -port /dev/ttyUSB0 -probe -hex                            # hex dump
package main

import (
	"bytes"
	"encoding/binary"
	"encoding/hex"
	"flag"
	"fmt"
	"io"
	"log"
	"os"
	"strings"
	"sync"
	"time"

	"go.bug.st/serial"
)

// socConfig describes the ROM boot protocol for a given SoC.
type socConfig struct {
	name        string
	readyBytes  []byte // byte sequence the ROM sends when ready
	readyName   string // human-readable description
	defaultBaud int
}

var socConfigs = map[string]socConfig{
	"ast1060": {
		name:        "AST1060",
		readyBytes:  []byte{0x55},
		readyName:   "'U' (0x55)",
		defaultBaud: 115200,
	},
	"ast1030": {
		name:        "AST1030",
		readyBytes:  []byte{0x55},
		readyName:   "'U' (0x55)",
		defaultBaud: 115200,
	},
	// AST2700 BootMCU stage 1: ROM just entered UART boot mode.
	// "Fc U1 0000" line contains 0x55 0x31.
	"ast2700": {
		name:        "AST2700 BootMCU (stage-1)",
		readyBytes:  []byte{0x55, 0x31},
		readyName:   "\"U1\" (0x55 0x31)",
		defaultBaud: 115200,
	},
	// AST2700 BootMCU stage 2: ROM finished Caliptra init, ready for app firmware.
	// "Ff U2" line ends with 0x55 0x32.
	"ast2700-app": {
		name:        "AST2700 BootMCU (stage-2 / app)",
		readyBytes:  []byte{0x55, 0x32},
		readyName:   "\"U2\" (0x55 0x32)",
		defaultBaud: 115200,
	},
}

func socNames() string {
	names := make([]string, 0, len(socConfigs))
	for k := range socConfigs {
		names = append(names, k)
	}
	return strings.Join(names, ", ")
}

func main() {
	log.SetFlags(log.Ltime | log.Lmicroseconds)

	portFlag := flag.String("port", "", "serial port device (e.g. /dev/ttyUSB0, COM3)")
	socFlag := flag.String("soc", "ast1060", "target SoC: "+socNames())
	baudFlag := flag.Int("baud", 0, "baud rate (0 = SoC default)")
	chunkFlag := flag.Int("chunk", 128, "bytes per write")
	delayFlag := flag.Duration("delay", 0, "delay between chunks (0 = no delay)")
	timeoutFlag := flag.Duration("timeout", 0, "total transfer timeout (0 = no limit)")
	monitorFlag := flag.Bool("monitor", false, "show all target output during and after transfer")
	monitorWaitFlag := flag.Duration("monitor-wait", 10*time.Second, "how long to listen after transfer in -monitor mode")
	probeFlag := flag.Bool("probe", false, "listen only — print any bytes received (Ctrl-C to stop)")
	hexFlag := flag.Bool("hex", false, "hex dump mode for -probe")
	waitFlag := flag.Duration("wait", 0, "wait up to this duration for ROM ready byte(s) before sending (e.g. -wait 30s)")
	stage2Flag := flag.String("stage2", "", "AST2700: application binary to send after stage-1 (waits for \"U2\" ready signal)")
	stage2WaitFlag := flag.Duration("stage2-wait", 60*time.Second, "AST2700: timeout waiting for \"U2\" signal after stage-1")
	rawFlag := flag.Bool("raw", false, "send file as-is without checking UART boot image header")
	verboseFlag := flag.Bool("v", false, "verbose: log every chunk write and RX event")
	flag.Parse()

	soc, ok := socConfigs[strings.ToLower(*socFlag)]
	if !ok {
		log.Fatalf("unknown SoC %q — supported: %s", *socFlag, socNames())
	}

	if *portFlag == "" {
		fmt.Fprintf(os.Stderr, "Usage: %s -port <device> [options] [uart_image.bin]\n\n", os.Args[0])
		flag.PrintDefaults()
		fmt.Fprintln(os.Stderr, "\nSoC targets:")
		fmt.Fprintln(os.Stderr, "  ast1060 (default)  AST1060/AST1030, ROM ready = 'U' (0x55)")
		fmt.Fprintln(os.Stderr, "  ast2700            AST2700 BootMCU stage-1, ROM ready = \"U1\" (0x55 0x31)")
		fmt.Fprintln(os.Stderr, "                     Appears as \"Fc U1 0000\" in ROM status output")
		fmt.Fprintln(os.Stderr, "  ast2700-app        AST2700 BootMCU stage-2, ROM ready = \"U2\" (0x55 0x32)")
		fmt.Fprintln(os.Stderr, "                     Appears as \"Ff U2\" after Caliptra init in ROM output")
		fmt.Fprintln(os.Stderr, "\nAST2700 two-stage boot (single command):")
		fmt.Fprintln(os.Stderr, "  aspeed-uart-load -port /dev/ttyUSB0 -soc ast2700 -wait 30s \\")
		fmt.Fprintln(os.Stderr, "      -stage2 app_uart.bin -monitor -monitor-wait 30s caliptra-fw.bin")
		fmt.Fprintln(os.Stderr, "\nAST2700 two-stage boot (manual, two commands):")
		fmt.Fprintln(os.Stderr, "  aspeed-uart-load -port /dev/ttyUSB0 -soc ast2700 -wait 30s caliptra-fw.bin")
		fmt.Fprintln(os.Stderr, "  aspeed-uart-load -port /dev/ttyUSB0 -soc ast2700-app -wait 60s -monitor -monitor-wait 30s app_uart.bin")
		fmt.Fprintln(os.Stderr, "\nModes:")
		fmt.Fprintln(os.Stderr, "  (default)    Send UART boot image to target")
		fmt.Fprintln(os.Stderr, "  -raw         Send raw binary without header validation")
		fmt.Fprintln(os.Stderr, "  -probe       Listen only, print target output")
		fmt.Fprintln(os.Stderr, "  -monitor     Send image, print all RX as text during+after")
		os.Exit(1)
	}

	baud := *baudFlag
	if baud == 0 {
		baud = soc.defaultBaud
	}

	// Open serial port.
	mode := &serial.Mode{
		BaudRate: baud,
		DataBits: 8,
		Parity:   serial.NoParity,
		StopBits: serial.OneStopBit,
	}
	port, err := serial.Open(*portFlag, mode)
	if err != nil {
		log.Fatalf("open %s: %v", *portFlag, err)
	}
	defer port.Close()

	fmt.Fprintf(os.Stderr, "Connected to %s @ %d baud 8N1 (SoC: %s)\n", *portFlag, baud, soc.name)

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

	// Background RX reader — accumulates all bytes from the target.
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
					if *monitorFlag {
						os.Stdout.Write(buf[:n])
					}
					if *verboseFlag {
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

	// Wait for ROM ready signal.
	if *waitFlag > 0 {
		fmt.Fprintf(os.Stderr, "Waiting up to %s for %s ROM ready %s (power-cycle/reset board now)...\n",
			*waitFlag, soc.name, soc.readyName)
		deadline := time.After(*waitFlag)
		gotReady := false
		for !gotReady {
			select {
			case <-deadline:
				rxMu.Lock()
				buf := append([]byte(nil), rxBuf...)
				rxMu.Unlock()
				if len(buf) > 0 {
					fmt.Fprintf(os.Stderr, "Wait timed out. Received %d bytes but no %s:\n", len(buf), soc.readyName)
					fmt.Fprint(os.Stderr, hex.Dump(buf))
					printAsText(os.Stderr, buf)
					fmt.Fprintf(os.Stderr, "Board may not be in UART boot mode. Check OTP/strap config.\n")
				}
				log.Fatalf("wait timed out: ROM ready %s not received", soc.readyName)
			default:
				rxMu.Lock()
				found := bytes.Contains(rxBuf, soc.readyBytes)
				rxMu.Unlock()
				if found {
					fmt.Fprintf(os.Stderr, "ROM ready %s detected. Sending image...\n", soc.readyName)
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

	// ── AST2700 stage-2 ───────────────────────────────────────────────────
	// If -stage2 is given, wait for "U2" (0x55 0x32) then send the app binary.
	if *stage2Flag != "" {
		stage2Data, err := os.ReadFile(*stage2Flag)
		if err != nil {
			log.Fatalf("read stage2 %s: %v", *stage2Flag, err)
		}
		if !*rawFlag {
			if len(stage2Data) < 8 {
				log.Fatalf("stage2 image too small (%d bytes)", len(stage2Data))
			}
			payloadSize := binary.LittleEndian.Uint32(stage2Data[:4])
			fmt.Fprintf(os.Stderr, "Stage-2 image: %s (%d bytes payload)\n", *stage2Flag, payloadSize)
		} else {
			fmt.Fprintf(os.Stderr, "Stage-2 raw: %s (%d bytes)\n", *stage2Flag, len(stage2Data))
		}

		u2Ready := socConfigs["ast2700-app"].readyBytes
		fmt.Fprintf(os.Stderr, "Waiting up to %s for stage-2 ready \"U2\" (0x55 0x32)...\n", *stage2WaitFlag)
		deadline2 := time.After(*stage2WaitFlag)
		gotU2 := false
		for !gotU2 {
			select {
			case <-deadline2:
				rxMu.Lock()
				buf := append([]byte(nil), rxBuf...)
				rxMu.Unlock()
				fmt.Fprintf(os.Stderr, "Stage-2 wait timed out. Total RX so far:\n")
				printAsText(os.Stderr, buf)
				log.Fatal("timed out waiting for U2 stage-2 ready signal")
			default:
				rxMu.Lock()
				found := bytes.Contains(rxBuf, u2Ready)
				rxMu.Unlock()
				if found {
					fmt.Fprintf(os.Stderr, "Stage-2 ready \"U2\" detected. Sending application...\n")
					gotU2 = true
				} else {
					time.Sleep(10 * time.Millisecond)
				}
			}
		}
		time.Sleep(50 * time.Millisecond)

		// Send stage-2 binary.
		sent = 0
		total = len(stage2Data)
		start = time.Now()
		for sent < total {
			end := sent + chunk
			if end > total {
				end = total
			}
			n, err := port.Write(stage2Data[sent:end])
			if err != nil {
				log.Fatalf("stage2 write error at byte %d: %v", sent, err)
			}
			sent += n
			pct := float64(sent) / float64(total) * 100
			elapsed := time.Since(start).Seconds()
			rate := float64(sent) / elapsed
			fmt.Fprintf(os.Stderr, "\rStage-2 sending: %d/%d bytes (%.1f%%) %.0f B/s   ", sent, total, pct, rate)
			if *delayFlag > 0 {
				time.Sleep(*delayFlag)
			}
		}
		elapsed = time.Since(start)
		fmt.Fprintf(os.Stderr, "\nStage-2 transfer complete: %d bytes in %s (%.0f B/s)\n",
			sent, elapsed.Round(time.Millisecond), float64(sent)/elapsed.Seconds())
	}

	// Post-transfer: wait for firmware response.
	if *monitorFlag {
		fmt.Fprintf(os.Stderr, "Waiting for target response (%s)...\n", *monitorWaitFlag)
		time.Sleep(*monitorWaitFlag)
		close(rxStop)
		<-rxDone

		rxMu.Lock()
		defer rxMu.Unlock()
		if len(rxBuf) > 0 {
			fmt.Fprintf(os.Stderr, "--- monitor complete: captured %d RX bytes ---\n", len(rxBuf))
		} else {
			fmt.Fprintf(os.Stderr, "--- no response from target ---\n")
		}
	} else if *waitFlag > 0 || *verboseFlag {
		close(rxStop)
		<-rxDone
	}
}

// printAsText writes buf to w, replacing non-printable bytes with '.' except
// CR and LF which are passed through unchanged.
func printAsText(w io.Writer, buf []byte) {
	for _, b := range buf {
		if b >= 0x20 && b < 0x7f {
			fmt.Fprintf(w, "%c", b)
		} else if b == '\r' || b == '\n' {
			fmt.Fprintf(w, "%c", b)
		} else {
			fmt.Fprintf(w, ".")
		}
	}
}

// probeLoop reads from the serial port and prints until interrupted.
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
