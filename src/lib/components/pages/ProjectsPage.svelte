<!--
  Full-page Projects view. Lists registered projects with sort + filter
  controls, mirroring the Sessions chrome. Click a row to select the
  project and jump to Sessions (scoped to that project).
-->
<script lang="ts">
  import PageHeader from '$lib/components/PageHeader.svelte';
  import ColorSwatchPicker from '$lib/components/ColorSwatchPicker.svelte';
  import { projectsStore } from '$lib/stores/projects.svelte';
  import { sessionsStore } from '$lib/stores/sessions.svelte';
  import type { Project } from '$lib/api/projects';
  import { getProjectColor } from '$lib/util/projectColor';

  type SortKey = 'name' | 'added' | 'active';

  interface Props {
    selectedProjectId?: number | null;
    onSelectProject?: (project: Project) => void;
    onSpawnSession?: (project: Project) => void;
  }

  let { selectedProjectId = null, onSelectProject, onSpawnSession }: Props = $props();

  let filterText = $state('');
  let sortKey = $state<SortKey>('active');
  let showArchived = $state(false);
  let addingProject = $state(false);
  let newPath = $state('');
  let newName = $state('');
  let colorSaveError = $state<string | null>(null);
  let disposed = false;

  const sessionCounts = $derived.by<Map<number, number>>(() => {
    const map = new Map<number, number>();
    for (const s of sessionsStore.sessions) {
      if (s.status === 'ended' || s.status === 'error') continue;
      map.set(s.project_id, (map.get(s.project_id) ?? 0) + 1);
    }
    return map;
  });

  const filtered = $derived.by(() => {
    const source = showArchived ? projectsStore.projects : projectsStore.activeProjects;
    const q = filterText.trim().toLowerCase();
    const list = q
      ? source.filter(
          (p) =>
            p.display_name.toLowerCase().includes(q) ||
            p.canonical_path.toLowerCase().includes(q),
        )
      : source.slice();

    list.sort((a, b) => {
      if (sortKey === 'name') return a.display_name.localeCompare(b.display_name);
      if (sortKey === 'added') return b.added_at.localeCompare(a.added_at);
      // 'active' — most active sessions first, then by name.
      const ca = sessionCounts.get(a.id) ?? 0;
      const cb = sessionCounts.get(b.id) ?? 0;
      if (ca !== cb) return cb - ca;
      return a.display_name.localeCompare(b.display_name);
    });
    return list;
  });

  async function handleAdd(): Promise<void> {
    if (!newPath.trim()) return;
    const p = await projectsStore.register(newPath.trim(), newName.trim() || undefined);
    if (p) {
      newPath = '';
      newName = '';
      addingProject = false;
      onSelectProject?.(p);
    }
  }

  // Colour picker state per row.
  let pickerId = $state<number | null>(null);
  let pickerEl = $state<HTMLButtonElement | null>(null);
  let pendingColor = $state<Record<number, string>>({});
  let pendingColorTimer: ReturnType<typeof setTimeout> | null = null;
  let pendingColorProject: Project | null = null;

  function flushPendingColor(): void {
    if (pendingColorTimer !== null) {
      clearTimeout(pendingColorTimer);
      pendingColorTimer = null;
    }
    const project = pendingColorProject;
    const hex = project ? pendingColor[project.id] : undefined;
    pendingColorProject = null;
    if (project && typeof hex === 'string') {
      void (async () => {
        const updated = await projectsStore.update(project.id, {
          settings: { ...project.settings, color: hex },
        });
        if (!updated && !disposed) {
          colorSaveError = projectsStore.error ?? 'Failed to save project color.';
        }
      })();
    }
  }

  function openPicker(event: MouseEvent, id: number): void {
    event.stopPropagation();
    if (pendingColorProject !== null) {
      flushPendingColor();
    }
    pickerId = pickerId === id ? null : id;
    pickerEl = event.currentTarget as HTMLButtonElement;
  }

  function handleColorChange(project: Project, hex: string): void {
    colorSaveError = null;
    pendingColor = { ...pendingColor, [project.id]: hex };
    pendingColorProject = project;
    if (pendingColorTimer !== null) clearTimeout(pendingColorTimer);
    pendingColorTimer = setTimeout(() => {
      pendingColorTimer = null;
      flushPendingColor();
    }, 200);
  }

  $effect(() => {
    return () => {
      disposed = true;
      flushPendingColor();
    };
  });
</script>

