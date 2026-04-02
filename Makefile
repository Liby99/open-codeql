.PHONY: install-vscode-ocql

install-vscode-ocql:
	cargo build --release -p ocql-lsp
	cd editors/vscode && npm install --cache /tmp/npm-cache && npx tsc -p tsconfig.json
	cd editors/vscode && npx @vscode/vsce package --no-dependencies -o open-codeql.vsix
	code --install-extension editors/vscode/open-codeql.vsix
	@echo "Installed. Restart VS Code to activate the extension."
