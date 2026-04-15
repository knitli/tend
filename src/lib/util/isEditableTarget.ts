// P1-C: decide whether a keydown event target is a text-input context in which
// global single-key shortcuts (e.g. `/` for filter focus) must be ignored.
//
// Covers:
//  - native <input> and <textarea> elements
//  - elements with `contenteditable="true"`
//  - xterm.js's hidden `.xterm-helper-textarea` (critical — without this the
//    shortcut would steal keystrokes from the embedded terminal)

export function isEditableTarget(target: EventTarget | null): boolean {
	if (!(target instanceof Element)) return false;
	if (target.closest(".xterm-helper-textarea")) return true;
	const tag = target.tagName;
	if (tag === "INPUT" || tag === "TEXTAREA") return true;
	if (target instanceof HTMLElement && target.isContentEditable) return true;
	// Fallback for environments (e.g. jsdom) where `isContentEditable` is not
	// populated: inspect the attribute directly.
	const ce = target.getAttribute("contenteditable");
	if (ce !== null && ce !== "false") return true;
	return false;
}
