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
    /** Controlled open state driven by the parent. */
    open?: boolean;
    /** Stable id for the Collapsible.Content element so the toggle button
     *  can point its `aria-controls` at it. */
    contentId?: string;
    /** Called when the user clicks the edge toggle button. */
    onToggle?: (nextOpen: boolean) => void;
  }

  let {
    selectedProjectId = null,
    onSelectProject,
    onSpawnSession,
    open = true,
    contentId = 'sidebar-collapsible-content',
    onToggle,
  }: Props = $props();

  /** Phase 2-B: id of the project whose colour picker is currently open. */
  let pickerProjectId = $state<number | null>(null);
  let pickerSwatchEl = $state<HTMLButtonElement | null>(null);

  let pendingColor = $state<Record<number, string>>({});
  let pendingColorTimer: ReturnType<typeof setTimeout> | null = null;
  let pendingColorProject: Project | null = null;

  function openPicker(event: MouseEvent, projectId: number): void {
    event.stopPropagation();
    if (pickerProjectId === projectId) {
      closePicker();
      return;
    }
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
      void projectsStore.update(project.id, {
        settings: { ...project.settings, color: hex },
      });
    }
  }

  function closePicker(): void {
    flushPendingColor();
    pickerProjectId = null;
    pickerSwatchEl = null;
  }

  function handleColorChange(project: Project, hex: string): void {
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
  <!-- The sidebar wrapper + the edge toggle live together so the toggle
       button visually sits at the right edge of the sidebar regardless of
       open/closed state. -->
  <div class="sidebar-wrapper" class:collapsed={!open}>
    <Collapsible.Content
      id={contentId}
      forceMount
      class="sidebar-collapsible"
    >
      <aside
        class="sidebar"
        role="navigation"
        aria-label="Projects"
        aria-hidden={!open}
        inert={!open}
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

    <!-- Edge toggle: always visible. When open shows ‹ (collapse), when
         closed shows › (expand). The ridged grip texture communicates
         "this edge is interactive". -->
    <button
      type="button"
      class="sidebar-edge-toggle"
      aria-controls={contentId}
      aria-expanded={open}
      aria-label={open ? 'Collapse sidebar' : 'Expand sidebar'}
      title={open ? 'Collapse sidebar' : 'Expand sidebar'}
      onclick={() => onToggle?.(!open)}
    >
      <span class="edge-grip" aria-hidden="true">
        <span class="grip-dots"></span>
      </span>
      <span class="edge-chevron" aria-hidden="true">{open ? '‹' : '›'}</span>
    </button>
  </div>
</Collapsible.Root>

<style>
  /* Wrapper holds sidebar + edge toggle side by side. When collapsed the
     sidebar content is width: 0 but the toggle stays visible. */
  .sidebar-wrapper {
    display: flex;
    flex-shrink: 0;
    height: 100%;
    position: relative;
  }

  :global(.sidebar-collapsible) {
    display: flex;
    flex-direction: column;
    height: 100%;
    flex-shrink: 0;
    overflow: hidden;
    transition: width 200ms ease;
  }

  :global(.sidebar-collapsible[data-state="open"]) {
    width: 260px;
  }

  :global(.sidebar-collapsible[data-state="closed"]) {
    width: 0;
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

  /* ── Edge toggle button ──────────────────────────────────────────── */
  .sidebar-edge-toggle {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    width: 12px;
    height: 100%;
    padding: 0;
    border: none;
    border-left: 1px solid var(--color-border, #2a2d35);
    background: var(--color-surface-raised, #15171c);
    color: var(--color-text-muted, #8b8fa3);
    cursor: pointer;
    flex-shrink: 0;
    position: relative;
    transition: background 150ms, width 150ms;
  }

  .sidebar-edge-toggle:hover {
    width: 16px;
    background: var(--color-surface-hover, #1e2028);
    color: var(--color-text, #e6e8ef);
  }

  .sidebar-edge-toggle:focus-visible {
    outline: 2px solid var(--color-accent, #60a5fa);
    outline-offset: -2px;
  }

  /* Grip texture: vertical dots that hint "draggable / interactive edge". */
  .edge-grip {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .grip-dots {
    display: block;
    width: 4px;
    height: 32px;
    /* Two columns of dots via radial-gradient */
    background-image: radial-gradient(circle, currentColor 1px, transparent 1px);
    background-size: 4px 4px;
    background-position: 0 0;
    opacity: 0.4;
    transition: opacity 150ms;
  }

  .sidebar-edge-toggle:hover .grip-dots {
    opacity: 0.8;
  }

  .edge-chevron {
    font-size: 0.625rem;
    line-height: 1;
    padding-bottom: 4px;
    opacity: 0;
    transition: opacity 150ms;
  }

  .sidebar-edge-toggle:hover .edge-chevron {
    opacity: 1;
  }

  /* When sidebar is collapsed, make the toggle slightly wider so it's
     easier to find and click. */
  .collapsed .sidebar-edge-toggle {
    width: 16px;
    border-left: none;
    border-right: 1px solid var(--color-border, #2a2d35);
  }

  .collapsed .sidebar-edge-toggle:hover {
    width: 20px;
  }

  .collapsed .edge-chevron {
    opacity: 0.6;
  }

  /* ── Sidebar internals ───────────────────────────────────────────── */
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
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2, 0.5rem);
    padding: var(--space-2, 0.5rem) var(--space-4, 1rem);
    cursor: pointer;
    transition: background 150ms;
    outline: none;
    border-left: 2px solid transparent;
  }

  .project-item:hover,
  .project-item:focus-visible {
    background: var(--color-surface-hover, #1e2028);
  }

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
    position: relative;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .project-actions > .btn-icon {
    opacity: 0.55;
    transition: opacity 150ms;
  }

  .project-item:hover .project-actions > .btn-icon,
  .project-item:focus-within .project-actions > .btn-icon {
    opacity: 1;
  }

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
