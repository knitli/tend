// T030: xterm.js factory. Centralizes the configuration of the embedded
// terminal so every pane that mounts one (`AgentPane`, `CompanionPane`,
// and the Phase 2 smoke test) has the same look and disposal semantics.

import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { WebglAddon } from "@xterm/addon-webgl";
import { type ITerminalOptions, Terminal } from "@xterm/xterm";

import "@xterm/xterm/css/xterm.css";

/** A mounted terminal plus its lifecycle hooks. */
export interface CreatedTerminal {
	readonly terminal: Terminal;
	readonly fit: FitAddon;
	/** Call on component unmount to free xterm + addons. */
	dispose(): void;
}

// Nerd-Font-aware font stack. Most CLI agents (Claude, Codex, aider) and
// most users' shell prompts use Nerd Font glyphs (powerline symbols, file-type
// icons, etc.). Prefer common Nerd Font families in order — the browser falls
// through until it finds one installed locally. `Symbols Nerd Font` is the
// glyph-only overlay font that works as a fallback next to any monospace.
const NERD_FONT_STACK = [
	'"Iosevka Nerd Font"',
	'"JetBrainsMono Nerd Font"',
	'"FiraCode Nerd Font"',
	'"Hack Nerd Font"',
	'"MesloLGS Nerd Font"',
	'"Symbols Nerd Font"',
	'"Symbols Nerd Font Mono"',
	"ui-monospace",
	"monospace",
].join(", ");

const defaultOptions: ITerminalOptions = {
	convertEol: true,
	cursorBlink: true,
	fontFamily: NERD_FONT_STACK,
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

	// GPU-accelerated renderer. The default canvas renderer repaints on the
	// CPU; under WebKitGTK (Tauri on Linux) that translates to real fan-worthy
	// load for TUIs that emit 20+ frames/sec. WebGL offloads the raster work
	// to the GPU. If the GPU context fails (context-lost, driver missing,
	// headless env, WSL without GPU pass-through), fall back silently to the
	// default renderer — functionality is identical, just slower.
	let webgl: WebglAddon | undefined;
	try {
		webgl = new WebglAddon();
		webgl.onContextLoss(() => {
			// Dispose on context loss so xterm falls back to canvas rendering
			// rather than displaying a frozen scene.
			webgl?.dispose();
		});
		terminal.loadAddon(webgl);
	} catch {
		webgl = undefined;
	}

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
			webgl?.dispose();
			terminal.dispose();
		},
	};
}
