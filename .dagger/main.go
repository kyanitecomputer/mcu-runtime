// Dagger CI module for aspeed-mcu-runtime.
//
// All containers use StageX images for reproducible, minimal builds. StageX
// ships a pinned, rustup-less Rust toolchain, so the embedded firmware is
// checked with -Zbuild-std=core (rust-src is bundled; RUSTC_BOOTSTRAP unlocks
// it on stable).
//
// Usage (from the aspeed-mcu-runtime repo root):
//
//	dagger call check-rot --aspeed-rs ../aspeed-rs \
//	                      --aspeed-data ../aspeed-data
//	dagger call check-ssp --aspeed-rs ../aspeed-rs \
//	                      --aspeed-data ../aspeed-data
//	dagger call check-coldfire
//	dagger call qemu-test --aspeed-rs ../aspeed-rs \
//	                      --aspeed-data ../aspeed-data
//	dagger call build-ast-2700-image --aspeed-rs ../aspeed-rs \
//	                              --aspeed-data ../aspeed-data \
//	                              --tamago ../tamago \
//	                              --tamago-go ../../tamago/tamago-go \
//	                              --cmd-nats ../cmd/nats \
//	                              --aspeed-go ../aspeed-go \
//	                              --lneto ../lneto \
//	                              --nats-server ../nats-server \
//	                              --scree ../scree \
//	                              --image-size 32M \
//	                              --bmc-pb ../bmc-pb/ast2700a1 export --path ./out
//	dagger call ci --aspeed-rs ../aspeed-rs \
//	               --aspeed-data ../aspeed-data
package main

import (
	"context"
	"fmt"
	"strings"

	"dagger/aspeed-mcu-runtime/internal/dagger"
)

const (
	// StageX container images for reproducible builds.
	stagexRust   = "stagex/pallet-rust:sx2026.06.0"
	stagexGo     = "stagex/pallet-go:sx2026.06.0"
	stagexPallet = "stagex/pallet-cgo:sx2026.06.0" // go + clang/cc + make + coreutils
	stagexLlvm   = "stagex/core-llvm:sx2026.06.0"

	// StageX user-qemu is x86_64-only (no qemu-system-arm), so QEMU smoke
	// tests use a Debian-based qemu image that ships qemu-system-arm with the
	// Aspeed ast1030-evb machine. Pinned by digest for reproducibility.
	qemuImage = "qemux/qemu-arm@sha256:39312360a8fdd723e61e9416f045e280912c6e307df9c9bc12ce791a1d070400"
)

const (
	bootmcuTarget = "riscv32imc-unknown-none-elf"
	bootmcuBin    = "rot_ast2700_bootmcu"
	// No portable_atomic_unsafe_assume_single_core cfg: app-rot deliberately
	// uses portable-atomic's critical-section provider (riscv
	// critical-section-single-hart), which is mutually exclusive with that cfg.
	bootmcuFlags = "-C link-arg=-Tmemory.x -C link-arg=-Tlink.x -C link-arg=--nmagic"
)

var rotTargets = []struct{ bin, feature, triple string }{
	{"rot_ast1060", "ast1060", "thumbv7em-none-eabihf"},
	{"rot_ast2700_bootmcu", "ast2700-bootmcu", "riscv32imc-unknown-none-elf"},
	{"hello_uart_ast1060", "ast1060", "thumbv7em-none-eabihf"},
	{"hello_uart_bootmcu", "ast2700-bootmcu", "riscv32imc-unknown-none-elf"},
	{"uart5_bare", "ast1060", "thumbv7em-none-eabihf"},
	{"uart5_systick", "ast1060", "thumbv7em-none-eabihf"},
}

var sspTargets = []struct{ bin, feature, triple string }{
	{"ssp_ast2600", "ast2600-ssp", "thumbv7m-none-eabi"},
	{"hello_uart", "ast2600-ssp", "thumbv7m-none-eabi"},
	{"ipc_echo", "ast2600-ssp", "thumbv7m-none-eabi"},
}

var qemuTargets = []struct {
	bin, feature, triple, machine, expect string
}{
	{"hello_uart_ast1060", "ast1060", "thumbv7em-none-eabihf", "ast1030-evb", "Hello from embassy on AST1060!"},
}

