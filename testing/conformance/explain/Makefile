TESTS += single-issue

RP = ../../../target/debug/rp

test:
	@cargo build
	@for test in $(TESTS); do \
		../run-test.sh "$(RP)" "$$test" "explain" || exit 1; \
	done

.PHONY: test
