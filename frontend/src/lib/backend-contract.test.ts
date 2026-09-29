import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative, resolve } from 'node:path'
import { describe, expect, it } from 'vitest';
import { collectRoutes, renderInventory } from '../../../scripts/backend-inventory.mjs';

const ROOT = resolve(process.cwd(), '..')

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir)) {
    if (entry === 'node_modules' || entry === 'dist') continue;
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) out.push(...walk(full));
    else if (entry.endsWith('.ts') || entry.endsWith('.tsx')) out.push(full);
  }
  return out;
}

function normalise(path: string): string {
  return path
    .split('/')
    .map((segment) => {
      if (segment.includes('${') || /^\{.*\}$/.test(segment)) return '*';
      return segment;
    })
    .join('/');
}

function inventoryRoutes(): string[] {
  const routes: string[] = [];
  for (const set of collectRoutes().values()) {
    for (const entry of set) routes.push(entry);
  }
  return routes;
}

function frontendPaths(): { file: string; path: string }[] {
  const found: { file: string; path: string }[] = [];
  const libDir = join(ROOT, 'frontend/src/lib');
  for (const file of walk(libDir)) {
    if (file.endsWith('.test.ts') || file.endsWith('.test.tsx')) continue;
    const source = readFileSync(file, 'utf8');
    const pattern = /(['"`])(\/(?:api|health)[^'"`]*)\1/g;
    for (const match of source.matchAll(pattern)) {
      found.push({ file: relative(ROOT, file), path: match[2] });
    }
  }
  return found;
}

const KNOWN_FRONTEND_ONLY = new Set<string>();

describe('backend contract', () => {
  it('has a committed inventory that matches the current routes', () => {
    const onDisk = readFileSync(join(ROOT, 'docs/backend-inventory.md'), 'utf8');
    expect(renderInventory(collectRoutes())).toBe(onDisk);
  });

  it('only calls routes the backend registers', () => {
    const known = inventoryRoutes()
      .map((entry) => entry.split(' ').slice(1).join(' '))
      .filter((path) => !path.includes('{*'))
      .map(normalise);
    const unknown = frontendPaths()
      .map(({ file, path }) => ({ file, path, normalised: normalise(path) }))
      .filter(({ normalised }) => !known.includes(normalised) && !KNOWN_FRONTEND_ONLY.has(normalised));

    expect(
      unknown.map(({ file, path }) => `${file}: ${path}`),
    ).toEqual([]);
  });

  it('finds api paths to check', () => {
    expect(frontendPaths().length).toBeGreaterThan(5);
  });
});
