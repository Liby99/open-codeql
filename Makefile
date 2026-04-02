.PHONY: install-vscode-ocql

install-vscode-ocql:
	cargo build --release -p ocql-lsp
	cd editors/vscode && npm install --cache /tmp/npm-cache && npx tsc -p tsconfig.json
	ln -sfn $(CURDIR)/editors/vscode $(HOME)/.vscode/extensions/open-codeql
	@echo "Installed. Restart VS Code to activate the extension."
