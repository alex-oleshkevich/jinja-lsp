import { test } from 'node:test';
import * as assert from 'node:assert/strict';
import { buildInitOptions, ConfigReader } from './init-options';

function fakeConfig(values: Record<string, unknown>, explicit: Set<string>): ConfigReader {
  return {
    get<T>(key: string): T | undefined {
      return values[key] as T | undefined;
    },
    inspect<T>(key: string) {
      if (!explicit.has(key)) return { key };
      return { key, workspaceValue: values[key] as T };
    },
  };
}

test('package.json defaults are not forwarded when the user never set them', () => {
  const config = fakeConfig(
    { templates: ['...'], extensions: ['html', 'jinja', 'jinja2', 'j2'] },
    new Set()
  );
  const opts = buildInitOptions(config);
  assert.deepEqual(opts, {});
});

test('explicitly-set values are forwarded even if they match the default', () => {
  const config = fakeConfig({ templates: ['src/tpl'] }, new Set(['templates']));
  const opts = buildInitOptions(config);
  assert.deepEqual(opts, { templates: ['src/tpl'] });
});

test('explicit empty arrays are still not forwarded', () => {
  const config = fakeConfig({ extensions: [] }, new Set(['extensions']));
  const opts = buildInitOptions(config);
  assert.deepEqual(opts, {});
});

test('inlinePatterns forwarded only when explicitly set', () => {
  const config = fakeConfig({ inlinePatterns: ['render_template_string'] }, new Set());
  const opts = buildInitOptions(config);
  assert.deepEqual(opts, {}, 'package.json default must not be forwarded');
});

test('explicitly-set inlinePatterns is forwarded as inline_patterns', () => {
  const config = fakeConfig(
    { inlinePatterns: ['render_template_string', 'render_jinja'] },
    new Set(['inlinePatterns'])
  );
  const opts = buildInitOptions(config);
  assert.deepEqual(opts, { inline_patterns: ['render_template_string', 'render_jinja'] });
});

test('lint select/ignore forwarded only when explicitly set', () => {
  const config = fakeConfig(
    { 'lint.select': ['JINJA-E1'], 'lint.ignore': [] },
    new Set(['lint.select'])
  );
  const opts = buildInitOptions(config);
  assert.deepEqual(opts, { lint: { select: ['JINJA-E1'], ignore: [] } });
});
