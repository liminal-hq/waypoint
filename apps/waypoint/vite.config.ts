import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

// Standard Tauri + Vite setup: a fixed port that `devUrl` in
// `src-tauri/tauri.conf.json` points at, with `strictPort` so a stale
// process never silently shifts it.
export default defineConfig({
	plugins: [react()],
	clearScreen: false,
	server: {
		port: 1420,
		strictPort: true,
		host: 'localhost',
		watch: { ignored: ['**/src-tauri/**'] },
	},
	build: {
		target: 'esnext',
		minify: 'esbuild',
		sourcemap: true,
	},
	test: {
		environment: 'happy-dom',
		setupFiles: ['./src/test/setup.ts'],
		css: { modules: { classNameStrategy: 'non-scoped' } },
	},
});
