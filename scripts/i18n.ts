// The translator tooling's command line: `bun run i18n:export`, `i18n:import`, `i18n:status`, `check:i18n`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import {
	cataloguePath,
	checkI18n,
	completenessProblems,
	formatStatus,
	plannedCatalogue,
	plannedExports,
	readEnglish,
	statusOf,
} from './i18nTooling';
import { repositoryWorkspace } from './i18nWorkspace';
import type { Workspace } from './i18nTooling';

const USAGE = `usage: bun scripts/i18n.ts <command> [locale ...]

  export   write apps/waypoint/src/i18n/translations/<locale>.json from the TypeScript
  import   write catalogues/<locale>.ts from the JSON (every locale, or the ones named)
  status   print translated, missing, stale and extra messages per locale
  check    fail when the JSON is out of date, a placeholder differs, or a complete locale has gaps`;

function selected(ws: Workspace, names: string[]): string[] {
	const unknown = names.filter((name) => !ws.locales.includes(name));
	if (unknown.length > 0) {
		throw new Error(
			`${unknown.join(', ')}: not a translatable locale (${ws.locales.join(', ')}); add it to LOCALES in locales.ts first`,
		);
	}
	return names.length > 0 ? names : [...ws.locales];
}

async function main(argv: string[]): Promise<number> {
	const [command, ...names] = argv;
	const ws = repositoryWorkspace();
	switch (command) {
		case 'export': {
			for (const [path, content] of plannedExports(ws)) {
				mkdirSync(dirname(path), { recursive: true });
				writeFileSync(path, content);
			}
			console.log(`i18n:export: wrote ${ws.locales.length + 1} files to translations/`);
			return 0;
		}
		case 'import': {
			const english = readEnglish(ws);
			const failures: string[] = [];
			const writes: [string, string][] = [];
			for (const locale of selected(ws, names)) {
				const planned = await plannedCatalogue(ws, locale, english);
				failures.push(...planned.errors);
				if (planned.source !== undefined) writes.push([cataloguePath(ws, locale), planned.source]);
			}
			if (failures.length > 0) {
				for (const failure of failures) console.error(`i18n:import: ${failure}`);
				console.error('i18n:import: nothing was written');
				return 1;
			}
			for (const [path, content] of writes) writeFileSync(path, content);
			console.log(`i18n:import: wrote ${writes.length} catalogue${writes.length === 1 ? '' : 's'}`);
			return 0;
		}
		case 'status': {
			const statuses = statusOf(ws).filter((s) => names.length === 0 || names.includes(s.locale));
			console.log(formatStatus(statuses, ws.incomplete));
			const problems = statuses
				.filter((s) => !ws.incomplete.includes(s.locale))
				.flatMap(completenessProblems);
			for (const problem of problems) console.error(`i18n:status: ${problem}`);
			return problems.length > 0 ? 1 : 0;
		}
		case 'check': {
			const problems = await checkI18n(ws);
			for (const problem of problems) console.error(`check-i18n: ${problem}`);
			if (problems.length > 0) {
				console.error(
					`check-i18n: ${problems.length} problem${problems.length === 1 ? '' : 's'}. See docs/translating.md.`,
				);
				return 1;
			}
			console.log(`check-i18n: ${ws.locales.length + 1} languages, ok`);
			return 0;
		}
		default:
			console.error(USAGE);
			return command === undefined || command === 'help' ? 0 : 2;
	}
}

try {
	process.exit(await main(process.argv.slice(2)));
} catch (error) {
	console.error(`i18n: ${(error as Error).message}`);
	process.exit(1);
}
