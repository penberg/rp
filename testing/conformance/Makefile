SUITES += check

test:
	@for suite in $(SUITES); do \
		$(MAKE) -C "$$suite" test || exit 1; \
	done

.PHONY: test
