import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		adapter: adapter({
			fallback: null
		}),
		paths: {
			base: '/ide'
		},
		prerender: {
			entries: ['*']
		}
	}
};

export default config;
