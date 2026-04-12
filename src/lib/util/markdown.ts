// T113: Light inline-markdown rendering using marked + DOMPurify.
// Block elements are disabled — only bold, italic, code, and links are rendered.

import { marked } from 'marked';
import DOMPurify from 'dompurify';

/**
 * Render a plain-text string with light inline markdown (bold, italic, code,
 * links). Block elements are stripped. Output is sanitized with DOMPurify.
 */
export function renderInlineMarkdown(text: string): string {
  // Use parseInline to avoid block-level wrapping (<p>, <h1>, etc.).
  const raw = marked.parseInline(text) as string;
  return DOMPurify.sanitize(raw, {
    ALLOWED_TAGS: ['strong', 'em', 'code', 'a', 'br'],
    ALLOWED_ATTR: ['href', 'target', 'rel'],
  });
}
