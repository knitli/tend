// T113: Light inline-markdown rendering using marked + DOMPurify.
// Block elements are disabled — only bold, italic, code, and links are rendered.
// M9 fix: links get target="_blank" rel="noopener noreferrer" to prevent
// navigation away from the Tauri app.

import { marked, Renderer } from 'marked';
import DOMPurify from 'dompurify';

// Custom renderer that opens links in external browser.
const renderer = new Renderer();
renderer.link = ({ href, text }) =>
  `<a href="${href}" target="_blank" rel="noopener noreferrer">${text}</a>`;

/**
 * Render a plain-text string with light inline markdown (bold, italic, code,
 * links). Block elements are stripped. Output is sanitized with DOMPurify.
 */
export function renderInlineMarkdown(text: string): string {
  const raw = marked.parseInline(text, { renderer }) as string;
  return DOMPurify.sanitize(raw, {
    ALLOWED_TAGS: ['strong', 'em', 'code', 'a', 'br'],
    ALLOWED_ATTR: ['href', 'target', 'rel'],
  });
}
