SDK_DIR := ..
include $(SDK_DIR)/sdk.mk
SRCS    += main.c

# SDK_PLATFORM  := ast2400   # uncomment for AST2400 target

# Internal target used by SDK root 'make check-static' to verify the
# template compiles cleanly with the cross-compiler.
.PHONY: check-compile
check-compile:
	$(MAKE) all
