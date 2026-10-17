.PHONY: test

# Run after cargo fetch --locked and npm ci in packages/logrix-sdk.
# Dependencies must already be cached/installed; neither command fetches them.
test:
	cargo test --workspace --offline --locked
	npm --prefix packages/logrix-sdk --offline test