type AspeedMcuRuntime struct{}

// ── Container builders ────────────────────────────────────────────────────────

// rustBase returns a StageX Rust container. StageX ships a pinned, rustup-less
// toolchain with no prebuilt bare-metal std, so the embedded firmware is checked
// with -Zbuild-std=core (rust-src is bundled); RUSTC_BOOTSTRAP unlocks that
// unstable flag on the pinned stable toolchain.
func (m *AspeedMcuRuntime) rustBase(
	src *dagger.Directory,
	aspeedRs *dagger.Directory,
	aspeedData *dagger.Directory,
) *dagger.Container {
	cargoCache := dag.CacheVolume("cargo-registry")
	buildCache := dag.CacheVolume("cargo-build-aspeed-mcu-runtime")

	return dag.Container().
		From(stagexRust).
		WithEnvVariable("CARGO_HOME", "/usr/local/cargo").
		WithEnvVariable("RUSTC_BOOTSTRAP", "1").
		WithMountedCache("/usr/local/cargo/registry", cargoCache).
		WithMountedCache("/build/target", buildCache).
		WithDirectory("/build/aspeed-data", aspeedData).
		WithDirectory("/build/aspeed-rs", aspeedRs).
		WithDirectory("/build/aspeed-mcu-runtime", src)
}

func (m *AspeedMcuRuntime) rotContainer(
	src *dagger.Directory,
	aspeedRs *dagger.Directory,
	aspeedData *dagger.Directory,
) *dagger.Container {
	return m.rustBase(src, aspeedRs, aspeedData).
		WithWorkdir("/build/aspeed-mcu-runtime/app-rot")
}

func (m *AspeedMcuRuntime) sspContainer(
	src *dagger.Directory,
	aspeedRs *dagger.Directory,
	aspeedData *dagger.Directory,
) *dagger.Container {
	return m.rustBase(src, aspeedRs, aspeedData).
		WithWorkdir("/build/aspeed-mcu-runtime/app-coprocessor/ssp")
}

// qemuRunner returns a container that can execute ARM firmware under
// qemu-system-arm. The firmware ELF is built separately in the StageX rust
// container and copied in, so no cross-libc binary stitching is needed.
func (m *AspeedMcuRuntime) qemuRunner() *dagger.Container {
	return dag.Container().From(qemuImage)
}

// coldfireContainer builds a StageX pallet-cgo container for the ColdFire host
// tests. pallet-cgo bundles make + a C compiler (cc = clang) + coreutils; the
// distroless pallet-gcc image lacks make/shell. The host tests are portable
// C11, so cc (clang) stands in for gcc — passed as CC=cc by CheckColdfire.
func (m *AspeedMcuRuntime) coldfireContainer(src *dagger.Directory) *dagger.Container {
	return dag.Container().
		From(stagexPallet).
		WithDirectory("/src", src).
		WithWorkdir("/src/app-coprocessor/coldfire")
}

// bootmcuFirmwareELF builds the AST2700 BootMCU firmware ELF in the StageX rust
// container. StageX ships no prebuilt riscv32 std, so core is built via
// -Zbuild-std (RUSTC_BOOTSTRAP unlocks it on the pinned stable toolchain) —
// the same rustup-less approach proven by CheckRot.
func (m *AspeedMcuRuntime) bootmcuFirmwareELF(
	src *dagger.Directory,
	aspeedRs *dagger.Directory,
	aspeedData *dagger.Directory,
) *dagger.File {
	elf := "/build/aspeed-mcu-runtime/app-rot/target/" +
		bootmcuTarget + "/release/" + bootmcuBin
	return m.rustBase(src, aspeedRs, aspeedData).
		WithWorkdir("/build/aspeed-mcu-runtime/app-rot").
		WithEnvVariable("RUSTFLAGS", bootmcuFlags).
		WithExec([]string{
			"cargo", "build",
			"--target", bootmcuTarget,
			"--release",
			"--bin", bootmcuBin,
			"--no-default-features",
			"--features", "ast2700-bootmcu",
			"-Z", "build-std=core",
		}).
		File(elf)
}

