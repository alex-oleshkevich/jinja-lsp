// Split out from extension.ts so this logic can be unit-tested without
// depending on the real `vscode` module, which only resolves inside the
// extension host.

export interface ConfigInspection<T> {
  key: string;
  defaultValue?: T;
  globalValue?: T;
  workspaceValue?: T;
  workspaceFolderValue?: T;
  globalLanguageValue?: T;
  workspaceLanguageValue?: T;
  workspaceFolderLanguageValue?: T;
}

export interface ConfigReader {
  get<T>(key: string): T | undefined;
  inspect<T>(key: string): ConfigInspection<T> | undefined;
}

// A setting only counts as "explicitly set" if the user configured it somewhere
// (user/workspace/folder, plain or language-scoped) rather than inheriting the
// package.json default — otherwise VS Code's non-empty defaults for `templates`
// and `extensions` would always win over jinja.toml (REQ-EDIT-05).
export function isExplicitlySet(config: ConfigReader, key: string): boolean {
  const inspected = config.inspect(key);
  if (!inspected) return false;
  return (
    inspected.globalValue !== undefined ||
    inspected.workspaceValue !== undefined ||
    inspected.workspaceFolderValue !== undefined ||
    inspected.globalLanguageValue !== undefined ||
    inspected.workspaceLanguageValue !== undefined ||
    inspected.workspaceFolderLanguageValue !== undefined
  );
}

// Build the InitializationOptions object that mirrors jinja.toml keys (REQ-EDIT-05).
export function buildInitOptions(config: ConfigReader): Record<string, unknown> {
  const opts: Record<string, unknown> = {};

  const forwardArray = (key: string, mapKey: string) => {
    if (!isExplicitlySet(config, key)) return;
    const value = config.get<string[]>(key);
    if (value && value.length > 0) opts[mapKey] = value;
  };

  forwardArray('templates', 'templates');
  forwardArray('extensions', 'extensions');
  forwardArray('extras', 'extras');
  forwardArray('customBuiltins', 'custom_builtins');
  forwardArray('hints', 'hints');
  forwardArray('inlinePatterns', 'inline_patterns');

  if (isExplicitlySet(config, 'lint.select') || isExplicitlySet(config, 'lint.ignore')) {
    const select = config.get<string[]>('lint.select');
    const ignore = config.get<string[]>('lint.ignore');
    if ((select && select.length > 0) || (ignore && ignore.length > 0)) {
      opts['lint'] = { select: select ?? [], ignore: ignore ?? [] };
    }
  }

  return opts;
}
