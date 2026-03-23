build:
	@cargo build

unit:
	@cargo test

conformance:
	@$(MAKE) -s -C testing/conformance test

test: unit conformance

.PHONY: build unit conformance test
