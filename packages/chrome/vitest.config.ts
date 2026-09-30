// Vitest configuration for the chrome package
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

export default defineConfig({
	plugins: [react()],
	test: {
		css: {
			include: /.+\.module\.css$/,
			modules: { classNameStrategy: 'non-scoped' },
		},
		environment: 'happy-dom',
		setupFiles: ['./src/test/setup.ts'],
		include: ['src/**/*.test.{ts,tsx}'],
	},
});