<div class="projects-page">
  <PageHeader title="Projects" subtitle="Manage registered repositories">
    {#snippet trailing()}
      <button class="btn primary" onclick={() => (addingProject = !addingProject)}>
        {addingProject ? 'Cancel' : '+ Add project'}
      </button>
    {/snippet}
  </PageHeader>

  {#if addingProject}
    <form class="add-form" onsubmit={(e) => { e.preventDefault(); handleAdd(); }}>
      <input class="input" type="text" placeholder="Project path…" bind:value={newPath} required />
      <input class="input" type="text" placeholder="Display name (optional)" bind:value={newName} />
      <button class="btn primary" type="submit" disabled={!newPath.trim()}>Add</button>
    </form>
  {/if}

  <div class="toolbar">
    <div class="search-wrap">
      <span class="search-icon" aria-hidden="true">⌕</span>
      <input
        class="search"
        type="search"
        placeholder="Search projects…"
        bind:value={filterText}
        aria-label="Search projects"
      />
    </div>
    <label class="sort-wrap">
      <select class="select" bind:value={sortKey} aria-label="Sort projects">
        <option value="active">Most active</option>
        <option value="name">Name (A→Z)</option>
        <option value="added">Recently added</option>
      </select>
    </label>
    <label class="check">
      <input type="checkbox" bind:checked={showArchived} />
      Show archived
    </label>
  </div>

  {#if projectsStore.error}
    <p class="error" role="alert">{projectsStore.error}</p>
  {/if}
  {#if colorSaveError}
    <p class="error" role="alert">{colorSaveError}</p>
  {/if}

  <div class="list-wrap">
    {#if projectsStore.loading}
      <p class="empty">Loading…</p>
    {:else if filtered.length === 0}
      <div class="empty-state">
        <p class="lead">No projects {filterText ? 'match your search' : 'yet'}.</p>
        {#if !filterText}
          <p class="hint">Click <strong>+ Add project</strong> to register one.</p>
        {/if}
      </div>
    {:else}
      <ul class="card-list" role="list">
        {#each filtered as project (project.id)}
          {@const color = getProjectColor(project)}
          {@const count = sessionCounts.get(project.id) ?? 0}
          <li
            class="project-card"
            class:selected={selectedProjectId === project.id}
            class:archived={project.archived_at !== null}
            style={color ? `--project-color: ${color}` : ''}
          >
            <button
              class="project-card-main"
              type="button"
              onclick={() => onSelectProject?.(project)}
              title="Open in Sessions"
            >
              <span class="stripe" aria-hidden="true"></span>
              <div class="card-info">
                <span class="card-name">{project.display_name}</span>
                <span class="card-path" title={project.canonical_path}>{project.canonical_path}</span>
              </div>
              <span class="session-count" title="Live sessions">
                {count}
                <span class="count-label">{count === 1 ? 'session' : 'sessions'}</span>
              </span>
            </button>
            <div class="card-actions">
              <button
                class="swatch"
                title="Change colour"
                aria-label="Change colour for {project.display_name}"
                onclick={(e) => openPicker(e, project.id)}
              ></button>
              {#if pickerId === project.id}
                <ColorSwatchPicker
                  value={pendingColor[project.id] ?? color ?? '#60a5fa'}
                  ignoreEl={pickerEl}
                  onChange={(hex) => handleColorChange(project, hex)}
                  onClose={() => {
                    flushPendingColor();
                    pickerId = null;
                  }}
                />
              {/if}
              {#if !project.archived_at}
                <button
                  class="btn-icon"
                  title="Start session"
                  aria-label="Start session in {project.display_name}"
                  onclick={() => onSpawnSession?.(project)}
                >▶</button>
                <button
                  class="btn-icon"
                  title="Archive"
                  aria-label="Archive {project.display_name}"
                  onclick={() => projectsStore.archive(project.id)}
                >⊘</button>
              {:else}
                <button
                  class="btn-icon"
                  title="Unarchive"
                  aria-label="Unarchive {project.display_name}"
                  onclick={() => projectsStore.unarchive(project.id)}
                >↩</button>
              {/if}
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>

<style>
  .projects-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    background: var(--color-surface, #0f1115);
  }

  .add-form {
    display: flex;
    gap: 0.5rem;
    padding: 0 1.5rem 0.75rem;
  }

  .input {
    flex: 1;
    padding: 0.375rem 0.625rem;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.375rem;
    background: var(--color-surface-raised, #15171c);
    color: var(--color-text, #e6e8ef);
    font-size: 0.8125rem;
    font-family: inherit;
  }

  .input:focus {
    outline: none;
    border-color: var(--color-accent, #60a5fa);
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0 1.5rem 0.75rem;
    flex-wrap: wrap;
  }

  .search-wrap {
    position: relative;
    flex: 1;
    min-width: 220px;
  }

  .search-icon {
    position: absolute;
    left: 0.5rem;
    top: 50%;
    transform: translateY(-50%);
    color: var(--color-text-muted, #8b8fa3);
    pointer-events: none;
  }

  .search {
    width: 100%;
    padding: 0.375rem 0.625rem 0.375rem 1.75rem;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.375rem;
    background: var(--color-surface-raised, #15171c);
    color: var(--color-text, #e6e8ef);
    font-size: 0.8125rem;
    font-family: inherit;
  }

  .search:focus {
    outline: none;
    border-color: var(--color-accent, #60a5fa);
  }

  .select {
    padding: 0.375rem 0.625rem;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.375rem;
    background: var(--color-surface-raised, #15171c);
    color: var(--color-text, #e6e8ef);
    font-size: 0.8125rem;
    font-family: inherit;
  }

  .check {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
    font-size: 0.75rem;
    color: var(--color-text-muted, #8b8fa3);
  }

  .error {
    margin: 0 1.5rem 0.5rem;
    padding: 0.5rem 0.75rem;
    background: rgba(248, 113, 113, 0.1);
    border: 1px solid var(--color-error, #f87171);
    border-radius: 0.25rem;
    color: var(--color-error, #f87171);
    font-size: 0.8125rem;
  }

  .list-wrap {
    flex: 1;
    overflow-y: auto;
    padding: 0 1.5rem 1.5rem;
  }

  .empty,
  .empty-state {
    padding: 2rem 0;
    text-align: center;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.8125rem;
  }

  .empty-state .lead {
    margin: 0 0 0.25rem;
    color: var(--color-text, #e6e8ef);
    font-size: 0.9375rem;
  }

  .empty-state .hint {
    margin: 0;
  }

  .card-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
  }

  .project-card {
    position: relative;
    display: flex;
    align-items: stretch;
    background: var(--color-surface-raised, #15171c);
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.5rem;
    overflow: hidden;
    transition: background 150ms, border-color 150ms;
  }

  .project-card.selected {
    border-color: var(--project-color, var(--color-accent, #60a5fa));
  }

  .project-card.archived {
    opacity: 0.6;
  }

  .project-card:hover {
    background: var(--color-surface-hover, #1e2028);
  }

  .project-card-main {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 0.875rem;
    background: transparent;
    border: none;
    color: inherit;
    font-family: inherit;
    font-size: inherit;
    text-align: left;
    cursor: pointer;
  }

  .stripe {
    width: 4px;
    align-self: stretch;
    background: var(--project-color, var(--color-accent, #60a5fa));
    border-radius: 2px;
    flex-shrink: 0;
  }

  .card-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .card-name {
    font-size: 0.9375rem;
    font-weight: 600;
    color: var(--color-text, #e6e8ef);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .card-path {
    font-size: 0.75rem;
    color: var(--color-text-muted, #8b8fa3);
    font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .session-count {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    color: var(--color-text, #e6e8ef);
    font-size: 1.25rem;
    font-weight: 600;
    line-height: 1;
  }

  .count-label {
    font-size: 0.625rem;
    font-weight: 500;
    color: var(--color-text-muted, #8b8fa3);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin-top: 2px;
  }

  .card-actions {
    position: relative;
    display: flex;
    align-items: center;
    gap: 0.125rem;
    padding: 0 0.625rem;
  }

  .swatch {
    width: 14px;
    height: 14px;
    padding: 0;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 50%;
    background: var(--project-color, var(--color-accent, #60a5fa));
    cursor: pointer;
    opacity: 0.7;
    transition: opacity 150ms, transform 150ms;
  }

  .swatch:hover {
    opacity: 1;
    transform: scale(1.1);
  }

  .btn-icon {
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 0.25rem;
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.875rem;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition: background 150ms, color 150ms;
  }

  .btn-icon:hover {
    background: var(--color-surface-hover, #1e2028);
    color: var(--color-text, #e6e8ef);
  }

  .btn {
    padding: 0.375rem 0.875rem;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.375rem;
    background: transparent;
    color: var(--color-text, #e6e8ef);
    font-size: 0.8125rem;
    font-family: inherit;
    cursor: pointer;
  }

  .btn.primary {
    background: var(--color-accent, #60a5fa);
    color: var(--color-surface, #0f1115);
    border-color: var(--color-accent, #60a5fa);
    font-weight: 500;
  }

  .btn.primary:hover:not(:disabled) {
    background: var(--color-accent-hover, #93c5fd);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
