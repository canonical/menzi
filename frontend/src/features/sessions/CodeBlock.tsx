import { useEffect, useState } from 'react';
import { highlight, loadHighlighter, splitHighlightedLines } from '../../lib/diff/highlight';

const MAX_HIGHLIGHT_CHARS = 120_000;

export function CodeBlock({ className, text }: { className?: string; text: string }) {
  const [markup, setMarkup] = useState<string[] | null>(null);
  const language = languageOf(className);

  useEffect(() => {
    let live = true;
    if (text.length > MAX_HIGHLIGHT_CHARS) {
      setMarkup(null);
      return () => {
        live = false;
      };
    }
    void loadHighlighter().then(() => {
      if (!live) return;
      setMarkup(splitHighlightedLines(highlight(text, language)));
    });
    return () => {
      live = false;
    };
  }, [text, language]);

  const inline = !className;

  if (inline) {
    return (
      <code
        className="app-md__code"
        dangerouslySetInnerHTML={{ __html: markup?.[0] ?? escapeHtml(text) }}
      />
    );
  }

  return (
    <code className="app-md__pre">
      {markup ? (
        markup.map((line, index) => (
          <span
            className="app-md__line"
            dangerouslySetInnerHTML={{ __html: line || ' ' }}
            key={index}
          />
        ))
      ) : (
        <span className="app-md__line">{text}</span>
      )}
    </code>
  );
}

function languageOf(className?: string): string | null {
  const match = /language-([\w+-]+)/.exec(className ?? '');
  return match ? match[1].toLowerCase() : null;
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}