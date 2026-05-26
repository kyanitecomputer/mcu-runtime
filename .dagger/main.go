// Dagger CI module for aspeed-mcu-runtime.
//
// Usage (from the aspeed-mcu-runtime repo root):
//
//	dagger call check-rot --aspeed-rs ../aspeed-rs \
//	                      --aspeed-data ../aspeed-data    # RoT firmware (AST10x0, BootMCU)
//	dagger call check-ssp --aspeed-rs ../aspeed-rs \
//	                      --aspeed-data ../aspeed-data    # coprocessor firmware (AST2600 SSP)
//	dagger call check-coldfire                            # ColdFire host tests
//	dagger call qemu-test --aspeed-rs ../aspeed-rs \
//	                      --aspeed-data ../aspeed-data    # QEMU firmware test
//	dagger call ci --aspeed-rs ../aspeed-rs \
//	               --aspeed-data ../aspeed-data           # full pipeline
package main

import (
	"context"
	"fmt"
	"strings"

	"dagger/aspeed-mcu-runtime/internal/dagger"
)

const rustChannel = "nightly-2026-04-01"

// rotTargets: RoT firmware binaries (AST10x0, AST2700 BootMCU).
// Built from app-rot/.
var rotTargets = []struct{ bin, feature, triple string }{
	// RoT firmware entry points
	{"rot_ast1060", "ast1060", "thumbv7em-none-eabihf"},
	{"rot_ast2700_bootmcu", "ast2700-bootmcu", "riscv32imc-unknown-none-elf"},
	// Diagnostics
	{"hello_uart_ast1060", "ast1060", "thumbv7em-none-eabihf"},
	{"hello_uart_bootmcu", "ast2700-bootmcu", "riscv32imc-unknown-none-elf"},
	{"uart5_bare", "ast1060", "thumbv7em-none-eabihf"},
	{"uart5_systick", "ast1060", "thumbv7em-none-eabihf"},
	// AST1080 and AST1040 require HAL support in embassy-aspeed first:
	// {"rot_ast1080", "ast1080", "thumbv7em-none-eabihf"},
	// {"rot_ast1040", "ast1040", "thumbv7em-none-eabihf"},
}

// sspTargets: coprocessor firmware binaries (AST2600 SSP, future AST2700 SSP/TSP).
// Built from app-coprocessor/ssp/.
var sspTargets = []struct{ bin, feature, triple string }{
	// Coprocessor firmware entry points
	{"ssp_ast2600", "ast2600-ssp", "thumbv7m-none-eabi"},
	// Diagnostics
	{"hello_uart", "ast2600-ssp", "thumbv7m-none-eabi"},
	{"ipc_echo", "ast2600-ssp", "thumbv7m-none-eabi"},
	// AST2700 SSP/TSP require HAL support in embassy-aspeed first:
	// {"ssp_ast2700", "ast2700-ssp", "thumbv7em-none-eabihf"},
	// {"tsp_ast2700", "ast2700-tsp", "thumbv7em-none-eabihf"},
}

// qemuTargets: firmwares testable under QEMU.
var qemuTargets = []struct {
	bin, feature, triple, machine, expect string
}{
	{"hello_uart_ast1060", "ast1060", "thumbv7em-none-eabihf", "ast1030-evb", "Hello from embassy on AST1060!"},
}

type AspeedMcuRuntime struct{}

// ── Container builders ────────────────────────────────────────────────────────

// rustBase returns a Rust nightly container with all embedded targets.
func (m *AspeedMcuRuntime) rustBase(
	src *dagger.Directory,
	aspeedRs *dagger.Directory,
	aspeedData *dagger.Directory,
) *dagger.Container {
	cargoCache := dag.CacheVolume("cargo-registry")
	buildCache := dag.CacheVolume("cargo-build-aspeed-mcu-runtime")

	return dag.Container().
		From("rust:1-slim").
		WithExec([]string{
			"rustup", "toolchain", "install", rustChannel,
			"--profile", "minimal",
			"--target", "thumbv7m-none-eabi",
			"--target", "thumbv7em-none-eabihf",
			"--target", "riscv32imc-unknown-none-elf",
			"--component", "rustfmt,clippy",
			"--no-self-update",
		}).
		WithExec([]string{"rustup", "default", rustChannel}).
		WithMountedCache("/usr/local/cargo/registry", cargoCache).
		WithMountedCache("/build/target", buildCache).
		WithDirectory("/build/aspeed-data", aspeedData).
		WithDirectory("/build/aspeed-rs", aspeedRs).
		WithDirectory("/build/aspeed-mcu-runtime", src)
}