// ast2700ImageContainer builds the Go/TamaGo CA35 payload and stitches the SPI
// flash image with the Go imgtools helper (no python/shell). The Rust BootMCU
// firmware ELF is built separately in the StageX rust container (see
// bootmcuFirmwareELF) and copied in, so this container needs no Rust toolchain.
func (m *AspeedMcuRuntime) ast2700ImageContainer(
	src *dagger.Directory,
	aspeedRs *dagger.Directory,
	aspeedData *dagger.Directory,
	tamago *dagger.Directory,
	tamagoGo *dagger.Directory,
	cmdNats *dagger.Directory,
	aspeedGo *dagger.Directory,
	lneto *dagger.Directory,
	natsServer *dagger.Directory,
	scree *dagger.Directory,
	bmcPb *dagger.Directory,
) *dagger.Container {
	goCache := dag.CacheVolume("go-mod-cache")
	goBuild := dag.CacheVolume("go-build-cache-tamago")

	llvmObjcopy := dag.Container().From(stagexLlvm).File("/usr/bin/llvm-objcopy")

	return dag.Container().
		From(stagexGo).
		// LLVM objcopy from StageX for ELF → raw binary conversion.
		WithFile("/usr/bin/llvm-objcopy", llvmObjcopy).
		WithMountedCache("/go/pkg/mod", goCache).
		WithMountedCache("/root/.cache/go-build", goBuild).
		WithDirectory("/build/aspeed-data", aspeedData).
		WithDirectory("/build/aspeed-rs", aspeedRs).
		WithDirectory("/build/aspeed-mcu-runtime", src).
		WithDirectory("/build/tamago", tamago).
		WithDirectory("/build/tamago-go", tamagoGo).
		WithDirectory("/build/cmd/nats", cmdNats).
		WithDirectory("/build/aspeed-go", aspeedGo).
		WithDirectory("/build/lneto", lneto).
		WithDirectory("/build/nats-server", natsServer).
		WithDirectory("/build/scree", scree).
		WithDirectory("/build/bmc-pb", bmcPb).
		WithNewFile("/build/go.work", "go 1.26.2\n\nuse (\n\t./tamago\n\t./aspeed-go\n\t./lneto\n\t./nats-server\n\t./scree\n\t./cmd/nats\n)\n").
		WithExec([]string{"mkdir", "-p", "/out"}).
		// Build the Go imgtools binary for image stitching.
		WithEnvVariable("GOWORK", "off").
		WithExec([]string{"go", "build", "-C", "/build/aspeed-mcu-runtime/tools/imgtools", "-o", "/usr/local/bin/imgtools", "."})
}

// ── Public functions ──────────────────────────────────────────────────────────

// CheckRot compiles all RoT firmware binaries (AST10x0, AST2700 BootMCU).
func (m *AspeedMcuRuntime) CheckRot(
	ctx context.Context,
	// +defaultPath="."
	src *dagger.Directory,
	aspeedRs *dagger.Directory,
	aspeedData *dagger.Directory,
) error {
	ctr := m.rotContainer(src, aspeedRs, aspeedData)
	for _, t := range rotTargets {
		_, err := ctr.
			WithExec([]string{
				"cargo", "check",
				"--bin", t.bin,
				"--features", t.feature,
				"--target", t.triple,
				"-Z", "build-std=core",
			}).
			Sync(ctx)
		if err != nil {
			return fmt.Errorf("check bin=%s feature=%s: %w", t.bin, t.feature, err)
		}
	}
	return nil
}

// CheckSsp compiles all coprocessor firmware binaries (AST2600 SSP).
func (m *AspeedMcuRuntime) CheckSsp(
	ctx context.Context,
	// +defaultPath="."
	src *dagger.Directory,
	aspeedRs *dagger.Directory,
	aspeedData *dagger.Directory,
) error {
	ctr := m.sspContainer(src, aspeedRs, aspeedData)
	for _, t := range sspTargets {
		_, err := ctr.
			WithExec([]string{
				"cargo", "check",
				"--bin", t.bin,
				"--features", t.feature,
				"--target", t.triple,
				"-Z", "build-std=core",
			}).
			Sync(ctx)
		if err != nil {
			return fmt.Errorf("check bin=%s feature=%s: %w", t.bin, t.feature, err)
		}
	}
	return nil
}

