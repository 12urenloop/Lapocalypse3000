<script lang="ts">
	import RangeChart from '$lib/components/RangeChart.svelte';
</script>

<svelte:head>
	<title>UWB Range Visualization</title>
</svelte:head>

<main>
	<RangeChart />
</main>

<style>
	main {
		min-height: 100vh;
		padding: 2rem;
		background: #0a0a12;
	}
</style>
