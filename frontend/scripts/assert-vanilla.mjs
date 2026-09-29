import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const distDir = fileURLToPath(new URL('../dist/assets/', import.meta.url));
const files = readdirSync(distDir).filter((f) => f.endsWith('.css'));

if (files.length === 0) {
  console.error('assert-vanilla: no CSS emitted in dist/assets');
  process.exit(1);
}

const css = files.map((f) => readFileSync(join(distDir, f), 'utf8')).join('\n');

const sentinels = [
  '.p-button--positive',
  '.p-button--negative',
  '.p-card',
  '.p-table',
  '.l-application',
  '.p-tabs__link',
  '.p-form__control',
];

const missing = sentinels.filter((s) => !css.includes(s));

if (missing.length > 0) {
  console.error('assert-vanilla: Vanilla Framework is missing from the build.');
  console.error('  missing:', missing.join(', '));
  console.error('  src/styles/index.scss must contain:');
  console.error('    @import "vanilla-framework/scss/_vanilla";');
  console.error('    @include vanilla;');
  process.exit(1);
}

const minBytes = 300 * 1024;
const total = files.reduce((n, f) => n + readFileSync(join(distDir, f)).length, 0);

if (total < minBytes) {
  console.error(`assert-vanilla: emitted CSS is ${total} bytes, expected at least ${minBytes}.`);
  console.error('  Vanilla Framework is not being compiled into the bundle.');
  process.exit(1);
}

console.log(`assert-vanilla: ok (${files.length} file(s), ${total} bytes, ${sentinels.length} sentinels present)`);
