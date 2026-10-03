// Fails when a tracked stylesheet or component uses a physical direction property
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { scanCss, scanTsx, type Offence } from './logicalCss';

const roots = (process.env.LOGICAL_CSS_ROOTS ?? 'apps packages').split(' ');
const files = execFileSync(
	'git',
	['ls-files', '--', ...roots.flatMap((root) => [`${root}/**/*.css`, `${root}/**/*.tsx`])],
	{ encoding: 'utf8' },
)
	.split('\n')
	.filter((file) => file && !/\.test\.tsx?$/.test(file));

let count = 0;
for (const file of files) {
	const source = readFileSync(file, 'utf8');
	const offences: Offence[] = file.endsWith('.css') ? scanCss(source) : scanTsx(source);
	for (const offence of offences) {
		console.error(`${file}:${offence.line}: ${offence.why}: ${offence.text}`);
		count += 1;
	}
}
if (count > 0) {
	console.error(
		`check-logical-css: ${count} physical direction propert${count === 1 ? 'y' : 'ies'}. Use the logical form (margin-inline-start, inset-inline-end, text-align: start, ...) or mark the line with a \`physical:\` comment and the reason.`,
	);
	process.exit(1);
}
console.log(`check-logical-css: ${files.length} files, ok`);
