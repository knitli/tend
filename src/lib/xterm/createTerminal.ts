// T030: xterm.js factory. Centralizes the configuration of the embedded
// terminal so every pane that mounts one (`AgentPane`, `CompanionPane`,
// and the Phase 2 smoke test) has the same look and disposal semantics.

import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { type ITerminalOptions, Terminal } from "@xterm/xterm";

import "@xterm/xterm/css/xterm.css";

/** A mounted terminal plus its lifecycle hooks. */
export interface CreatedTerminal {
	readonly terminal: Terminal;
	readonly fit: FitAddon;
	/** Call on component unmount to free xterm + addons. */
	dispose(): void;
}

const defaultOptions: ITerminalOptions = {
	convertEol: true,
	cursorBlink: true,
	fontFamily: "var(--font-mono), ui-monospace, monospace",
	fontSize: 13,
	theme: {
		background: "#0f1115",
		foreground: "#e6e8ef",
		cursor: "#60a5fa",
		cursorAccent: "#0f1115",
		selectionBackground: "#3b82f644",
	},
	scrollback: 5000,
};

/**
 * Create and mount a new xterm terminal into `container`.
 *
 * The returned `dispose()` function tears down the terminal and all addons;
 * call it when the Svelte host component unmounts.
 */
export function createTerminal(
	container: HTMLElement,
	options?: ITerminalOptions,
): CreatedTerminal {
	const terminal = new Terminal({ ...defaultOptions, ...options });
	const fit = new FitAddon();
	const webLinks = new WebLinksAddon();

	terminal.loadAddon(fit);
	terminal.loadAddon(webLinks);

	terminal.open(container);
	fit.fit();

	const resizeObserver = new ResizeObserver(() => {
		try {
			fit.fit();
		} catch {
			// Ignore transient fit errors during teardown.
		}
	});
	resizeObserver.observe(container);

	return {
		terminal,
		fit,
		dispose() {
			resizeObserver.disconnect();
			terminal.dispose();
		},
	};
}
