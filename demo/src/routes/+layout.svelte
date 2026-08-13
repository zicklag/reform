<script lang="ts">
	import './layout.css';
	import favicon from '$lib/assets/favicon.svg';

	let { children } = $props();

	// The demo is a dark-only UI. Put the `dark` class on <html> so the shadcn
	// dark theme variables cascade down to `body` (which reads --background from
	// its ancestors) as well as every descendant. Guarded for the prerendered
	// shell where `document` isn't present.
	if (typeof document !== 'undefined') {
		document.documentElement.classList.add('dark');
	}
</script>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>
{@render children()}
