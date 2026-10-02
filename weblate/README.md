# Weblate

Translators who would rather not edit files can translate Waypoint in a browser with [Weblate](https://weblate.org). Nothing here turns Weblate on: it is the component definition to create in a Weblate project, and the rules for taking its output back into the repository. The command-line route (edit `translations/<locale>.json`, run `bun run i18n:import`) is in [`docs/translating.md`](../docs/translating.md) and works without Weblate.

## Component

`component.json` is the body of `POST /api/projects/<project>/components/` (or the same values entered in the web form). The ones that matter:

| Setting             | Value                                                                                                                                                                                |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| File format         | WebExtension JSON (`webextension`), the Weblate format whose entries are `{ "message": ..., "description": ... }`                                                                    |
| File mask           | `apps/waypoint/src/i18n/translations/*.json`                                                                                                                                         |
| Monolingual base    | `apps/waypoint/src/i18n/translations/en-CA.json` (the English source, with every message and its note)                                                                               |
| Language code style | BCP 47 (`fr-CA`), the file names the app uses                                                                                                                                        |
| Excluded languages  | `en-XA` and `ar-XB`, the developer-only pseudo-locales (generated from English, never translated), and `en-CA` itself; they have no JSON file, and the language regex keeps them out |
| Adding a language   | Allowed (`new_lang: add`); the new file also needs the steps in the guide before the app offers it                                                                                   |

Create the Weblate project with `en-CA` as the source language and a licence that matches the repository (Apache-2.0 OR MIT). Do not give Weblate write access to `main`: let it push to its own branch and open pull requests.

## What Weblate does not know

- **Notes for translators.** Each entry in the exported files has a `description` (the area, the placeholders, the plural group and the translator comment from `messages.ts`), which is the field Weblate's WebExtension JSON format shows beside a string as its explanation, so the notes reach translators in the browser.
- **The `source` hash.** Weblate rewrites the files and drops fields it does not own, so the hash that marks a translation stale is lost on its way back. `bun run i18n:import` keeps the hash already in the catalogue for a message whose text did not change and takes the current English text's hash for one that did, so a translator who edits a string in Weblate is taken to have translated the English they saw. Weblate's own "needs editing" state is how it flags a changed source; use that there.
- **Plural groups.** Waypoint's plurals are separate keys (`tabs.count.one`, `tabs.count.other`, and for a language that needs more, `.few`, `.many`, `.zero`, `.two`). The base file has only English's `one` and `other`, so Weblate shows a language that needs `few` or `many` only the keys English has. Add the missing forms by editing the JSON (the guide lists them for each language), or in a Weblate language with extra forms, add the key through the form's "Add new string" and keep the `.few` or `.many` name.

## How translations come back

1. Weblate commits to its own branch and opens a pull request, or a maintainer pulls the branch. The pull request title follows the usual rules (human readable, no Conventional Commit prefix).
2. A maintainer checks out the branch and runs `bun run i18n:import`, then commits the regenerated `catalogues/<locale>.ts` onto it. Weblate edits only the JSON; the app reads the TypeScript, and `bun run check:i18n` (part of `validate` and CI) fails until the two agree.
3. `bun run i18n:export` after an English change refreshes the JSON Weblate reads, so a changed English message reaches it; commit that with the English change.
4. A locale with gaps stays in `INCOMPLETE_LOCALES` (`apps/waypoint/src/i18n/catalogueParity.ts`) until it is complete, and `bun run i18n:status` shows how far along it is.

## Optional: import automatically

A workflow that runs `bun run i18n:import` and commits the result on Weblate's pull requests would remove step 2. It is not added: it needs a token that can push to the pull request's branch, and that is a decision for the repository's owner. Do not enable one without it.
