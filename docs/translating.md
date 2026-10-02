# Translating Waypoint

Waypoint's text lives in message catalogues: English (`en-CA`) is the source in `apps/waypoint/src/i18n/messages.ts`, and every other language is a catalogue with the same keys. A message a language lacks shows in English, so a language can ship before it is complete. This guide covers adding a language, what to keep intact in a message, how to see your work in the app and how the tooling keeps English and the translations in step. The design is A68 and D119 in the decision logs.

## The commands

```bash
bun run i18n:export    # write translations/<locale>.json from the TypeScript (English, and each language)
bun run i18n:import    # write catalogues/<locale>.ts from the JSON; `bun run i18n:import fr-CA` for one
bun run i18n:status    # per language: translated, missing, stale, extra and percent
bun run check:i18n     # the gate: part of `bun run validate` and CI
```

A translator edits `apps/waypoint/src/i18n/translations/<locale>.json` (or does it in Weblate, see [`weblate/README.md`](../weblate/README.md)) and runs `bun run i18n:import`. A developer who changes English edits `messages.ts` and runs `bun run i18n:export`. The JSON and the TypeScript are both checked in, and `bun run check:i18n` fails if one is out of date with the other.

## What is in a translation file

A flat JSON object in English source order, one entry per message:

```json
"tabs.count.one": {
  "message": "{count} onglet",
  "note": "Area: tabs. Placeholders, kept exactly as written: {count} is a number of items (the plural form is chosen from it). Plural: one of a group; the other forms are tabs.count.many, tabs.count.other.",
  "source": "219e92dc"
}
```

- `message` is the translation. An empty message counts as untranslated.
- `note` is written for you by the exporter; changing it does nothing. It gives the **area** (the first part of the key: `settings`, `browse`, `ops`), each `{placeholder}` and what it stands for, whether the message is one of a **plural** group and its other forms, and any comment the developers wrote for the translator in the English source, which is where a word like "Kind" or "Open" says which sense is meant.
- `source` is a short hash of the English text the translation was made from. When English changes, the hash no longer matches, the message is **stale**, and `bun run i18n:status` lists it as needing review. English's own file, `en-CA.json`, has `message` and `note` only.

## Placeholders

`{count}`, `{name}`, `{reason}` and the like are filled in by the app. Keep each one exactly as written, with the same braces and name; you can move it, and use it more than once. `bun run i18n:import` refuses a message whose placeholders differ from English's, and `check:i18n` does too. Do not translate the name inside the braces.

French: `"Renamed {from} to {to}"` becomes `"{from} renommé en {to}"`.

## Plurals

A plural group is a set of keys that end in a plural category, such as `tabs.count.one` and `tabs.count.other`. The app picks the key by the language's own number rules (`Intl.PluralRules`), and `bun run i18n:status` and the parity test expect exactly the categories the language has. English has `one` and `other`.

- **French** has `one` (0 and 1: "0 onglet", "1 onglet"), `many` (a million or more, "1 000 000 d’onglets", the form that takes "de") and `other` ("2 onglets"). All three are needed.
- **Polish** has `one` (1), `few` (2 to 4, 22 to 24, ...), `many` (0, 5 to 21, ...) and `other` (fractions such as 1.5). Write all four: `"{count} karta"`, `"{count} karty"`, `"{count} kart"`, `"{count} karty"`.
- **Arabic** has all six: `zero`, `one`, `two`, `few`, `many` and `other`.
- **Japanese** has only `other`.

A form English does not have (`few`, `many`) takes its note and its placeholders from the group's English `other`. If a language needs a form you leave out, the app falls back to its `other` for that count, which can read wrongly.

## Adding a language

1. Add the tag to `LOCALES` in `apps/waypoint/src/i18n/locales.ts`, a name for it in `settings.language.language.<tag>` in `messages.ts`, and a loader in `MODULES` in `loadCatalogue.ts` (`'pl-PL': () => import('./catalogues/pl-PL')`). The Language setting's options in `apps/waypoint/src/services/settingsClient.ts` and the list of accepted languages in `crates/waypoint-settings/src/model.rs` list `fr-CA` too, so a new language is added there as well; searching for `fr-CA` finds every place.
2. List it in `INCOMPLETE_LOCALES` in `catalogueParity.ts` while it is being written, so a missing message is not a failure.
3. Run `bun run i18n:export` to create `translations/<tag>.json`, then translate entries into it (or copy the ones you want from `en-CA.json` and edit the `message`s; `note` and `source` are written by the exporter). Run `bun run i18n:import` to create `catalogues/<tag>.ts`.
4. Run `bun run i18n:status`: it counts what is translated, missing, stale and extra.
5. When it reaches 100% with nothing stale, take it out of `INCOMPLETE_LOCALES`. From then on a missing, stale or extra message fails `check:i18n`, so an English change cannot ship without the translation being revisited.

## Testing in the app

Open Settings → Language & region and choose the language; every window changes at once. A message you have not translated appears in English. A developer build (`bun run tauri:dev`) also lists two pseudo-locales, for checking the layout: `en-XA` (English with accents and about a third more text, to find text that does not fit) and `ar-XB` (mirrored, right to left). They are generated from English, are never translated and are not in the release.

## Stale translations

Each translated message records the hash of its English text. Changing a message in `messages.ts` makes its translations stale: `bun run i18n:status` shows a count per language, and `bun run check:i18n` fails for a language that is not in `INCOMPLETE_LOCALES`. To clear it, check the translation against the new English and run `bun run i18n:export`, or edit the JSON and run `bun run i18n:import` with the `source` line removed so the hash follows the English you translated.

## Notes for developers

A comment written directly above a key in `messages.ts`, starting `// translator:`, is copied into the key's note. Use it where a short word could be read two ways. Plain `//` lines after it continue it:

```ts
// translator: A column heading: what sort of thing each entry is, such as Folder or Image.
// Not the file extension; that is “Type”.
'browse.column.kind': 'Kind',
```
