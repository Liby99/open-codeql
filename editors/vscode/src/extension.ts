import * as path from "path";
import * as fs from "fs";
import {
  workspace,
  ExtensionContext,
  window,
} from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  Executable,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

function findServerBinary(context: ExtensionContext): string | undefined {
  // 1. Check user setting
  const configured = workspace
    .getConfiguration("open-codeql")
    .get<string>("serverPath");
  if (configured && fs.existsSync(configured)) {
    return configured;
  }

  // 2. Check workspace target directories (dev builds)
  const workspaceFolders = workspace.workspaceFolders;
  if (workspaceFolders) {
    for (const folder of workspaceFolders) {
      for (const profile of ["release", "debug"]) {
        const candidate = path.join(
          folder.uri.fsPath,
          "target",
          profile,
          "ocql-lsp"
        );
        if (fs.existsSync(candidate)) {
          return candidate;
        }
      }
    }
  }

  // 3. Check next to the extension
  const extDir = context.extensionPath;
  const nearby = path.join(extDir, "..", "..", "target", "release", "ocql-lsp");
  if (fs.existsSync(nearby)) {
    return nearby;
  }

  // 4. Fall back to PATH
  return "ocql-lsp";
}

export function activate(context: ExtensionContext) {
  const serverPath = findServerBinary(context);
  if (!serverPath) {
    window.showWarningMessage(
      "Open CodeQL: could not find ocql-lsp binary. Syntax highlighting is active but diagnostics are unavailable. Build with: cargo build --release -p ocql-lsp"
    );
    return;
  }

  const run: Executable = {
    command: serverPath,
    options: { env: { ...process.env, RUST_LOG: "info" } },
  };

  const serverOptions: ServerOptions = { run, debug: run };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "ql" },
    ],
    synchronize: {
      fileEvents: workspace.createFileSystemWatcher("**/*.{ql,qll}"),
    },
  };

  client = new LanguageClient(
    "open-codeql",
    "Open CodeQL Language Server",
    serverOptions,
    clientOptions
  );

  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
