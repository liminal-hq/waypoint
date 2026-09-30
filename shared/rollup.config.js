import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import typescript from '@rollup/plugin-typescript';

/**
 * Creates Rollup configuration for building Tauri plugin guest bindings.
 *
 * @param {string} pluginName - The plugin name (for example, 'alarm-manager').
 * @returns {import('rollup').RollupOptions[]} Rollup configs.
 */
export function createConfig(pluginName) {
	// Read package.json to use package-level output paths.
	const pkg = JSON.parse(readFileSync(join(process.cwd(), 'package.json'), 'utf8'));
	const deps = Object.keys(pkg.dependencies || {});
	const peerDeps = Object.keys(pkg.peerDependencies || {});
	const pluginJsName = pluginName.replace(/-([a-z])/g, (_, c) => c.toUpperCase());
	const iifeVarName = `__TAURI_PLUGIN_${pluginName.toUpperCase().replace(/-/g, '_')}__`;

	function onwarn(warning) {
		throw Object.assign(new Error(), warning);
	}

	return [
		// ESM and CJS builds for npm distribution.
		{
			input: 'guest-js/index.ts',
			output: [
				{
					file: pkg.exports.import,
					format: 'es',
					sourcemap: true,
				},
				{
					file: pkg.exports.require,
					format: 'cjs',
					sourcemap: true,
				},
			],
			external: [/^@tauri-apps\/api/, ...deps, ...peerDeps],
			plugins: [typescript({ declaration: true, declarationDir: './dist-js' })],
			onwarn,
		},
		// IIFE build for global mode (used by build.rs).
		{
			input: 'guest-js/index.ts',
			output: {
				file: 'api-iife.js',
				format: 'iife',
				name: iifeVarName,
				banner: "if ('__TAURI__' in window) {",
				footer: `Object.defineProperty(window.__TAURI__, '${pluginJsName}', { value: ${iifeVarName} }) }`,
				// Maps any @tauri-apps/api/<module> import to window.__TAURI__.<module>,
				// which Tauri's runtime populates in IIFE/global mode.
				globals: (id) => {
					const match = id.match(/^@tauri-apps\/api\/(.+)$/);
					return match ? `__TAURI__.${match[1]}` : id;
				},
			},
			external: [/^@tauri-apps\/api/, ...deps, ...peerDeps],
			plugins: [
				typescript({
					tsconfig: join(process.cwd(), '../../tsconfig.base.json'),
					declaration: false,
					declarationMap: false,
					noEmit: false,
					sourceMap: false,
				}),
			],
			onwarn,
		},
	];
}

export default createConfig;
