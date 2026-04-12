// T061: Svelte 5 runes store for project state.
// Hydrates from the backend on app start, provides reactive project list
// with derived filters for active (non-archived) projects.

import {
  projectList,
  projectRegister,
  projectArchive,
  projectUnarchive,
  projectUpdate,
  type Project,
  type ProjectSettings,
} from '$lib/api/projects';

function createProjectsStore() {
  let projects = $state<Project[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);

  const activeProjects = $derived(
    projects.filter((p) => p.archived_at === null),
  );

  const archivedProjects = $derived(
    projects.filter((p) => p.archived_at !== null),
  );

  return {
    get projects() {
      return projects;
    },
    get activeProjects() {
      return activeProjects;
    },
    get archivedProjects() {
      return archivedProjects;
    },
    get loading() {
      return loading;
    },
    get error() {
      return error;
    },

    /** Fetch all projects from the backend and replace local state. */
    async hydrate(opts?: { includeArchived?: boolean }): Promise<void> {
      loading = true;
      error = null;
      try {
        const result = await projectList({
          includeArchived: opts?.includeArchived ?? true,
        });
        projects = result.projects;
      } catch (err) {
        error = err instanceof Error ? err.message : String(err);
      } finally {
        loading = false;
      }
    },

    /** Register a new project directory. */
    async register(
      path: string,
      displayName?: string,
    ): Promise<Project | undefined> {
      error = null;
      try {
        const result = await projectRegister({ path, displayName });
        projects = [...projects, result.project];
        return result.project;
      } catch (err) {
        error = err instanceof Error ? err.message : String(err);
        return undefined;
      }
    },

    /** Update a project's display name or settings. */
    async update(
      id: number,
      patch: { displayName?: string; settings?: ProjectSettings },
    ): Promise<Project | undefined> {
      error = null;
      try {
        const result = await projectUpdate({ id, ...patch });
        projects = projects.map((p) =>
          p.id === id ? result.project : p,
        );
        return result.project;
      } catch (err) {
        error = err instanceof Error ? err.message : String(err);
        return undefined;
      }
    },

    /** Soft-delete a project. */
    async archive(id: number): Promise<boolean> {
      error = null;
      try {
        await projectArchive({ id });
        // Re-hydrate to get the updated archived_at timestamp from the backend
        await this.hydrate();
        return true;
      } catch (err) {
        error = err instanceof Error ? err.message : String(err);
        return false;
      }
    },

    /** Restore a previously archived project. */
    async unarchive(id: number): Promise<Project | undefined> {
      error = null;
      try {
        const result = await projectUnarchive({ id });
        projects = projects.map((p) =>
          p.id === id ? result.project : p,
        );
        return result.project;
      } catch (err) {
        error = err instanceof Error ? err.message : String(err);
        return undefined;
      }
    },

    /** Find a project by id. */
    byId(id: number): Project | undefined {
      return projects.find((p) => p.id === id);
    },
  };
}

export const projectsStore = createProjectsStore();
