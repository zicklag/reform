// This page is a fully client-side interactive app (Monaco editor, xterm.js
// terminal, and a wasm engine). None of those run on the server, so disable
// SSR and prerender the page as a static shell that hydrates in the browser.
export const ssr = false;
export const prerender = true;
