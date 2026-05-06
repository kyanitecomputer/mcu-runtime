# SDK root Makefile — self-tests and static analysis only.
# To build firmware, use sdk.mk from your project directory (see template/).

.PHONY: check check-static clean

# Run host-native unit tests (compiled with system gcc, not cross-compiler).
check:
	$(MAKE) -C test/host check

# Static analysis: cppcheck + full -Werror cross-compile build check.
# Requires m68k-elf-gcc and cppcheck to be installed.
check-static:
	cppcheck \
	    --enable=all \
	    --std=c11 \
	    --suppress=missingIncludeSystem \
	    --error-exitcode=1 \
	    -Iinclude \
	    src/
	@echo "cppcheck passed"
	$(MAKE) -C template check-compile

clean:
	$(MAKE) -C test/host clean
	$(MAKE) -C template clean
