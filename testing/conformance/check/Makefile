TESTS += multiple-explicit-selection
TESTS += multiple-issues-error
TESTS += single-reproduced

RP = ../../../target/debug/rp

test: $(RP)
	@for test in $(TESTS); do \
		../run-test.sh "$(RP)" "$$test" "check" || exit 1; \
	done

$(RP):
	@cargo build

.PHONY: test
