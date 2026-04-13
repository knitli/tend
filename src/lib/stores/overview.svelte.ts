// T114: Cross-project overview store.
// Re-queried on reminder state changes.

import { crossProjectOverview, type OverviewGroup } from "$lib/api/scratchpad";

class OverviewStore {
	groups = $state<OverviewGroup[]>([]);
	loading = $state(false);
	error = $state<string | null>(null);

	/** Fetch the cross-project overview. */
	async refresh() {
		this.loading = true;
		this.error = null;

		try {
			const result = await crossProjectOverview();
			this.groups = result.groups;
		} catch (err: unknown) {
			this.error = err instanceof Error ? err.message : String(err);
		} finally {
			this.loading = false;
		}
	}
}

export const overviewStore = new OverviewStore();
