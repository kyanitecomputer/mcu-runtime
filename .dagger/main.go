// Dagger CI module for aspeed-mcu-runtime.
//
// Usage (from the aspeed-mcu-runtime repo root):
//
//	dagger call check-coldfire                            # ColdFire host tests
//	dagger call check-rot --aspeed-rs ../aspeed-rs \
//	                      --aspeed-data ../aspeed-data    # compile all firmware
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

// rotTargets: (binary, feature, triple) for every app-rot firmware binary.
var rotTargets = []struct{ bin, feature, triple string }{
	{"hello_uart", "ast2600-ssp", "thumbv7m-none-eabi"},
	{"blinky", "ast2600-ssp", "thumbv7m-none-eabi"},
	{"ipc_echo", "ast2600-ssp", "thumbv7m-none-eabi"},
	{"hello_uart_ast1060", "ast1060", "thumbv7em-none-eabihf"},
	{"blinky_ast1060", "ast1060", "thumbv7em-none-eabihf"},
	{"hello_uart_bootmcu", "ast2700-bootmcu", "riscv32imc-unknown-none-elf"},
	{"ipc_echo_bootmcu", "ast2700-bootmcu", "riscv32imc-unknown-none-elf"},
}

type AspeedMcuRuntime struct{}

// ── Container builders ────────────────────────────────────────────────────────

// rotContainer builds a Rust nightly container with all embedded targets and
// the full three-repo path-dep chain mounted:
//
//	/build/aspeed-data/         (aspeed-pac path dep from embassy-aspeed)
//	/build/aspeed-rs/           (embassy-aspeed path dep from app-rot)
//	/build/aspeed-mcu-runtime/  (app-rot source)
func (m *AspeedMcuRuntime) rotContainer(
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
		WithDirectory("/build/aspeed-mcu-runtime", src).
		WithWorkdir("/build/aspeed-mcu-runtime/app-rot")
}

// coldfireContainer builds a container with GCC for ColdFire host tests.
func (m *AspeedMcuRuntime) coldfireContainer(src *dagger.Directory) *dagger.Container {
	return dag.Container().
		From("gcc:14").
		WithDirectory("/src", src).
		WithWorkdir("/src/app-coprocessor/coldfire")
}

// ── Public functions ──────────────────────────────────────────────────────────

// CheckRot compiles all app-rot firmware binaries for their respective chip targets.
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

// Check compiles all firmware and runs ColdFire host tests.
// Pass both sibling repos: --aspeed-rs ../aspeed-rs --aspeed-data ../aspeed-data
func (m *AspeedMcuRuntime) Check(
	ctx context.Context,
	// +defaultPath="."
	src *dagger.Directory,
	aspeedRs *dagger.Directory,
	aspeedData *dagger.Directory,
) error {
	if err := m.CheckRot(ctx, src, aspeedRs, aspeedData); err != nil {
		return fmt.Errorf("check-rot: %w", err)
	}
	return m.CheckColdfire(ctx, src)
}

// Ci runs the full pipeline: Check (rot + coldfire).
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

	if err := m.CheckColdfire(ctx, src); err != nil {
		return "", fmt.Errorf("check-coldfire: %w", err)
	}
	steps = append(steps, "check-coldfire: ok")

	return strings.Join(steps, "\n"), nil
}
