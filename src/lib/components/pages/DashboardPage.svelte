<!--
  Dashboard page. Wraps the existing CrossProjectOverview (open reminders
  grouped by project) with full-page chrome so the top-level nav's
  "Dashboard" section has a proper title + subtitle.
-->
<script lang="ts">
  import PageHeader from '$lib/components/PageHeader.svelte';
  import CrossProjectOverview from '$lib/components/CrossProjectOverview.svelte';
</script>

<div class="dashboard-page">
  <PageHeader title="Dashboard" subtitle="Activity and open reminders across projects" />
  <div class="body">
    <CrossProjectOverview />
  </div>
</div>

<style>
  .dashboard-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    background: var(--color-surface, #0f1115);
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    padding: 0 1.5rem 1.5rem;
  }
</style>
