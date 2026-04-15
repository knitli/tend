<!-- T063: Project sidebar. Lists registered projects with an "Add Project"
     action and an archived toggle. Dispatches project selection events. -->
<script lang="ts">
  import { Collapsible } from 'bits-ui';
  import { projectsStore } from '$lib/stores/projects.svelte';
  import type { Project } from '$lib/api/projects';
  import ColorSwatchPicker from '$lib/components/ColorSwatchPicker.svelte';
  import { getProjectColor } from '$lib/util/projectColor';

  interface Props {
    selectedProjectId?: number | null;
    onSelectProject?: (project: Project) => void;
    onSpawnSession?: (project: Project) => void;
    /** P3-A: controlled open state driven by the parent (so the hamburger
     *  button, which lives OUTSIDE this component, can toggle it). */
    open?: boolean;
    /** Stable id for the Collapsible.Content element so HamburgerButton can
     *  point its `aria-controls` at it. */
    contentId?: string;
    /** P3-A: when the sidebar is collapsed, the parent sets `peeking=true` on
     *  hover-peek. The sidebar element takes `position: absolute` over the
     *  content area so it does not compress the content while peeking. */
    peeking?: boolean;
    /** P3-A (review fix): hover handlers bound directly to the `<aside>` so
     *  the cursor crossing from the left-edge hotzone onto the peeked sidebar
     *  body keeps the peek alive. A separate "peek-zone" div was occluded by
     *  the sidebar overlay (z-index) and its events never fired in practice. */
    onPeekEnter?: () => void;
    onPeekLeave?: () => void;
  }

  let {
    selectedProjectId = null,
    onSelectProject,
    onSpawnSession,
    open = true,
    contentId = 'sidebar-collapsible-content',
    peeking = false,
    onPeekEnter,
    onPeekLeave,
  }: Props = $props();

  /** Phase 2-B: id of the project whose colour picker is currently open, or
   *  `null` if none. Only one picker is open at a time. */
  let pickerProjectId = $state<number | null>(null);
  /** Phase 2 review fix (Fix 3): the swatch button that opened the currently-
   *  active picker. Passed into `ColorSwatchPicker` as an ignore target so the
   *  document-click-capture handler doesn't race with the swatch's own
   *  `onclick`. Without this, re-clicking the open swatch toggled: capture
   *  closed it (`pickerProjectId = null`), then `openPicker` reopened it. */
  let pickerSwatchEl = $state<HTMLButtonElement | null>(null);

  /** Phase 2 review fix (Fix 1): debounce drag writes.
   *  `vanilla-colorful` fires `color-changed` on every pointer-move (60-200 Hz).
   *  Without debouncing, each move hits `projectsStore.update` → Tauri IPC →
   *  SQLite UPDATE and races the workspace store. We mirror the dragged colour
   *  into `pendingColor` immediately (the picker UI stays in sync) and only
   *  flush the write after a 200 ms trailing idle. Matches SessionList's
   *  filter-debounce style; no shared utility by design. */
  let pendingColor = $state<Record<number, string>>({});
  let pendingColorTimer: ReturnType<typeof setTimeout> | null = null;
  let pendingColorProject: Project | null = null;

  function openPicker(event: MouseEvent, projectId: number): void {
    event.stopPropagation();
    if (pickerProjectId === projectId) {
      // Re-clicking the open swatch explicitly toggles it closed.
      closePicker();
      return;
    }
    // Flush any in-flight debounced write for the previously-open project
    // before switching, so rapid project switching can't drop colour updates.
    if (pendingColorProject !== null) {
      flushPendingColor();
    }
    pickerProjectId = projectId;
    pickerSwatchEl = event.currentTarget as HTMLButtonElement;
  }

  function flushPendingColor(): void {
    if (pendingColorTimer !== null) {
      clearTimeout(pendingColorTimer);
      pendingColorTimer = null;
    }
    const project = pendingColorProject;
    const hex = project ? pendingColor[project.id] : undefined;
    pendingColorProject = null;
    if (project && typeof hex === 'string') {
      // Fire-and-forget; the store surfaces errors via its `.error` field.
      void projectsStore.update(project.id, {
        settings: { ...project.settings, color: hex },
      });
    }
  }

  function closePicker(): void {
    // Commit any in-progress drag before hiding the picker.
    flushPendingColor();
    pickerProjectId = null;
    pickerSwatchEl = null;
  }

  function handleColorChange(project: Project, hex: string): void {
    // Optimistic local view: the picker reads from `pendingColor` so its
    // drag-handle positions reflect every tick, even while the DB write is
    // debounced. The displayed hex text also updates immediately.
    pendingColor = { ...pendingColor, [project.id]: hex };
    pendingColorProject = project;
    if (pendingColorTimer !== null) clearTimeout(pendingColorTimer);
    pendingColorTimer = setTimeout(() => {
      pendingColorTimer = null;
      flushPendingColor();
    }, 200);
  }

  let showArchived = $state(false);
  let addingProject = $state(false);
  let newProjectPath = $state('');
  let newProjectName = $state('');

  const displayedProjects = $derived(
    showArchived
      ? projectsStore.projects
      : projectsStore.activeProjects,
  );

  async function handleAddProject(): Promise<void> {
    if (!newProjectPath.trim()) return;

    const project = await projectsStore.register(
      newProjectPath.trim(),
      newProjectName.trim() || undefined,
    );

    if (project) {
      newProjectPath = '';
      newProjectName = '';
      addingProject = false;
      onSelectProject?.(project);
    }
  }

  function handleSelectProject(project: Project): void {
    onSelectProject?.(project);
  }

  function handleKeydown(event: KeyboardEvent, project: Project): void {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      handleSelectProject(project);
    }
  }

  async function handleArchive(
    event: MouseEvent,
    projectId: number,
  ): Promise<void> {
    event.stopPropagation();
    await projectsStore.archive(projectId);
  }

  async function handleUnarchive(
    event: MouseEvent,
    projectId: number,
  ): Promise<void> {
    event.stopPropagation();
    await projectsStore.unarchive(projectId);
  }

  let copiedProjectId = $state<number | null>(null);
  let copyTimer: ReturnType<typeof setTimeout> | null = null;

  async function handleCopyRunCommand(
    event: MouseEvent,
    project: Project,
  ): Promise<void> {
    event.stopPropagation();
    const command = `tend run --project "${project.canonical_path}" -- claude`;
    try {
      await navigator.clipboard.writeText(command);
      copiedProjectId = project.id;
      if (copyTimer) clearTimeout(copyTimer);
      copyTimer = setTimeout(() => { copiedProjectId = null; }, 1500);
    } catch {
      // Clipboard API unavailable; fail silently.
    }
  }
