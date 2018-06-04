DIR := $(shell basename $(CURDIR))

.PHONY: build
build: nightly
	cargo build

.PHONY: nightly
nightly:
	rustup override set nightly


.PHONY: install
install: nightly
	python3 setup.py install

.PHONY: test
test:
	# Run tests outside of project folder.
	# See https://github.com/PyO3/pyo3/issues/105
	cd .. && pytest --verbose --capture=no $(DIR)