.PHONY: install-ocodeql install-vscode-ocql docker-build docker-run

install-ocodeql:
	cargo install --path crates/ocodeql --locked
	@echo "Installed ocodeql to $$(which ocodeql || echo '~/.cargo/bin/ocodeql')"

install-vscode-ocql:
	cargo build --release -p ocql-lsp
	cd editors/vscode && npm install --cache /tmp/npm-cache && npx tsc -p tsconfig.json
	cd editors/vscode && npx @vscode/vsce package --no-dependencies -o open-codeql.vsix
	code --install-extension editors/vscode/open-codeql.vsix
	@echo "Installed. Restart VS Code to activate the extension."

docker-build:
	@if [ ! -f vendor/codeql-linux64.zip ]; then \
		echo "ERROR: vendor/codeql-linux64.zip not found."; \
		echo "Download it first:"; \
		echo "  wget https://github.com/github/codeql-cli-binaries/releases/download/v2.25.1/codeql-linux64.zip -O vendor/codeql-linux64.zip"; \
		exit 1; \
	fi
	docker build --platform linux/amd64 -t open-codeql .

docker-run:
	docker run --platform linux/amd64 -it --rm \
		-v $$(pwd)/vendor/codeql:/workspace/vendor/codeql \
		-v $$(pwd)/vendor/test-repos:/workspace/vendor/test-repos \
		open-codeql
