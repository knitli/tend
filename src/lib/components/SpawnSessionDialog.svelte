<!--
  SpawnSessionDialog: modal form to launch a workbench-owned session.

  - When `lockedProject` is provided (e.g. opened from a per-project button),
    the project field is shown as a label, not a selector.
  - Otherwise the user picks from the active projects list.
  - Built-in presets cover common agents and dev commands; users can save
    additional presets to localStorage.
  - On success, calls onSpawned with the new session so the caller can
    activate it in the SplitView.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import type { Project } from '$lib/api/projects';
  import type { Session } from '$lib/api/sessions';
  import { projectsStore } from '$lib/stores/projects.svelte';
  import { sessionSpawn } from '$lib/api/sessions';

  interface Props {
    open: boolean;
    /** If set, the project field is locked to this project. */
    lockedProject?: Project | null;
    onClose: () => void;
    onSpawned?: (session: Session) => void;
  }

  let { open, lockedProject = null, onClose, onSpawned }: Props = $props();

  // ---------- Form state ----------

  let selectedProjectId = $state<number | null>(null);
  let commandText = $state('');
  let label = $state('');
  let workingDirectory = $state('');
  let showAdvanced = $state(false);
  let submitting = $state(false);
  let error = $state<string | null>(null);

  // Reset form whenever the dialog opens.
  $effect(() => {
    if (open) {
      selectedProjectId =
        lockedProject?.id ?? projectsStore.activeProjects[0]?.id ?? null;
      commandText = '';
      label = '';
      workingDirectory = '';
      showAdvanced = false;
      submitting = false;
      error = null;
    }
  });

  // ---------- Presets ----------

  interface Preset {
    label: string;
    command: string;
  }

  const BUILTIN_PRESETS: readonly Preset[] = [
    { label: 'claude', command: 'claude' },
    { label: 'claude --dsp', command: 'claude --dangerously-skip-permissions' },
    { label: 'codex', command: 'codex' },
    { label: 'aider', command: 'aider' },
    { label: 'npm run dev', command: 'npm run dev' },
    { label: 'pnpm dev', command: 'pnpm dev' },
  ];

  const SAVED_PRESETS_KEY = 'tend.spawn.presets';
  let savedPresets = $state<Preset[]>([]);

  onMount(() => {
    loadSavedPresets();
  });

  function loadSavedPresets(): void {
    try {
      const raw = localStorage.getItem(SAVED_PRESETS_KEY);
      if (!raw) return;
      const parsed: unknown = JSON.parse(raw);
      if (
        Array.isArray(parsed) &&
        parsed.every(
          (p): p is Preset =>
            typeof p === 'object' &&
            p !== null &&
            typeof (p as Preset).label === 'string' &&
            typeof (p as Preset).command === 'string',
        )
      ) {
        savedPresets = parsed;
      }
    } catch {
      // Corrupt localStorage; ignore.
    }
  }

  function persistSavedPresets(): void {
    try {
      localStorage.setItem(SAVED_PRESETS_KEY, JSON.stringify(savedPresets));
    } catch {
      // Quota exceeded or storage disabled; ignore.
    }
  }

  function applyPreset(preset: Preset): void {
    commandText = preset.command;
  }

  function saveCurrentAsPreset(): void {
    const command = commandText.trim();
    if (!command) return;
    const name = window.prompt('Name this preset:', command);
    if (!name) return;
    const trimmedName = name.trim();
    if (!trimmedName) return;
    // Replace if a saved preset with this name already exists.
    const next = savedPresets.filter((p) => p.label !== trimmedName);
    next.push({ label: trimmedName, command });
    savedPresets = next;
    persistSavedPresets();
  }

  function removeSavedPreset(event: MouseEvent, preset: Preset): void {
    event.preventDefault();
    event.stopPropagation();
    if (!window.confirm(`Remove preset "${preset.label}"?`)) return;
    savedPresets = savedPresets.filter((p) => p.label !== preset.label);
    persistSavedPresets();
  }

  // ---------- Command parsing ----------

  /**
   * POSIX-ish argv splitter. Handles single quotes (literal), double quotes
   * (escapes \ \" \\ \$ \`), backslash escapes outside quotes, and runs of
   * whitespace as separators. Throws on unterminated quotes.
   */
  function parseCommandLine(input: string): string[] {
    const out: string[] = [];
    let current = '';
    let inSingle = false;
    let inDouble = false;
    let hasContent = false;

    for (let i = 0; i < input.length; i++) {
      const c = input[i];

      if (inSingle) {
        if (c === "'") {
          inSingle = false;
        } else {
          current += c;
        }
        continue;
      }

      if (inDouble) {
        if (c === '\\' && i + 1 < input.length) {
          const next = input[i + 1];
          if (next === '"' || next === '\\' || next === '$' || next === '`') {
            current += next;
            i++;
            continue;
          }
        }
        if (c === '"') {
          inDouble = false;
        } else {
          current += c;
        }
        continue;
      }

      // Outside quotes.
      if (c === "'") {
        inSingle = true;
        hasContent = true;
        continue;
      }
      if (c === '"') {
        inDouble = true;
        hasContent = true;
        continue;
      }
      if (c === '\\' && i + 1 < input.length) {
        current += input[i + 1];
        i++;
        hasContent = true;
        continue;
      }
      if (c === ' ' || c === '\t' || c === '\n') {
        if (hasContent) {
          out.push(current);
          current = '';
          hasContent = false;
        }
        continue;
      }
      current += c;
      hasContent = true;
    }

    if (inSingle || inDouble) {
      throw new Error('Unterminated quoted string in command');
    }
    if (hasContent) {
      out.push(current);
    }
    return out;
  }

  // ---------- Submit ----------

  const canSubmit = $derived(
    selectedProjectId !== null && commandText.trim().length > 0 && !submitting,
  );

  async function handleSubmit(): Promise<void> {
    if (!canSubmit || selectedProjectId === null) return;
    error = null;

    let command: string[];
    try {
      command = parseCommandLine(commandText);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      return;
    }
    if (command.length === 0) {
      error = 'Command is empty';
      return;
    }

    submitting = true;
    try {
      // Don't pass cols/rows here: any estimate that overshoots what xterm
      // ends up with causes Claude to render an alt-screen taller/wider than
      // the visible buffer, clipping rows. The backend spawns at a
      // conservative default and xterm's onResize reconciles on mount.
      const result = await sessionSpawn({
        projectId: selectedProjectId,
        command,
        label: label.trim() || undefined,
        workingDirectory: workingDirectory.trim() || undefined,
      });
      onSpawned?.(result.session);
      onClose();
    } catch (e) {
      error = friendlyError(e);
    } finally {
      submitting = false;
    }
  }

  /** Map known backend error codes to actionable messages. */
  function friendlyError(e: unknown): string {
    const raw = e instanceof Error ? e.message : String(e);
    if (raw.includes('PROJECT_ARCHIVED')) {
      return 'This project is archived. Unarchive it first.';
    }
    if (raw.includes('WORKING_DIRECTORY_INVALID')) {
      return 'Working directory does not exist.';
    }
    if (raw.includes('SPAWN_FAILED')) {
      return 'Could not start the command. Check that it exists on your PATH.';
    }
    if (raw.includes('PATH_NOT_FOUND')) {
      return 'Path does not exist.';
    }
    return raw;
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
      return;
    }
    if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      handleSubmit();
    }
  }

  function handleBackdropClick(event: MouseEvent): void {
    // Only close if the click was on the backdrop itself, not a child.
    if (event.target === event.currentTarget) {
      onClose();
    }
  }
