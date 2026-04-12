<!-- T063: Project sidebar. Lists registered projects with an "Add Project"
     action and an archived toggle. Dispatches project selection events. -->
<script lang="ts">
  import { projectsStore } from '$lib/stores/projects.svelte';
  import type { Project } from '$lib/api/projects';

  interface Props {
    selectedProjectId?: number | null;
    onSelectProject?: (project: Project) => void;
  }

  let { selectedProjectId = null, onSelectProject }: Props = $props();

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
</script>

<aside class="sidebar" role="navigation" aria-label="Projects">
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
          <li
            role="option"
            aria-selected={selectedProjectId === project.id}
            class="project-item"
            class:selected={selectedProjectId === project.id}
            class:archived={project.archived_at !== null}
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

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 260px;
    min-width: 200px;
    border-right: 1px solid var(--color-border, #2a2d35);
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
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2, 0.5rem);
    padding: var(--space-2, 0.5rem) var(--space-4, 1rem);
    cursor: pointer;
    transition: background 150ms;
    outline: none;
  }

  .project-item:hover,
  .project-item:focus-visible {
    background: var(--color-surface-hover, #1e2028);
  }

  .project-item.selected {
    background: var(--color-surface-active, #252830);
    border-left: 2px solid var(--color-accent, #60a5fa);
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
    flex-shrink: 0;
    opacity: 0;
    transition: opacity 150ms;
  }

  .project-item:hover .project-actions,
  .project-item:focus-within .project-actions {
    opacity: 1;
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
