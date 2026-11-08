SOURCE_REPO_META := source_repo=github:BLNT-Protocol/blnt-backstop-manager-helper
WASM := target/wasm32v1-none/optimized/blnt_backstop_manager_helper.wasm

.PHONY: default build verify-source-metadata test check fmt clean

default: build

build:
	stellar contract build --package blnt-backstop-manager-helper --locked \
		--meta $(SOURCE_REPO_META) \
		--out-dir target/wasm32v1-none/optimized
	$(MAKE) --no-print-directory verify-source-metadata

verify-source-metadata:
	@stellar contract info meta --wasm $(WASM) 2>/dev/null | \
		grep -Fq "source_repo: github:BLNT-Protocol/blnt-backstop-manager-helper" || \
		{ printf 'error: missing source_repo metadata in helper WASM\n' >&2; exit 1; }

test:
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features --locked -- -D warnings
	cargo test --all-features --locked

check:
	cargo check --all-targets --all-features --locked

fmt:
	cargo fmt --all

clean:
	cargo clean
