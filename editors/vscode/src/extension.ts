// REQ-EDIT-03/04/05: VS Code extension for jinja-lsp.
// Launches jinja-lsp lsp over stdio and forwards settings as InitializationOptions.

import * as vscode from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
  const config = vscode.workspace.getConfiguration('jinja-lsp');
  const serverPath = config.get<string>('server.path') || 'jinja-lsp';

  // REQ-EDIT-03: spawn jinja-lsp lsp over stdio.
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

  client = new LanguageClient('jinja-lsp', 'Jinja LSP', serverOptions, clientOptions);

  client.start().catch(() => {
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

  // REQ-EDIT-05: re-push settings on workspace/didChangeConfiguration.
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration(e => {
      if (e.affectsConfiguration('jinja-lsp') && client) {
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

// Build the InitializationOptions object that mirrors jinja.toml keys (REQ-EDIT-05).
function buildInitOptions(config: vscode.WorkspaceConfiguration): Record<string, unknown> {
  const opts: Record<string, unknown> = {};

  const templates = config.get<string[]>('templates');
  if (templates && templates.length > 0) opts['templates'] = templates;

  const extensions = config.get<string[]>('extensions');
  if (extensions && extensions.length > 0) opts['extensions'] = extensions;

  const extras = config.get<string[]>('extras');
  if (extras && extras.length > 0) opts['extras'] = extras;

  const customBuiltins = config.get<string[]>('customBuiltins');
  if (customBuiltins && customBuiltins.length > 0) opts['custom_builtins'] = customBuiltins;

  const hints = config.get<string[]>('hints');
  if (hints && hints.length > 0) opts['hints'] = hints;

  const select = config.get<string[]>('lint.select');
  const ignore = config.get<string[]>('lint.ignore');
  if ((select && select.length > 0) || (ignore && ignore.length > 0)) {
    opts['lint'] = { select: select ?? [], ignore: ignore ?? [] };
  }

  return opts;
}
