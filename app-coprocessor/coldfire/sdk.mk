# sdk.mk — Include from your project Makefile.
#
# Required before inclusion:
#   SDK_DIR  := path/to/ast-cf-runtime
#   SRCS     += your_file.c ...
#
# Optional overrides (set before inclusion):
#   SDK_PLATFORM        ast2500 | ast2400   (default: ast2500)
#   SDK_MAX_TASKS       integer             (default: 8)
#   SDK_TICK_HZ         integer             (default: 1000)
#   SDK_TICK_TIMER      1-7                 (default: 7, reserved for SDK)
#   SDK_SRAM_STACK_SIZE bytes               (default: 2048)
#   SDK_USE_NEWLIB_NANO 1                   (default: off)
#   CROSS               toolchain prefix    (default: m68k-elf-)
#
# Targets provided:
#   all (default)  firmware.elf + firmware.bin
#   clean          remove build/*, firmware.elf, firmware.bin, firmware.lst
#   disasm         firmware.lst (annotated disassembly)
#   size           section sizes via m68k-elf-size

SDK_PLATFORM        ?= ast2500
SDK_MAX_TASKS       ?= 8
SDK_TICK_HZ         ?= 1000
SDK_TICK_TIMER      ?= 7
SDK_SRAM_STACK_SIZE ?= 2048
SDK_USE_NEWLIB_NANO ?= 0

CROSS   ?= m68k-elf-
CC      := $(CROSS)gcc
OBJCOPY := $(CROSS)objcopy
OBJDUMP := $(CROSS)objdump
SIZE    := $(CROSS)size

# Convert platform name to upper-case define (ast2500 → AST2500)
_SDK_PLATFORM_UC := $(shell echo $(SDK_PLATFORM) | tr '[:lower:]' '[:upper:]')

CFLAGS := \
    -mcpu=cfv1 \
    -std=c11 \
    -ffreestanding \
    -fno-builtin \
    -fno-common \
    -ffunction-sections \
    -fdata-sections \
    -Os \
    -g3 \
    -Wall \
    -Wextra \
    -Werror \
    -Wshadow \
    -Wconversion \
    -Wno-unused-parameter \
    -DSDK_PLATFORM_$(_SDK_PLATFORM_UC) \
    -DSDK_MAX_TASKS=$(SDK_MAX_TASKS) \
    -DSDK_TICK_HZ=$(SDK_TICK_HZ) \
    -DSDK_TICK_TIMER=$(SDK_TICK_TIMER) \
    -DSDK_SRAM_STACK_SIZE=$(SDK_SRAM_STACK_SIZE)

ASFLAGS := -mcpu=cfv1

LDFLAGS := \
    -mcpu=cfv1 \
    -nostdlib \
    -Wl,--gc-sections \
    -T $(SDK_DIR)/linker/cfv1-$(SDK_PLATFORM).ld

ifeq ($(SDK_USE_NEWLIB_NANO),1)
LDFLAGS += -lc_nano -lm
endif
LDFLAGS += -lgcc

INCLUDES := -I$(SDK_DIR)/include

BUILD := build

# SDK sources — runtime always included; HAL files included if they exist
_SDK_RUNTIME := \
    $(SDK_DIR)/src/runtime/startup.S \
    $(SDK_DIR)/src/runtime/vectors.c \
    $(SDK_DIR)/src/runtime/executor.c \
    $(SDK_DIR)/src/runtime/mem.c

_SDK_HAL := $(wildcard $(SDK_DIR)/src/hal/*.c)

_SDK_SRCS := $(_SDK_RUNTIME) $(_SDK_HAL)

# Derive object paths, preserving subdirectory structure under build/sdk/
_sdk_obj = $(BUILD)/sdk/$(patsubst $(SDK_DIR)/%,%,$(patsubst %.S,%.o,$(patsubst %.c,%.o,$(1))))
_user_obj = $(BUILD)/user/$(notdir $(patsubst %.c,%.o,$(1)))

_SDK_OBJS  := $(foreach f,$(_SDK_SRCS),$(call _sdk_obj,$(f)))
_USER_OBJS := $(foreach f,$(SRCS),$(call _user_obj,$(f)))
_ALL_OBJS  := $(_SDK_OBJS) $(_USER_OBJS)

.PHONY: all clean disasm size

all: firmware.elf firmware.bin

firmware.elf: $(_ALL_OBJS)
	$(CC) $(LDFLAGS) -o $@ $^

firmware.bin: firmware.elf
	$(OBJCOPY) -O binary $< $@

# SDK C sources
$(BUILD)/sdk/%.o: $(SDK_DIR)/%.c
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) $(INCLUDES) -c -o $@ $<

# SDK assembly sources
$(BUILD)/sdk/%.o: $(SDK_DIR)/%.S
	@mkdir -p $(dir $@)
	$(CC) $(ASFLAGS) $(INCLUDES) -c -o $@ $<

# User C sources (flat; place all .c files in the project root)
$(BUILD)/user/%.o: %.c
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) $(INCLUDES) -c -o $@ $<

clean:
	rm -rf $(BUILD) firmware.elf firmware.bin firmware.lst

disasm: firmware.elf
	$(OBJDUMP) -d -S $< > firmware.lst

size: firmware.elf
	$(SIZE) $<