// CheckColdfire runs the ColdFire SDK host unit tests (make check).
func (m *AspeedMcuRuntime) CheckColdfire(
	ctx context.Context,
	// +defaultPath="."
	src *dagger.Directory,
) error {
	// `make clean` first so any stale (locally-built, glibc-linked) host-test
	// binaries in the source tree are rebuilt with the container's cc.
	_, err := m.coldfireContainer(src).
		WithExec([]string{"make", "clean", "CC=cc"}).
		WithExec([]string{"make", "check", "CC=cc"}).
		Sync(ctx)
	return err
}

// QemuTest builds AST1060 firmware and runs it under QEMU to verify UART output.
func (m *AspeedMcuRuntime) QemuTest(
	ctx context.Context,
	// +defaultPath="."
	src *dagger.Directory,
	aspeedRs *dagger.Directory,
	aspeedData *dagger.Directory,
) error {
	build := m.rotContainer(src, aspeedRs, aspeedData)

	for _, t := range qemuTargets {
		built := build.WithExec([]string{
			"cargo", "build",
			"--bin", t.bin,
			"--features", t.feature,
			"--target", t.triple,
			"--release",
			"-Z", "build-std=core",
		})

		elf := fmt.Sprintf(
			"/build/aspeed-mcu-runtime/app-rot/target/%s/release/%s",
			t.triple, t.bin,
		)
		out, err := m.qemuRunner().
			WithFile("/fw.elf", built.File(elf)).
			WithExec([]string{
				"timeout", "10",
				"qemu-system-arm",
				"-M", t.machine,
				"-nographic",
				"-kernel", "/fw.elf",
			}, dagger.ContainerWithExecOpts{
				Expect: dagger.ReturnTypeAny,
			}).
			Sync(ctx)
		if err != nil {
			return fmt.Errorf("qemu bin=%s: %w", t.bin, err)
		}

		stdout, err := out.Stdout(ctx)
		if err != nil {
			return fmt.Errorf("qemu stdout bin=%s: %w", t.bin, err)
		}
		if !strings.Contains(stdout, t.expect) {
			return fmt.Errorf("qemu bin=%s: expected %q in output, got:\n%s", t.bin, t.expect, stdout)
		}
	}

	return nil
}