</script>

{#if open}
  <div
    class="backdrop"
    onclick={handleBackdropClick}
    onkeydown={handleKeydown}
    role="dialog"
    aria-modal="true"
    aria-labelledby="spawn-dialog-title"
    tabindex="-1"
  >
    <form
      class="dialog"
      onsubmit={(e) => {
        e.preventDefault();
        handleSubmit();
      }}
    >
      <header class="dialog-header">
        <h2 id="spawn-dialog-title">New session</h2>
        <button
          type="button"
          class="btn-icon"
          onclick={onClose}
          aria-label="Close"
        >×</button>
      </header>

      <div class="dialog-body">
        <label class="field">
          <span class="field-label">Project</span>
          {#if lockedProject}
            <div class="locked-project">
              <strong>{lockedProject.display_name}</strong>
              <span class="locked-path">{lockedProject.canonical_path}</span>
            </div>
          {:else}
            <select
              bind:value={selectedProjectId}
              class="input"
              required
            >
              {#each projectsStore.activeProjects as project (project.id)}
                <option value={project.id}>{project.display_name}</option>
              {/each}
            </select>
          {/if}
        </label>

        <label class="field">
          <span class="field-label">Command</span>
          <input
            type="text"
            bind:value={commandText}
            class="input mono"
            placeholder="claude --dangerously-skip-permissions"
            autocomplete="off"
            spellcheck="false"
            required
          />
        </label>

        <div class="presets-section">
          <span class="presets-label">Quick start:</span>
          <div class="presets-row">
            {#each BUILTIN_PRESETS as preset (preset.label)}
              <button
                type="button"
                class="preset-chip"
                onclick={() => applyPreset(preset)}
                title={preset.command}
              >{preset.label}</button>
            {/each}
            {#each savedPresets as preset (preset.label)}
              <button
                type="button"
                class="preset-chip preset-saved"
                onclick={() => applyPreset(preset)}
                oncontextmenu={(e) => removeSavedPreset(e, preset)}
                title={`${preset.command}  (right-click to remove)`}
              >{preset.label}</button>
            {/each}
            <button
              type="button"
              class="preset-chip preset-add"
              onclick={saveCurrentAsPreset}
              disabled={!commandText.trim()}
              title="Save current command as preset"
            >+ Save</button>
          </div>
        </div>

        <button
          type="button"
          class="advanced-toggle"
          onclick={() => (showAdvanced = !showAdvanced)}
          aria-expanded={showAdvanced}
        >
          {showAdvanced ? '▾' : '▸'} Advanced
        </button>

        {#if showAdvanced}
          <label class="field">
            <span class="field-label">Label</span>
            <input
              type="text"
              bind:value={label}
              class="input"
              placeholder="(defaults to 'session')"
              autocomplete="off"
            />
          </label>
          <label class="field">
            <span class="field-label">Working directory</span>
            <input
              type="text"
              bind:value={workingDirectory}
              class="input mono"
              placeholder="(defaults to project path)"
              autocomplete="off"
              spellcheck="false"
            />
          </label>
        {/if}

        {#if error}
          <p class="error" role="alert">{error}</p>
        {/if}
      </div>

      <footer class="dialog-footer">
        <button type="button" class="btn btn-ghost" onclick={onClose}>
          Cancel
        </button>
        <button type="submit" class="btn btn-primary" disabled={!canSubmit}>
          {submitting ? 'Starting…' : 'Start session'}
        </button>
      </footer>
    </form>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    padding: var(--space-4, 1rem);
  }

  .dialog {
    width: 100%;
    max-width: 560px;
    max-height: calc(100vh - 2rem);
    display: flex;
    flex-direction: column;
    background: var(--color-surface-raised, #15171c);
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 8px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
    overflow: hidden;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-3, 0.75rem) var(--space-4, 1rem);
    border-bottom: 1px solid var(--color-border, #2a2d35);
  }

  .dialog-header h2 {
    margin: 0;
    font-size: 0.9375rem;
    font-weight: 600;
    color: var(--color-text, #e6e8ef);
  }

  .btn-icon {
    width: 28px;
    height: 28px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 1.25rem;
    line-height: 1;
    cursor: pointer;
  }

  .btn-icon:hover {
    background: var(--color-surface-hover, #1e2028);
    color: var(--color-text, #e6e8ef);
  }

  .dialog-body {
    padding: var(--space-4, 1rem);
    display: flex;
    flex-direction: column;
    gap: var(--space-3, 0.75rem);
    overflow-y: auto;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field-label {
    font-size: 0.75rem;
    font-weight: 500;
    color: var(--color-text-muted, #8b8fa3);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .input {
    width: 100%;
    padding: var(--space-2, 0.5rem) var(--space-3, 0.75rem);
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 4px;
    background: var(--color-surface, #0f1115);
    color: var(--color-text, #e6e8ef);
    font-size: 0.8125rem;
    font-family: inherit;
  }

  .input.mono {
    font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
    font-size: 0.8125rem;
  }

  .input::placeholder {
    color: var(--color-text-muted, #8b8fa3);
  }

  .input:focus {
    outline: none;
    border-color: var(--color-accent, #60a5fa);
  }

  .locked-project {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--space-2, 0.5rem) var(--space-3, 0.75rem);
    background: var(--color-surface, #0f1115);
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 4px;
  }

  .locked-project strong {
    font-size: 0.875rem;
    color: var(--color-text, #e6e8ef);
  }

  .locked-path {
    font-size: 0.6875rem;
    font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
    color: var(--color-text-muted, #8b8fa3);
  }

  .presets-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .presets-label {
    font-size: 0.75rem;
    font-weight: 500;
    color: var(--color-text-muted, #8b8fa3);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .presets-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .preset-chip {
    padding: 3px 10px;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 12px;
    background: var(--color-surface, #0f1115);
    color: var(--color-text, #e6e8ef);
    font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
    font-size: 0.6875rem;
    cursor: pointer;
    transition: background 120ms, border-color 120ms;
  }

  .preset-chip:hover {
    background: var(--color-surface-hover, #1e2028);
    border-color: var(--color-accent, #60a5fa);
  }

  .preset-chip:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .preset-saved {
    border-color: var(--color-accent, #60a5fa);
  }

  .preset-add {
    font-family: inherit;
    color: var(--color-text-muted, #8b8fa3);
    border-style: dashed;
  }

  .advanced-toggle {
    align-self: flex-start;
    background: transparent;
    border: none;
    color: var(--color-text-muted, #8b8fa3);
    cursor: pointer;
    font-size: 0.75rem;
    padding: 0;
  }

  .advanced-toggle:hover {
    color: var(--color-text, #e6e8ef);
  }

  .error {
    margin: 0;
    padding: var(--space-2, 0.5rem) var(--space-3, 0.75rem);
    background: rgba(248, 113, 113, 0.1);
    border: 1px solid var(--color-error, #f87171);
    border-radius: 4px;
    color: var(--color-error, #f87171);
    font-size: 0.8125rem;
  }

  .dialog-footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2, 0.5rem);
    padding: var(--space-3, 0.75rem) var(--space-4, 1rem);
    border-top: 1px solid var(--color-border, #2a2d35);
  }

  .btn {
    padding: var(--space-2, 0.5rem) var(--space-4, 1rem);
    border: none;
    border-radius: 4px;
    font-size: 0.8125rem;
    font-family: inherit;
    cursor: pointer;
    transition: background 120ms;
  }

  .btn-primary {
    background: var(--color-accent, #60a5fa);
    color: var(--color-surface, #0f1115);
    font-weight: 500;
  }

  .btn-primary:hover:not(:disabled) {
    background: var(--color-accent-hover, #93c5fd);
  }

  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-ghost {
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
  }

  .btn-ghost:hover {
    background: var(--color-surface-hover, #1e2028);
    color: var(--color-text, #e6e8ef);
  }
</style>