</script>

<Collapsible.Root {open}>
  <Collapsible.Content
    id={contentId}
    forceMount
    class="sidebar-collapsible"
    data-peeking={peeking ? 'true' : 'false'}
  >
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <aside
      class="sidebar"
      role="navigation"
      aria-label="Projects"
      aria-hidden={!open && !peeking}
      inert={!open && !peeking}
      style:pointer-events={!open && !peeking ? 'none' : undefined}
      onmouseenter={() => onPeekEnter?.()}
      onmouseleave={() => onPeekLeave?.()}
    >
  <header class="sidebar-header">

    <h2>Projects</h2>
    <button
      class="btn-icon"
      title="Add project"
      aria-label="Add project"
      onclick={() => (addingProject = !addingProject)}
    >
      {addingProject ? '−' : '+'}
    </button>
  </header>

  {#if addingProject}
    <form class="add-project-form" onsubmit={(e) => { e.preventDefault(); handleAddProject(); }}>
      <input
        type="text"
        placeholder="Project path..."
        bind:value={newProjectPath}
        class="input"
        required
        aria-label="Project path"
      />
      <input
        type="text"
        placeholder="Display name (optional)"
        bind:value={newProjectName}
        class="input"
        aria-label="Display name"
      />
      <div class="form-actions">
        <button type="submit" class="btn btn-primary" disabled={!newProjectPath.trim()}>
          Add
        </button>
        <button
          type="button"
          class="btn btn-ghost"
          onclick={() => { addingProject = false; newProjectPath = ''; newProjectName = ''; }}
        >
          Cancel
        </button>
      </div>
    </form>
  {/if}

  {#if projectsStore.error}
    <p class="error-message" role="alert">{projectsStore.error}</p>
  {/if}

  <nav class="project-list" aria-label="Project list">
    {#if projectsStore.loading}
      <p class="loading">Loading projects...</p>
    {:else if displayedProjects.length === 0}
      <p class="empty">
        {showArchived ? 'No projects found.' : 'No active projects. Add one to get started.'}
      </p>
    {:else}
      <ul role="listbox" aria-label="Projects">
        {#each displayedProjects as project (project.id)}
          {@const projectColor = getProjectColor(project)}
          <li
            role="option"
            aria-selected={selectedProjectId === project.id}
            class="project-item"
            class:selected={selectedProjectId === project.id}
            class:archived={project.archived_at !== null}
            style={projectColor ? `--project-color: ${projectColor}` : ''}
            tabindex="0"
            onclick={() => handleSelectProject(project)}
            onkeydown={(e) => handleKeydown(e, project)}
          >
            <div class="project-info">
              <span class="project-name">{project.display_name}</span>
              <span class="project-path" title={project.canonical_path}>
                {project.canonical_path}
              </span>
            </div>
            <div class="project-actions">
              <!-- Phase 2-B: Colour swatch. Always visible (60% opacity at
                   rest, full opacity on hover) so the project's identity
                   colour is readable at a glance even without hovering. -->
              <button
                class="color-swatch"
                title="Change project colour"
                aria-label="Change colour for {project.display_name}"
                onclick={(e) => openPicker(e, project.id)}
              ></button>
              {#if pickerProjectId === project.id}
                <ColorSwatchPicker
                  value={pendingColor[project.id] ?? projectColor ?? '#60a5fa'}
                  ignoreEl={pickerSwatchEl}
                  onChange={(hex) => handleColorChange(project, hex)}
                  onClose={closePicker}
                />
              {/if}
              {#if !project.archived_at}
                <button
                  class="btn-icon btn-sm"
                  title="Start a session in this project"
                  aria-label="Start a session in {project.display_name}"
                  onclick={(e) => {
                    e.stopPropagation();
                    onSpawnSession?.(project);
                  }}
                >
                  ▶
                </button>
                <button
                  class="btn-icon btn-sm"
                  class:copied={copiedProjectId === project.id}
                  title={copiedProjectId === project.id
                    ? 'Copied!'
                    : `Copy: tend run --project "${project.canonical_path}" -- claude`}
                  aria-label="Copy tend run command for {project.display_name}"
                  onclick={(e) => handleCopyRunCommand(e, project)}
                >
                  {copiedProjectId === project.id ? '✓' : '⧉'}
                </button>
              {/if}
              {#if project.archived_at}
                <button
                  class="btn-icon btn-sm"
                  title="Unarchive"
                  aria-label="Unarchive {project.display_name}"
                  onclick={(e) => handleUnarchive(e, project.id)}
                >
                  ↩
                </button>
              {:else}
                <button
                  class="btn-icon btn-sm"
                  title="Archive"
                  aria-label="Archive {project.display_name}"
                  onclick={(e) => handleArchive(e, project.id)}
                >
                  ⊘
                </button>
              {/if}
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </nav>

  <footer class="sidebar-footer">
    <label class="toggle-label">
      <input
        type="checkbox"
        bind:checked={showArchived}
        aria-label="Show archived projects"
      />
      Show archived
    </label>
  </footer>
    </aside>
  </Collapsible.Content>
</Collapsible.Root>

<style>
  /* P3-A: The Collapsible.Content wrapper is the element that animates between
     the 260 px open state and the 0 px collapsed state. bits-ui emits
     `data-state="open" | "closed"` attributes on the Content element — we
     target those directly rather than tracking a separate class.

     `overflow: hidden` prevents the sidebar's children from painting during
     the transition; `white-space: nowrap` + `min-width: 0` on descendants is
     handled naturally by the existing flex layout.

     `:global()` is required here because bits-ui renders `.sidebar-collapsible`
     outside Svelte's component-scoped style boundary. */
  :global(.sidebar-collapsible) {
    display: flex;
    flex-direction: column;
    height: 100%;
    flex-shrink: 0;
    overflow: hidden;
    transition:
      width 200ms ease,
      border-right-color 200ms ease;
  }

  :global(.sidebar-collapsible[data-state="open"]) {
    width: 260px;
    border-right: 1px solid var(--color-border, #2a2d35);
  }

  :global(.sidebar-collapsible[data-state="closed"]) {
    position: absolute;
    top: 0;
    left: 0;
    height: 100%;
    width: 0;
    z-index: 50;
    border-right: 1px solid transparent;
  }

  /* Peek overlay: while the collapsed sidebar is being hovered/peeked, expand
     the already out-of-flow wrapper to the usual 260 px width so the main
     content never gets compressed during either the peek-open or peek-close
     transition. */
  :global(.sidebar-collapsible[data-state="closed"][data-peeking="true"]) {
    width: 260px;
    border-right: 1px solid var(--color-border, #2a2d35);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  }

  @media (prefers-reduced-motion: reduce) {
    :global(.sidebar-collapsible) {
      transition: none;
    }
  }

  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 260px;
    min-width: 200px;
    background: var(--color-surface-raised, #15171c);
    overflow: hidden;
  }

  .sidebar-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-3, 0.75rem) var(--space-4, 1rem);
    border-bottom: 1px solid var(--color-border, #2a2d35);
  }

  .sidebar-header h2 {
    margin: 0;
    font-size: 0.875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--color-text-muted, #8b8fa3);
  }

  .btn-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: none;
    border-radius: var(--radius-sm, 4px);
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 1.125rem;
    cursor: pointer;
    transition: background 150ms, color 150ms;
  }

  .btn-icon:hover {
    background: var(--color-surface-hover, #1e2028);
    color: var(--color-text, #e6e8ef);
  }

  .btn-icon.copied {
    color: var(--color-accent, #60a5fa);
  }

  .btn-sm {
    width: 24px;
    height: 24px;
    font-size: 0.875rem;
  }

  .add-project-form {
    display: flex;
    flex-direction: column;
    gap: var(--space-2, 0.5rem);
    padding: var(--space-3, 0.75rem) var(--space-4, 1rem);
    border-bottom: 1px solid var(--color-border, #2a2d35);
  }

  .input {
    width: 100%;
    padding: var(--space-2, 0.5rem);
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: var(--radius-sm, 4px);
    background: var(--color-surface, #0f1115);
    color: var(--color-text, #e6e8ef);
    font-size: 0.8125rem;
    font-family: inherit;
  }

  .input::placeholder {
    color: var(--color-text-muted, #8b8fa3);
  }

  .input:focus {
    outline: none;
    border-color: var(--color-accent, #60a5fa);
  }

  .form-actions {
    display: flex;
    gap: var(--space-2, 0.5rem);
  }

  .btn {
    padding: var(--space-1, 0.25rem) var(--space-3, 0.75rem);
    border: none;
    border-radius: var(--radius-sm, 4px);
    font-size: 0.8125rem;
    font-family: inherit;
    cursor: pointer;
    transition: background 150ms;
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

  .error-message {
    margin: 0;
    padding: var(--space-2, 0.5rem) var(--space-4, 1rem);
    color: var(--color-error, #f87171);
    font-size: 0.8125rem;
  }

  .project-list {
    flex: 1;
    overflow-y: auto;
  }

  .project-list ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .loading,
  .empty {
    margin: 0;
    padding: var(--space-4, 1rem);
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.8125rem;
    text-align: center;
  }

  .project-item {
    /* Position relative so the absolutely-positioned ColorSwatchPicker
       inside `.project-actions` anchors to this row. */
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2, 0.5rem);
    padding: var(--space-2, 0.5rem) var(--space-4, 1rem);
    cursor: pointer;
    transition: background 150ms;
    outline: none;
    /* Reserve the left-border gutter for the selected-state accent so
       non-selected items don't shift by 2 px when the selection changes. */
    border-left: 2px solid transparent;
  }

  .project-item:hover,
  .project-item:focus-visible {
    background: var(--color-surface-hover, #1e2028);
  }

  /* Phase 2: selected projects use their project colour (falling back to the
     global accent for pre-Phase-2 projects without `settings.color`). */
  .project-item.selected {
    background: var(--color-surface-active, #252830);
    border-left-color: var(--project-color, var(--color-accent, #60a5fa));
  }

  .project-item.archived {
    opacity: 0.6;
  }

  .project-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .project-name {
    font-size: 0.875rem;
    font-weight: 500;
    color: var(--color-text, #e6e8ef);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .project-path {
    font-size: 0.6875rem;
    color: var(--color-text-muted, #8b8fa3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .project-actions {
    /* Position relative so the ColorSwatchPicker anchors here for a
       properly-placed popover under the swatch button. */
    position: relative;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 2px;
  }

  /* Non-swatch action buttons (▶, ⧉, ⊘) retain the original hover-reveal
     pattern — they exist for power users and shouldn't clutter every row. */
  .project-actions > .btn-icon {
    opacity: 0.55;
    transition: opacity 150ms;
  }

  .project-item:hover .project-actions > .btn-icon,
  .project-item:focus-within .project-actions > .btn-icon {
    opacity: 1;
  }

  /* Phase 2-B: Colour swatch. Always visible (60% opacity at rest → 100%
     on hover) so each project's identity colour is legible even without
     hovering. Uses `--project-color` threaded through the `.project-item`
     inline style. */
  .color-swatch {
    width: 12px;
    height: 12px;
    padding: 0;
    margin: 0 4px 0 0;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 50%;
    background: var(--project-color, var(--color-accent, #60a5fa));
    cursor: pointer;
    opacity: 0.6;
    transition: opacity 150ms, transform 150ms;
    flex-shrink: 0;
  }

  .color-swatch:hover,
  .color-swatch:focus-visible {
    opacity: 1;
    transform: scale(1.15);
    outline: none;
  }

  /* Phase 2-B: Colour swatch. Always visible (60% opacity at rest → 100%
     on hover) so each project's identity colour is legible even without
     hovering. Uses `--project-color` threaded through the `.project-item`
     inline style. */
  .color-swatch {
    width: 12px;
    height: 12px;
    padding: 0;
    margin: 0 4px 0 0;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 50%;
    background: var(--project-color, var(--color-accent, #60a5fa));
    cursor: pointer;
    opacity: 0.6;
    transition: opacity 150ms, transform 150ms;
    flex-shrink: 0;
  }

  .color-swatch:hover,
  .color-swatch:focus-visible {
    opacity: 1;
    transform: scale(1.15);
    outline: none;
  }

  .sidebar-footer {
    padding: var(--space-3, 0.75rem) var(--space-4, 1rem);
    border-top: 1px solid var(--color-border, #2a2d35);
  }

  .toggle-label {
    display: flex;
    align-items: center;
    gap: var(--space-2, 0.5rem);
    font-size: 0.75rem;
    color: var(--color-text-muted, #8b8fa3);
    cursor: pointer;
  }

  .toggle-label input[type="checkbox"] {
    accent-color: var(--color-accent, #60a5fa);
  }
</style>
