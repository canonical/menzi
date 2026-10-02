import { useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent } from 'react';
import { Button, Icon } from '@canonical/react-components';
import { S } from '../../strings/catalogue';

interface FileNode {
  name: string;
  path: string;
  children: FileNode[];
  directory: boolean;
}

function buildTree(files: string[]): FileNode[] {
  const roots: FileNode[] = [];
  for (const file of files) {
    let children = roots;
    const parts = file.split('/');
    parts.forEach((name, index) => {
      const path = parts.slice(0, index + 1).join('/');
      const directory = index < parts.length - 1;
      let node = children.find((entry) => entry.path === path && entry.directory === directory);
      if (!node) {
        node = { name, path, directory, children: [] };
        children.push(node);
      }
      children = node.children;
    });
  }
  const sort = (nodes: FileNode[]): FileNode[] => nodes
    .sort((a, b) => Number(b.directory) - Number(a.directory) || a.name.localeCompare(b.name))
    .map((node) => ({ ...node, children: sort(node.children) }));
  return sort(roots);
}

function ancestors(path: string): string[] {
  const parts = path.split('/');
  return parts.slice(0, -1).map((_, index) => parts.slice(0, index + 1).join('/'));
}

export function ReviewFileTree({
  files,
  selected,
  onSelect,
}: {
  files: string[];
  selected: string | null;
  onSelect: (file: string) => void;
}) {
  const nodes = useMemo(() => buildTree(files), [files]);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [focused, setFocused] = useState<string | null>(null);
  const treeRef = useRef<HTMLUListElement>(null);

  useEffect(() => {
    if (!selected) return;
    setCollapsed((previous) => {
      const next = new Set(previous);
      ancestors(selected).forEach((path) => next.delete(path));
      return next;
    });
  }, [selected]);

  const toggle = (path: string) => setCollapsed((previous) => {
    const next = new Set(previous);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    return next;
  });

  const focusItem = (item?: HTMLElement) => {
    if (!item) return;
    setFocused(item.dataset.path ?? null);
    item.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLUListElement>) => {
    const items = Array.from(treeRef.current?.querySelectorAll<HTMLElement>('[role="treeitem"]') ?? []);
    const current = items.find((item) => item === document.activeElement);
    if (!current) return;
    const index = items.indexOf(current);
    const path = current.dataset.path ?? '';
    const directory = current.hasAttribute('aria-expanded');
    switch (event.key) {
      case 'ArrowDown':
        focusItem(items[index + 1]);
        break;
      case 'ArrowUp':
        focusItem(items[index - 1]);
        break;
      case 'Home':
        focusItem(items[0]);
        break;
      case 'End':
        focusItem(items.at(-1));
        break;
      case 'ArrowRight':
        if (directory && collapsed.has(path)) toggle(path);
        else if (directory) focusItem(items[index + 1]);
        break;
      case 'ArrowLeft':
        if (directory && !collapsed.has(path)) toggle(path);
        else focusItem(items.find((item) => item.dataset.path === ancestors(path).at(-1)));
        break;
      default:
        return;
    }
    event.preventDefault();
  };

  const visiblePaths: string[] = [];
  const collect = (entries: FileNode[]) => entries.forEach((node) => {
    visiblePaths.push(node.path);
    if (node.directory && !collapsed.has(node.path)) collect(node.children);
  });
  collect(nodes);
  const tabPath = focused && visiblePaths.includes(focused)
    ? focused
    : selected && visiblePaths.includes(selected) ? selected : visiblePaths[0];

  const renderNodes = (entries: FileNode[], level: number) => entries.map((node) => (
    <li key={node.path} role="none">
      <Button
        appearance="base"
        className="app-review-tree__item"
        role="treeitem"
        aria-label={node.path}
        aria-level={level}
        aria-expanded={node.directory ? !collapsed.has(node.path) : undefined}
        aria-selected={!node.directory ? node.path === selected : undefined}
        data-path={node.path}
        tabIndex={node.path === tabPath ? 0 : -1}
        title={node.path}
        style={{ '--tree-level': level - 1 } as CSSProperties}
        onFocus={() => setFocused(node.path)}
        onClick={() => {
          if (node.directory) toggle(node.path);
          else onSelect(node.path);
        }}
      >
        <span className="app-review-tree__disclosure" aria-hidden="true">
          {node.directory ? <Icon name="chevron" className={collapsed.has(node.path) ? 'app-review-tree__chevron--closed' : ''} /> : null}
        </span>
        {node.directory ? (
          <svg className="app-review-tree__icon" viewBox="0 0 20 20" fill="none" aria-hidden="true">
            <path d="M2.5 5h5l2 2h8v9h-15z" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" />
          </svg>
        ) : <FileIcon file={node.name} />}
        <span className="app-review-tree__name">{node.name}</span>
      </Button>
      {node.directory && !collapsed.has(node.path) ? (
        <ul className="app-review-tree__group" role="group">{renderNodes(node.children, level + 1)}</ul>
      ) : null}
    </li>
  ));

  return (
    <ul ref={treeRef} className="app-review__files app-review-tree" role="tree" aria-label={S.review.filesLabel} onKeyDown={onKeyDown}>
      {renderNodes(nodes, 1)}
    </ul>
  );
}

function FileIcon({ file }: { file: string }) {
  const extension = file.includes('.') ? file.split('.').at(-1)!.toLowerCase() : '';
  const badges: Record<string, string> = {
    ts: 'TS', tsx: 'TS', js: 'JS', jsx: 'JS', mjs: 'JS', cjs: 'JS',
    rs: 'RS', py: 'PY', go: 'GO', rb: 'RB', java: 'J', c: 'C', h: 'C', cpp: 'C+', cs: 'C#',
    css: '#', scss: '#', sass: '#', less: '#', html: '<>', vue: 'V', svelte: 'S',
    json: '{}', yaml: '{}', yml: '{}', toml: '{}', xml: '<>', sql: 'DB',
    md: 'M↓', mdx: 'M↓', txt: '≡', sh: '>_', bash: '>_', zsh: '>_',
  };
  const image = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'ico', 'avif'].includes(extension);
  const badge = badges[extension];
  const kind = image ? 'image' : ['css', 'scss', 'sass', 'less'].includes(extension)
    ? 'style' : ['json', 'yaml', 'yml', 'toml', 'xml'].includes(extension)
      ? 'data' : ['md', 'mdx', 'txt'].includes(extension) ? 'document' : badge ? 'code' : 'file';
  return (
    <svg className={`app-review-tree__icon app-review-tree__icon--${kind}`} viewBox="0 0 20 20" fill="none" aria-hidden="true" data-extension={extension}>
      {image ? (
        <>
          <rect x="2.5" y="3.5" width="15" height="13" rx="1" stroke="currentColor" strokeWidth="1.3" />
          <circle cx="6.5" cy="7.5" r="1.5" fill="currentColor" />
          <path d="m3 15 5-5 3 3 3-4 3 6" stroke="currentColor" strokeWidth="1.3" />
        </>
      ) : badge ? (
        <text x="10" y="13" fill="currentColor" textAnchor="middle" fontSize="8" fontWeight="700" fontFamily="monospace">{badge}</text>
      ) : (
        <path d="M5 2.5h6l4 4v11H5zm6 0v4h4" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round" />
      )}
    </svg>
  );
}
