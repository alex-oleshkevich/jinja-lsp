// REQ-EDIT-03/04/05: VS Code extension for jinja-lsp.
// Launches jinja-lsp lsp over stdio and forwards settings as InitializationOptions.

import * as vscode from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from 'vscode-languageclient/node';
import { buildInitOptions } from './init-options';

let client: LanguageClient | undefined;

// REQ-EDIT-03: build a fresh client from the current settings.
function createClient(): LanguageClient {
  const config = vscode.workspace.getConfiguration('jinja-lsp');
  const serverPath = config.get<string>('server.path') || 'jinja-lsp';

  const serverOptions: ServerOptions = {
    command: serverPath,
    args: ['lsp'],
    transport: TransportKind.stdio,
  };

  // REQ-EDIT-05: forward VS Code settings as InitializationOptions.
  const initializationOptions = buildInitOptions(config);

  const clientOptions: LanguageClientOptions = {
    // REQ-EDIT-11: canonical languageIds jinja and jinja-html.
    documentSelector: [
      { scheme: 'file', language: 'jinja' },
      { scheme: 'file', language: 'jinja-html' },
    ],
    initializationOptions,
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher('**/jinja.toml'),
    },
  };

  return new LanguageClient('jinja-lsp', 'Jinja LSP', serverOptions, clientOptions);
}

function startClient(): void {
  client?.start().catch(() => {
    // REQ-EDIT-03: surface not-found toast with install instructions.
    vscode.window.showErrorMessage(
      'jinja-lsp not found.\n\n' +
        'Install it:\n  pip install jinja-lsp  |  uv tool install jinja-lsp  |  cargo install jinja-lsp\n\n' +
        'Or set jinja-lsp.server.path to the binary location.',
      'Open Settings',
      'Dismiss'
    ).then(choice => {
      if (choice === 'Open Settings') {
        vscode.commands.executeCommand('workbench.action.openSettings', 'jinja-lsp.server.path');
      }
    });
  });
}

export function activate(context: vscode.ExtensionContext): void {
  client = createClient();
  startClient();

  // jinja-lsp-x6us: server.path selects the spawned binary, so changing it can't
  // take effect via a settings notification alone — the client (and its child
  // process) must be stopped and a new one started against the new path.
  // Every other jinja-lsp.* setting still just re-pushes InitializationOptions.
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration(async e => {
      if (!e.affectsConfiguration('jinja-lsp')) {
        return;
      }
      if (e.affectsConfiguration('jinja-lsp.server.path')) {
        await client?.stop();
        client = createClient();
        startClient();
        return;
      }
      if (client) {
        const updated = vscode.workspace.getConfiguration('jinja-lsp');
        client.sendNotification('workspace/didChangeConfiguration', {
          settings: buildInitOptions(updated),
        });
      }
    })
  );
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}