// rotContainer returns a container with workdir set to app-rot.
func (m *AspeedMcuRuntime) rotContainer(
	src *dagger.Directory,
	aspeedRs *dagger.Directory,
	aspeedData *dagger.Directory,
) *dagger.Container {
	return m.rustBase(src, aspeedRs, aspeedData).
		WithWorkdir("/build/aspeed-mcu-runtime/app-rot")
}

// sspContainer returns a container with workdir set to app-coprocessor/ssp.
func (m *AspeedMcuRuntime) sspContainer(
	src *dagger.Directory,
	aspeedRs *dagger.Directory,
	aspeedData *dagger.Directory,
) *dagger.Container {
	return m.rustBase(src, aspeedRs, aspeedData).
		WithWorkdir("/build/aspeed-mcu-runtime/app-coprocessor/ssp")
}

// qemuContainer extends rotContainer with qemu-system-arm installed.
func (m *AspeedMcuRuntime) qemuContainer(
	src *dagger.Directory,
	aspeedRs *dagger.Directory,
	aspeedData *dagger.Directory,
) *dagger.Container {
	return m.rotContainer(src, aspeedRs, aspeedData).
		WithExec([]string{
			"apt-get", "update",
		}).
		WithExec([]string{
			"apt-get", "install", "-y", "--no-install-recommends",
			"qemu-system-arm",
		}).
		WithExec([]string{
			"rm", "-rf", "/var/lib/apt/lists/*",
		})
}

// coldfireContainer builds a container with GCC for ColdFire host tests.
func (m *AspeedMcuRuntime) coldfireContainer(src *dagger.Directory) *dagger.Container {
	return dag.Container().
		From("gcc:14").
		WithDirectory("/src", src).
		WithWorkdir("/src/app-coprocessor/coldfire")
}

// ── Public functions ──────────────────────────────────────────────────────────

// CheckRot compiles all RoT firmware binaries (AST10x0, AST2700 BootMCU).
// Pass both sibling repos: --aspeed-rs ../aspeed-rs --aspeed-data ../aspeed-data
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
			}).
			Sync(ctx)
		if err != nil {
			return fmt.Errorf("check bin=%s feature=%s: %w", t.bin, t.feature, err)
		}
	}
	return nil
}

// CheckSsp compiles all coprocessor firmware binaries (AST2600 SSP).
// Pass both sibling repos: --aspeed-rs ../aspeed-rs --aspeed-data ../aspeed-data
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
	_, err := m.coldfireContainer(src).
		WithExec([]string{"make", "check"}).
		Sync(ctx)
	return err
}

// QemuTest builds AST1060 firmware and runs it under QEMU (ast1030-evb) to
// verify UART output. Validates that the firmware boots and prints expected text.
// Pass both sibling repos: --aspeed-rs ../aspeed-rs --aspeed-data ../aspeed-data
func (m *AspeedMcuRuntime) QemuTest(
	ctx context.Context,
	// +defaultPath="."
	src *dagger.Directory,
	aspeedRs *dagger.Directory,
	aspeedData *dagger.Directory,
) error {
	ctr := m.qemuContainer(src, aspeedRs, aspeedData)

	for _, t := range qemuTargets {
		ctr = ctr.WithExec([]string{
			"cargo", "build",
			"--bin", t.bin,
			"--features", t.feature,
			"--target", t.triple,
			"--release",
		})

		elf := fmt.Sprintf("target/%s/release/%s", t.triple, t.bin)
		out, err := ctr.
			WithExec([]string{
				"timeout", "10",
				"qemu-system-arm",
				"-M", t.machine,
				"-nographic",
				"-kernel", elf,
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

// Ci runs the full pipeline: CheckRot + CheckSsp + CheckColdfire + QemuTest.
// Pass both sibling repos: --aspeed-rs ../aspeed-rs --aspeed-data ../aspeed-data
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
