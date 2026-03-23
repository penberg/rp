TESTS += broken-reproducer-stderr
TESTS += multiple-explicit-selection
TESTS += multiple-issues-error
TESTS += single-reproduced

RP = ../../../target/debug/rp

test:
	@cargo build
	@for test in $(TESTS); do \
		../run-test.sh "$(RP)" "$$test" "check" || exit 1; \
	done

.PHONY: test