// BuildAst2700Image builds BootMCU firmware + TamaGo payload and stitches
// them into a complete SPI flash image using Go imgtools.
func (m *AspeedMcuRuntime) BuildAst2700Image(
	ctx context.Context,
	// +defaultPath="."
	src *dagger.Directory,
	aspeedRs *dagger.Directory,
	aspeedData *dagger.Directory,
	tamago *dagger.Directory,
	tamagoGo *dagger.Directory,
	cmdNats *dagger.Directory,
	aspeedGo *dagger.Directory,
	lneto *dagger.Directory,
	natsServer *dagger.Directory,
	scree *dagger.Directory,
	bmcPb *dagger.Directory,
	// +default="32M"
	imageSize string,
	// +default="ast2700_bootmcu_nats.bin"
	outputName string,
	// +default="linkcpuinit,ast2700dcscm"
	goBuildTags string,
	// +default="0x404000000"
	ca35LinkAddress string,
	// +default="0x1000"
	ca35Reserve string,
	// +default=false
	includeSsp bool,
	// +default=false
	includeTsp bool,
) (*dagger.Directory, error) {
	if imageSize == "" {
		imageSize = "32M"
	}
	if outputName == "" {
		outputName = "ast2700_bootmcu_nats.bin"
	}
	if goBuildTags == "" {
		goBuildTags = "linkcpuinit,ast2700dcscm"
	}
	if ca35LinkAddress == "" {
		ca35LinkAddress = "0x404000000"
	}
	if ca35Reserve == "" {
		ca35Reserve = "0x1000"
	}

	spiImageArgs := []string{
		"imgtools", "spi-image",
		"--caliptra", "/build/bmc-pb/caliptra-fw.bin",
		"--fmc", "/out/rot_ast2700_bootmcu.fmc.bin",
		"--prebuilt", "1:/build/bmc-pb/ddr4_pmu_train_imem.bin",
		"--prebuilt", "2:/build/bmc-pb/ddr4_pmu_train_dmem.bin",
		"--prebuilt", "3:/build/bmc-pb/ddr4_2d_pmu_train_imem.bin",
		"--prebuilt", "4:/build/bmc-pb/ddr4_2d_pmu_train_dmem.bin",
		"--prebuilt", "5:/build/bmc-pb/ddr5_pmu_train_imem.bin",
		"--prebuilt", "6:/build/bmc-pb/ddr5_pmu_train_dmem.bin",
		"--prebuilt", "7:/build/bmc-pb/dp_fw.bin",
		"--a35-payload", "/out/ast2700_nats.raw.bin",
	}
	if includeSsp {
		spiImageArgs = append(spiImageArgs, "--ssp-payload", "/build/bmc-pb/ssp.bin")
	}
	if includeTsp {
		spiImageArgs = append(spiImageArgs, "--tsp-payload", "/build/bmc-pb/tsp.bin")
	}
	spiImageArgs = append(spiImageArgs,
		"--size", imageSize,
		"--output", "/out/"+outputName,
	)

	firmware := m.bootmcuFirmwareELF(src, aspeedRs, aspeedData)

	ctr := m.ast2700ImageContainer(src, aspeedRs, aspeedData, tamago, tamagoGo, cmdNats, aspeedGo, lneto, natsServer, scree, bmcPb).
		// BootMCU firmware ELF (built in the StageX rust container) → raw bin.
		WithFile("/out/rot_ast2700_bootmcu.elf", firmware).
		WithExec([]string{"llvm-objcopy", "-O", "binary",
			"/out/rot_ast2700_bootmcu.elf",
			"/out/rot_ast2700_bootmcu.fmc.bin"}).
		// Build NATS CA35 payload (Go/TamaGo) for hardware PHY testing.
		WithWorkdir("/build/cmd/nats").
		WithEnvVariable("GOOSPKG", "github.com/usbarmory/tamago").
		WithEnvVariable("GOOS", "tamago").
		WithEnvVariable("GOARCH", "arm64").
		WithEnvVariable("GOTOOLCHAIN", "local").
		WithEnvVariable("GOWORK", "/build/go.work").
		WithExec([]string{"/build/tamago-go/bin/go", "build",
			"-tags", goBuildTags,
			"-ldflags", "-T " + ca35LinkAddress + " -R " + ca35Reserve,
			"-o", "/out/ast2700_nats.elf",
			".",
		}).
		WithExec([]string{"llvm-objcopy", "-O", "binary",
			"/out/ast2700_nats.elf",
			"/out/ast2700_nats.raw.bin"}).
		// Stitch SPI flash image using Go imgtools (no Python).
		WithWorkdir("/build/aspeed-mcu-runtime").
		WithExec(spiImageArgs)

	return ctr.Directory("/out").Sync(ctx)
}

// Ci runs the full pipeline: CheckRot + CheckSsp + CheckColdfire + QemuTest.
func (m *AspeedMcuRuntime) Ci(
	ctx context.Context,
	// +defaultPath="."
	src *dagger.Directory,
	aspeedRs *dagger.Directory,
	aspeedData *dagger.Directory,
) (string, error) {
	steps := []string{}

	if err := m.CheckRot(ctx, src, aspeedRs, aspeedData); err != nil {
		return "", fmt.Errorf("check-rot: %w", err)
	}
	steps = append(steps, "check-rot: ok")

	if err := m.CheckSsp(ctx, src, aspeedRs, aspeedData); err != nil {
		return "", fmt.Errorf("check-ssp: %w", err)
	}
	steps = append(steps, "check-ssp: ok")

	if err := m.CheckColdfire(ctx, src); err != nil {
		return "", fmt.Errorf("check-coldfire: %w", err)
	}
	steps = append(steps, "check-coldfire: ok")

	if err := m.QemuTest(ctx, src, aspeedRs, aspeedData); err != nil {
		return "", fmt.Errorf("qemu-test: %w", err)
	}
	steps = append(steps, "qemu-test: ok")

	return strings.Join(steps, "\n"), nil
}
