// The Canadian French (fr-CA) catalogue, written from translations/fr-CA.json by `bun run i18n:import`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Catalogue } from '../active';

/**
 * Generated: edit `translations/fr-CA.json` (or use Weblate) and run `bun run i18n:import`. A
 * message missing here shows in English. Each `// source:` comment records the hash of the English
 * text the translation was made from, so `bun run i18n:status` can tell when English has moved on.
 */
const messages: Catalogue = {
	// source: eb814be3
	'window.main.title': 'Principale',
	// source: 45d10e1b
	'window.main.titleWithApp': 'Waypoint — Principale',
	// source: a72f1d0b
	'window.main.description': 'L’explorateur de fichiers à onglets — bientôt disponible.',
	// source: 12b1b753
	'window.main.startFailed': 'Waypoint n’a pas pu démarrer l’explorateur de fichiers.',
	// source: 57034aef
	'window.settings.title': 'Waypoint — Paramètres',
	// source: 1142060a
	'window.properties.title': 'Waypoint — Propriétés',
	// source: 674d10ba
	'window.ops.title': 'Waypoint — Opérations',
	// source: 9f4dc1e5
	'window.shelf.title': 'Waypoint — Étagère',
	// source: 74a883a0
	'window.settings.osTitle': 'Paramètres',
	// source: ae43692b
	'window.properties.osTitle': 'Propriétés',
	// source: 358cc201
	'window.ops.osTitle': 'Opérations',
	// source: 338c8ac8
	'window.shelf.osTitle': 'Étagère',
	// source: 2051a13a
	'window.shelf.dropHint': 'Déposer pour ajouter à l’Étagère',
	// source: c2403a41
	'window.shelf.startFailed': 'Waypoint n’a pas pu démarrer la fenêtre de l’Étagère.',
	// source: 812b5090
	'window.tearGhost.title': 'Waypoint — Aperçu de l’onglet',
	// source: e26d51d3
	'settings.nav.label': 'Sections des paramètres',
	// source: ca184496
	'settings.unavailable': 'Indisponible',
	// source: c910d474
	'settings.section.general': 'Général',
	// source: 358cc201
	'settings.section.operations': 'Opérations',
	// source: f08de9db
	'settings.section.dnd': 'Glisser-déposer',
	// source: 3907fa7f
	'settings.section.appearance': 'Apparence',
	// source: d3368cbf
	'settings.section.accessibility': 'Accessibilité',
	// source: 090512d9
	'settings.section.integrations': 'Intégrations',
	// source: a45737c5
	'settings.section.previews': 'Aperçus et miniatures',
	// source: 8b773803
	'settings.section.transparency': 'Transparence',
	// source: 003e8754
	'settings.section.language': 'Langue et région',
	// source: e71a1345
	'settings.loading': 'Chargement des paramètres…',
	// source: b58e662e
	'settings.group.startup': 'Démarrage',
	// source: bd6eb3cf
	'settings.group.browsing': 'Navigation',
	// source: 11bca4ac
	'settings.group.titleBar': 'Barre de titre',
	// source: 21ed2f9e
	'settings.group.deleting': 'Suppression',
	// source: 82bc68a6
	'settings.group.copying': 'Copie',
	// source: 3b2fe03e
	'settings.group.queue': 'File d’attente',
	// source: c560122a
	'settings.group.trash': 'Corbeille',
	// source: e404aa80
	'settings.group.archives': 'Archives',
	// source: 036dbf5b
	'settings.group.dropping': 'Dépôt',
	// source: 338c8ac8
	'settings.group.shelf': 'Étagère',
	// source: 604dce44
	'services.title': 'Services',
	// source: 18c11893
	'services.intro':
		'Ce qui fonctionne sur ce système et, le cas échéant, pourquoi quelque chose ne fonctionne pas. Waypoint masque les options qui ne peuvent pas fonctionner ici.',
	// source: a3b67f8c
	'services.loading': 'Vérification des services…',
	// source: 213333b5
	'services.features': 'Fonctionne : {features}',
	// source: e6744473
	'services.state.available': 'Disponible',
	// source: 274cb11a
	'services.state.partial': 'Partiellement disponible',
	// source: ca184496
	'services.state.unavailable': 'Indisponible',
	// source: 72ede065
	'services.name.file-system': 'Système de fichiers',
	// source: c560122a
	'services.name.trash': 'Corbeille',
	// source: 8cf37a4e
	'services.name.native-dnd': 'Glisser-déposer avec d’autres applications',
	// source: ed235159
	'services.name.waypoint-ops': 'Opérations sur les fichiers',
	// source: 104d1d96
	'services.name.waypoint-session': 'Fenêtres et onglets',
	// source: 74a883a0
	'services.name.waypoint-settings': 'Paramètres',
	// source: 30d10dfb
	'services.name.os-prefs': 'Préférences du système (horloge)',
	// source: 529d62bd
	'services.name.system-appearance': 'Apparence du système',
	// source: 3537cc36
	'services.name.window-manager': 'Gestionnaire de fenêtres',
	// source: 4b6a9dd9
	'services.name.window-tearoff': 'Détachement d’onglets',
	// source: f82d6466
	'services.name.thumbnails': 'Miniatures',
	// source: fc504ce5
	'services.note.thumbnails':
		'Les fichiers sur des serveurs n’ont d’aperçus que si leur serveur enregistré les active (Paramètres → Aperçus et miniatures); ils sont lus par la connexion de Waypoint et gardés en mémoire, jamais dans le dossier de miniatures partagé.',
	// source: b15a7fb6
	'services.name.volumes': 'Lecteurs et volumes',
	// source: 79cd7236
	'services.name.secrets': 'Trousseau et mots de passe enregistrés',
	// source: 77fe0e58
	'services.name.window-effects': 'Effets de fenêtre',
	// source: 97918f06
	'services.name.mime-apps': 'Types de fichiers et Ouvrir avec',
	// source: d068fa03
	'services.name.xdg-portal': 'Portail de bureau',
	// source: 3b23f28f
	'services.name.desktop-integration': 'Services de bureau',
	// source: 24a834c0
	'settings.group.notifications': 'Notifications et progression',
	// source: b15a7fb6
	'settings.group.devices': 'Lecteurs et volumes',
	// source: f0fc20bd
	'settings.integrations.rememberPassphrases.label':
		'Mémoriser les phrases secrètes des volumes chiffrés',
	// source: a38411f4
	'settings.integrations.rememberPassphrases.description':
		'Propose d’enregistrer une phrase secrète dans le trousseau du système lorsque vous déverrouillez un volume chiffré, et déverrouille d’elles-mêmes les volumes mémorisés lorsqu’ils sont branchés. Désactivé tant que vous ne l’activez pas.',
	// source: 8106f6bc
	'settings.group.fileManager': 'Gestionnaire de fichiers',
	// source: ca7eb278
	'settings.group.shortcut': 'Raccourci global',
	// source: ab21112e
	'settings.integrations.unreadable': 'Waypoint n’a pas pu vérifier ce que ce système peut faire.',
	// source: 1a4884ec
	'settings.integrations.notifications.label': 'Avertir à la fin d’une tâche',
	// source: 921f8ffa
	'settings.integrations.notifications.description':
		'Affiche une notification quand une tâche se termine ou échoue alors qu’aucune fenêtre de Waypoint n’est au premier plan ou après plus de 10 secondes d’exécution, et chaque fois qu’une tâche attend votre réponse. Un clic sur la notification ramène Waypoint au premier plan.',
	// source: a30790ca
	'settings.integrations.notificationActions.label': 'Afficher des boutons sur les notifications',
	// source: fef01152
	'settings.integrations.notificationActions.description':
		'Ajoute des boutons à la notification d’une tâche : Afficher dans le dossier à la fin, Afficher les détails en cas d’échec, Annuler après un déplacement ou une mise à la corbeille, et Remplacer, Ignorer et Garder les deux quand un fichier existe déjà. Certains bureaux n’affichent les boutons que lorsque la notification est développée, et d’autres n’offrent que le clic, qui ramène Waypoint au premier plan.',
	// source: df1c2178
	'settings.integrations.progress.label': 'Afficher la progression sur l’icône de l’application',
	// source: 50dc170e
	'settings.integrations.progress.description':
		'Montre où en sont les tâches en cours sur l’icône de Waypoint dans le dock ou la barre des tâches.',
	// source: 4d189222
	'settings.integrations.sleep.label': 'Garder l’ordinateur éveillé pendant les tâches',
	// source: 4f2047e2
	'settings.integrations.sleep.description':
		'Empêche l’ordinateur de se mettre en veille pendant l’exécution d’une tâche, puis lui permet de se mettre en veille de nouveau ensuite.',
	// source: d1c463e0
	'settings.integrations.fileManager.make.label':
		'Faire de Waypoint le gestionnaire de fichiers par défaut',
	// source: 1fb6dad3
	'settings.integrations.fileManager.make.description':
		'Les dossiers que d’autres applications ouvrent s’ouvriront dans Waypoint.',
	// source: f43b9425
	'settings.integrations.fileManager.make.action': 'Définir par défaut',
	// source: 192f28ef
	'settings.integrations.fileManager.settings.label':
		'Choisir Waypoint dans les paramètres du système',
	// source: e1b6c716
	'settings.integrations.fileManager.settings.description':
		'Windows ne permet qu’à vous de changer l’application par défaut. Ouvrez Applications par défaut, puis choisissez Waypoint pour les dossiers.',
	// source: ea7784e7
	'settings.integrations.fileManager.settings.action': 'Ouvrir Applications par défaut',
	// source: 98111ad9
	'settings.integrations.fileManager.state.default':
		'Waypoint est le gestionnaire de fichiers par défaut.',
	// source: 1b969bf3
	'settings.integrations.fileManager.state.other':
		'Le gestionnaire de fichiers par défaut est {name}.',
	// source: 6a0f8f01
	'settings.integrations.fileManager.failed':
		'Impossible de définir Waypoint par défaut : {reason}',
	// source: b073e448
	'settings.integrations.fileManager.service.label':
		'Ouvrir les dossiers que demandent d’autres applications',
	// source: 92f53136
	'settings.integrations.fileManager.service.description':
		'Tant que cette option est activée, « Afficher dans le dossier » et les demandes semblables d’autres applications s’ouvrent dans Waypoint, chaque dossier dans un nouvel onglet.',
	// source: 45f061ff
	'settings.integrations.shortcut.enabled.label':
		'Ramener Waypoint au premier plan avec un raccourci',
	// source: 669f91e1
	'settings.integrations.shortcut.enabled.description':
		'Fonctionne depuis n’importe quelle application : affiche la dernière fenêtre utilisée ou en ouvre une nouvelle.',
	// source: 5753ea37
	'settings.integrations.shortcut.key.label': 'Raccourci',
	// source: bdd944e8
	'settings.integrations.shortcut.key.description':
		'Une ou plusieurs des touches Ctrl, Alt, Maj et Super, puis une touche, comme Ctrl+Alt+W.',
	// source: 88f20221
	'settings.group.colours': 'Couleurs',
	// source: a5119091
	'settings.group.layout': 'Disposition',
	// source: eae96e02
	'settings.group.icons': 'Icônes',
	// source: c587c260
	'settings.group.vision': 'Vision',
	// source: 0e5ffb72
	'settings.group.motion': 'Mouvement et transparence',
	// source: 69f68950
	'settings.group.touch': 'Tactile',
	// source: f82d6466
	'settings.group.thumbnails': 'Miniatures',
	// source: 1c25dd0e
	'settings.group.serverPreviews': 'Fichiers sur les serveurs',
	// source: 15c08445
	'settings.group.language': 'Langue et formats',
	// source: 734e6606
	'settings.group.direction': 'Sens de la disposition',
	// source: d4b1ea57
	'settings.group.overview': 'Vue d’ensemble',
	// source: 1f2f41fc
	'settings.appearance.mode.label': 'Mode de couleur',
	// source: f732cd04
	'settings.appearance.mode.description': 'Clair, sombre ou celui du système.',
	// source: 6725e7bb
	'settings.appearance.mode.system': 'Système',
	// source: dbcd5e7b
	'settings.appearance.mode.light': 'Clair',
	// source: 60acc53f
	'settings.appearance.mode.dark': 'Sombre',
	// source: 1cea0a16
	'settings.appearance.source.label': 'Source du thème',
	// source: 3701f9c8
	'settings.appearance.source.description':
		'Les couleurs propres à Waypoint, ou le jeu de couleurs et l’accent du système.',
	// source: d7cfab3c
	'settings.appearance.source.liminal': 'Waypoint',
	// source: 6725e7bb
	'settings.appearance.source.os': 'Système',
	// source: 12376f4c
	'settings.appearance.accent.label': 'Couleur d’accent',
	// source: c89d9208
	'settings.appearance.accent.description':
		'Le texte qui s’y superpose garde toujours un contraste suffisant; une couleur trop claire ou trop foncée est ajustée.',
	// source: f3266ebe
	'settings.appearance.accent.ember': 'Orange Waypoint',
	// source: 6725e7bb
	'settings.appearance.accent.os': 'Système',
	// source: 6799c465
	'settings.appearance.accent.custom': 'Choisir une couleur',
	// source: e1535e71
	'settings.appearance.accentColour.label': 'Accent personnalisé',
	// source: 3f7017b3
	'settings.appearance.accentColour.description': 'Choisissez la couleur à utiliser.',
	// source: 07db0824
	'settings.appearance.systemColours.label': 'Adopter les couleurs du système',
	// source: b37ee2d6
	'settings.appearance.systemColours.description':
		'Dessine les surfaces, le texte et la sélection avec les couleurs du thème de votre bureau plutôt qu’avec celles de Waypoint. L’accent suit toujours son propre réglage, le texte garde un contraste d’au moins 4,5:1 et l’anneau de focus de 3:1, et l’apparence Waypoint n’est qu’à un clic.',
	// source: 0100f124
	'settings.appearance.systemColours.lifted':
		'Pour que tout reste lisible, Waypoint a ajusté {parts} par rapport aux couleurs de votre système.',
	// source: 41a1f654
	'settings.appearance.systemColours.part.text': 'le texte',
	// source: cd418405
	'settings.appearance.systemColours.part.selection': 'la sélection',
	// source: 401cb4f7
	'settings.appearance.systemColours.part.status':
		'les couleurs d’avertissement, d’erreur et de réussite',
	// source: c2c23e6d
	'settings.appearance.systemColours.part.focus': 'l’anneau de focus',
	// source: 7c5680e0
	'settings.appearance.systemColours.part.titleBar': 'la barre de titre',
	// source: a8f1cf41
	'settings.appearance.systemColours.highContrast':
		'Le contraste élevé est activé et utilise déjà les couleurs propres au système : ce réglage n’a donc aucun effet pour le moment.',
	// source: 5e2b42df
	'settings.appearance.systemColours.otherVariant':
		'Le mode de couleur ci-dessus est réglé sur une apparence que les couleurs de votre système n’ont pas, alors Waypoint dessine avec ses propres couleurs. Choisissez Système pour utiliser les couleurs du système.',
	// source: 77a283d6
	'settings.appearance.density.label': 'Densité',
	// source: 693b7292
	'settings.appearance.density.description': 'L’espace qu’occupent les listes et la fenêtre.',
	// source: 99452646
	'settings.appearance.density.compact': 'Compacte',
	// source: 459a23a5
	'settings.appearance.density.comfortable': 'Confortable',
	// source: 57edf234
	'settings.appearance.density.spacious': 'Aérée',
	// source: b1dcc728
	'settings.appearance.iconStyle.label': 'Style des icônes',
	// source: fe5eae80
	'settings.appearance.iconStyle.description':
		'L’épaisseur des icônes du thème Waypoint. Elle ne s’applique pas à Portage.',
	// source: dbcd5e7b
	'settings.appearance.iconStyle.light': 'Fin',
	// source: b455784a
	'settings.appearance.iconStyle.regular': 'Normal',
	// source: 94fee62e
	'settings.appearance.iconStyle.bold': 'Gras',
	// source: 3a1bc53f
	'settings.appearance.iconStyle.filled': 'Plein',
	// source: ca5d3a88
	'settings.appearance.iconTheme.label': 'Thème d’icônes',
	// source: b7e37496
	'settings.appearance.iconTheme.description':
		'L’ensemble d’icônes de fichiers et de dossiers que dessinent les vues de fichiers, la barre latérale et l’Étagère. Les icônes des barres d’outils et des menus ne changent pas.',
	// source: d7cfab3c
	'settings.appearance.iconTheme.waypoint': 'Waypoint',
	// source: 9485c24f
	'settings.appearance.iconTheme.portage': 'Portage',
	// source: 6725e7bb
	'settings.appearance.iconTheme.system': 'Système',
	// source: cd52621b
	'settings.appearance.iconTheme.systemUnavailable':
		'Le thème d’icônes Système n’est pas disponible sur ce système : {reason}',
	// source: 29b9017c
	'settings.appearance.iconTheme.systemUnavailableNoReason':
		'Le thème d’icônes Système n’est pas disponible sur ce système.',
	// source: 0ed86024
	'settings.appearance.iconStyle.systemNote':
		'Le style des icônes s’applique au thème d’icônes Waypoint. Le thème Système dessine les icônes que dessine votre système.',
	// source: 9eab1db2
	'settings.appearance.folderColour.label': 'Couleur des dossiers',
	// source: fdeb8a8e
	'settings.appearance.folderColour.description':
		'La couleur des dossiers dans le thème d’icônes Portage. La teinte suit le mode clair ou sombre.',
	// source: 5fb0dd13
	'settings.appearance.folderColour.waypointNote':
		'Les couleurs de dossier s’appliquent au thème d’icônes Portage. Le thème Waypoint dessine les dossiers dans la couleur d’accent.',
	// source: 10d42061
	'settings.appearance.folderColour.systemNote':
		'Les couleurs de dossier s’appliquent au thème d’icônes Portage. Le thème Système dessine les dossiers comme le fait votre système.',
	// source: 1c3f97b6
	'settings.appearance.folderColour.liminal': 'Liminal',
	// source: 8f6a75fa
	'settings.appearance.folderColour.gnome': 'GNOME',
	// source: c42f334f
	'settings.appearance.folderColour.cinnamon': 'Cinnamon',
	// source: 89fa0e1f
	'settings.appearance.folderColour.kde': 'KDE',
	// source: 254d3817
	'settings.appearance.folderColour.windows11': 'Windows 11',
	// source: ba19e9c3
	'settings.appearance.folderColour.red': 'Rouge',
	// source: bd38ed77
	'settings.appearance.folderColour.pink': 'Rose',
	// source: 78e7771b
	'settings.appearance.folderColour.orange': 'Orange',
	// source: 7d465fb9
	'settings.appearance.folderColour.purple': 'Mauve',
	// source: b1f7b2ee
	'settings.appearance.folderColour.rainbow': 'Arc-en-ciel',
	// source: a4fe6526
	'settings.language.language.label': 'Langue',
	// source: 9c4ce556
	'settings.language.language.description':
		'La langue des menus, des boîtes de dialogue et des messages, ainsi que la façon d’écrire les dates, les nombres et les tailles. Ce qui n’est pas encore traduit s’affiche en anglais.',
	// source: b0459211
	'settings.language.language.system': 'Valeur par défaut du système',
	// source: 55e947c6
	'settings.language.language.en-CA': 'English (Canada)',
	// source: 1abf744f
	'settings.language.language.fr-CA': 'Français (Canada)',
	// source: bede147e
	'settings.language.language.en-XA': 'English avec accents (en-XA, pour les développeurs)',
	// source: 579385ac
	'settings.language.language.ar-XB': 'Inversé, de droite à gauche (ar-XB, pour les développeurs)',
	// source: 9c8a9579
	'settings.language.direction.label': 'Sens',
	// source: 6449cca2
	'settings.language.direction.description':
		'Le sens de la disposition de la fenêtre. Automatique suit la langue; les autres servent à vérifier une disposition.',
	// source: d461a493
	'settings.language.direction.auto': 'Automatique',
	// source: d6c33263
	'settings.language.direction.ltr': 'De gauche à droite',
	// source: 909fb836
	'settings.language.direction.rtl': 'De droite à gauche',
	// source: d153cd62
	'settings.access.follow': 'Suivre le système',
	// source: 13001175
	'settings.access.on': 'Activé',
	// source: ca7981b4
	'settings.access.off': 'Désactivé',
	// source: 415a8336
	'settings.access.highContrast.label': 'Contraste élevé',
	// source: f0d0e780
	'settings.access.highContrast.description':
		'Noir sur blanc ou blanc sur noir, avec des bordures marquées et un accent bien visible.',
	// source: d68761cc
	'settings.access.textSize.label': 'Taille du texte',
	// source: ded569d7
	'settings.access.textSize.description':
		'Agrandit le texte et les lignes. La taille de texte du système compte aussi; la plus grande l’emporte.',
	// source: 32e48995
	'settings.access.textSize.100': '100 %',
	// source: 1b7769a7
	'settings.access.textSize.115': '115 %',
	// source: c6ac0a1f
	'settings.access.textSize.130': '130 %',
	// source: 3c64d7a7
	'settings.access.strongFocus.label': 'Anneau de focus renforcé',
	// source: 1e74a55a
	'settings.access.strongFocus.description':
		'Un contour plus épais autour de l’élément qui a le focus du clavier.',
	// source: 297ef4c7
	'settings.access.reducedMotion.label': 'Réduire les animations',
	// source: 6d3c26ba
	'settings.access.reducedMotion.description':
		'Aucun glissement, rebond ni fondu; les changements se font immédiatement.',
	// source: 326911d2
	'settings.access.reducedTransparency.label': 'Réduire la transparence',
	// source: d3e053f6
	'settings.access.reducedTransparency.description':
		'Dessine toutes les surfaces de façon opaque, quel que soit le réglage de la page Transparence.',
	// source: 849294f5
	'settings.access.touchMode.label': 'Mode tactile',
	// source: e9a5327f
	'settings.access.touchMode.description':
		'Cibles plus grandes, d’au moins 44 px. Automatique l’active après un toucher sur la fenêtre, ou en l’absence de souris.',
	// source: ca7981b4
	'settings.access.touchMode.off': 'Désactivé',
	// source: d461a493
	'settings.access.touchMode.auto': 'Automatique',
	// source: 13001175
	'settings.access.touchMode.on': 'Activé',
	// source: 244b3725
	'settings.ops.unreadable':
		'Les paramètres des opérations n’ont pas pu être lus; ils ne peuvent donc pas être modifiés pour le moment : {reason}',
	// source: 78cf79e7
	'settings.error.range': 'Choisissez une valeur entre {min} et {max}.',
	// source: c4af0661
	'settings.error.saveFailed': 'Le changement n’a pas pu être enregistré : {reason}',
	// source: c340dc01
	'settings.error.generic': 'une erreur s’est produite',
	// source: 4c438374
	'settings.group.backup': 'Sauvegarde et restauration',
	// source: 4431ea74
	'settings.backup.export.label': 'Exporter les paramètres',
	// source: 75ffb988
	'settings.backup.export.description':
		'Enregistrez vos paramètres dans un fichier que vous pouvez conserver ou charger sur un autre ordinateur.',
	// source: 3cd14bbe
	'settings.backup.export.action': 'Exporter les paramètres…',
	// source: 72f01a73
	'settings.backup.import.label': 'Importer les paramètres',
	// source: 10e4e1e8
	'settings.backup.import.description':
		'Remplacez vos paramètres par ceux d’un fichier exporté plus tôt.',
	// source: 5fcef179
	'settings.backup.import.action': 'Importer les paramètres…',
	// source: 4b7cc8ef
	'settings.backup.exported': 'Paramètres exportés vers {path}',
	// source: 476e61c7
	'settings.backup.imported': 'Paramètres importés.',
	// source: e6950042
	'settings.backup.nothingToChange':
		'Ce fichier correspond à vos paramètres actuels; rien n’a été modifié.',
	// source: 187d0662
	'settings.backup.confirm.title': 'Remplacer vos paramètres ?',
	// source: eb505954
	'settings.backup.confirm.description':
		'Vos paramètres actuels seront remplacés par ceux de ce fichier.',
	// source: badaab99
	'settings.backup.confirm.source': 'Ce fichier a été créé par Waypoint {version}.',
	// source: 6eb4bc42
	'settings.backup.confirm.changes': 'Ce qui va changer',
	// source: 95559635
	'settings.backup.confirm.replace': 'Remplacer mes paramètres',
	// source: 19766ed6
	'settings.backup.confirm.cancel': 'Annuler',
	// source: eae3c8cd
	'settings.backup.changes.one': '{count} modification',
	// source: 7f8fbadf
	'settings.backup.changes.many': '{count} de modifications',
	// source: 7f8fbadf
	'settings.backup.changes.other': '{count} modifications',
	// source: 3edaa6b5
	'settings.backup.group.ui': 'Barre de titre et barre d’actions',
	// source: a15aa3b0
	'settings.backup.group.folders': 'Vues mémorisées des dossiers',
	// source: 0c667864
	'settings.backup.group.connections': 'Connexions enregistrées',
	// source: aebd8a03
	'settings.backup.group.recent': 'Serveurs récents',
	// source: 9bee25a4
	'settings.backup.warning.unknownKeys.one':
		'{count} paramètre du fichier n’existe pas dans cette version de Waypoint; il sera ignoré.',
	// source: 809d1b7e
	'settings.backup.warning.unknownKeys.many':
		'{count} de paramètres du fichier n’existent pas dans cette version de Waypoint; ils seront ignorés.',
	// source: 809d1b7e
	'settings.backup.warning.unknownKeys.other':
		'{count} paramètres du fichier n’existent pas dans cette version de Waypoint; ils seront ignorés.',
	// source: 2dd1b0de
	'settings.backup.warning.unknownFiles.one':
		'{count} partie du fichier n’existe pas dans cette version de Waypoint; elle sera ignorée.',
	// source: 963668a0
	'settings.backup.warning.unknownFiles.many':
		'{count} de parties du fichier n’existent pas dans cette version de Waypoint; elles seront ignorées.',
	// source: 963668a0
	'settings.backup.warning.unknownFiles.other':
		'{count} parties du fichier n’existent pas dans cette version de Waypoint; elles seront ignorées.',
	// source: fc50de09
	'settings.backup.warning.unlistedEntries.one':
		'{count} fichier supplémentaire de l’archive n’a pas été lu.',
	// source: f8974337
	'settings.backup.warning.unlistedEntries.many':
		'{count} de fichiers supplémentaires de l’archive n’ont pas été lus.',
	// source: f8974337
	'settings.backup.warning.unlistedEntries.other':
		'{count} fichiers supplémentaires de l’archive n’ont pas été lus.',
	// source: 19cff21e
	'settings.backup.error.notABundle': 'Ce fichier n’est pas un fichier de paramètres Waypoint.',
	// source: 1df0fc70
	'settings.backup.error.corrupt':
		'Ce fichier est endommagé ou incomplet; il ne peut donc pas être lu.',
	// source: fb233e0d
	'settings.backup.error.newerFormat':
		'Ce fichier a été créé par une version plus récente de Waypoint. Mettez Waypoint à jour pour l’importer.',
	// source: 42186c50
	'settings.backup.error.tooLarge':
		'Ce fichier est trop volumineux pour être un fichier de paramètres.',
	// source: 98be8dfd
	'settings.backup.error.unsafe':
		'Cette archive contient des fichiers que Waypoint ne lira pas; rien n’a été importé.',
	// source: 07cf9157
	'settings.backup.error.invalid': 'Ce fichier contient un paramètre non permis : {detail}',
	// source: f4a5e6ed
	'settings.backup.error.nothing': 'Il n’y a aucun paramètre à exporter.',
	// source: 6a93db10
	'settings.backup.error.io': 'Le fichier n’a pas pu être lu ou écrit : {detail}',
	// source: 6aebff8d
	'settings.backup.error.unavailable':
		'Ce système n’a aucune boîte de dialogue de fichier que Waypoint peut utiliser.',
	// source: 92b2c4f7
	'settings.backup.error.stale':
		'Cette importation n’est plus prête. Choisissez de nouveau le fichier.',
	// source: 4927c4ba
	'settings.backup.error.apply':
		'L’importation a échoué; vos paramètres sont restés tels quels : {detail}',
	// source: cfa24c48
	'settings.general.startup.label': 'Au démarrage de Waypoint',
	// source: 8b9f3270
	'settings.general.startup.description':
		'Rouvrir les fenêtres et les onglets de la dernière fois, ou ouvrir une seule fenêtre dans le dossier personnel. Prend effet au prochain démarrage de Waypoint.',
	// source: 35e4dc9b
	'settings.general.startup.restore': 'Restaurer la session',
	// source: 029e8085
	'settings.general.startup.home': 'Ouvrir le dossier personnel',
	// source: 99f51518
	'settings.general.defaultView.label': 'Vue par défaut',
	// source: 2c066d7e
	'settings.general.defaultView.description':
		'La façon dont une nouvelle fenêtre affiche un dossier. Les fenêtres déjà ouvertes gardent leur vue.',
	// source: 6f202f54
	'settings.general.defaultView.list': 'Liste',
	// source: 0d7d12ac
	'settings.general.defaultView.grid': 'Grille',
	// source: 61477f03
	'settings.general.clickMode.label': 'Ouvrir les éléments avec',
	// source: 49f14de1
	'settings.general.clickMode.description':
		'Ouvrir les fichiers et les dossiers par un clic simple ou un double-clic.',
	// source: 8ffb032f
	'settings.general.clickMode.single': 'Clic simple',
	// source: 691f614c
	'settings.general.clickMode.double': 'Double-clic',
	// source: a7d466a3
	'settings.general.rememberFolderViews.label': 'Mémoriser la vue de chaque dossier',
	// source: 997a97f9
	'settings.general.rememberFolderViews.description':
		'Un dossier garde la vue, le tri, le regroupement, le choix des fichiers masqués et la taille des icônes que vous y avez choisis en dernier. Waypoint mémorise jusqu’à 1 000 dossiers et oublie celui modifié le plus anciennement au-delà. Désactivé, chaque dossier affiche la vue de la fenêtre; ce que les dossiers avaient mémorisé est conservé pour la prochaine activation.',
	// source: 5848f846
	'settings.general.showHidden.label': 'Afficher les fichiers masqués par défaut',
	// source: 9955f9ba
	'settings.general.appNameInTitle.label':
		'Afficher le nom de l’application dans la barre de titre',
	// source: 4f21c593
	'settings.general.appNameInTitle.description':
		'Fait commencer l’intitulé de la barre de titre par « Waypoint ». Désactivé, l’intitulé est simplement « Principale ».',
	// source: 2d9de7e6
	'menuBar.label': 'Barre de menus',
	// source: d47d7cb0
	'menuBar.more': 'Plus',
	// source: 982e1ab2
	'settings.general.menuBar.label': 'Afficher une barre de menus sous la barre de titre',
	// source: 1c421a68
	'settings.general.menuBar.description':
		'Place les menus de l’application (Fichier, Édition, Affichage et Fenêtre) dans une barre à part. Le bouton du nom de l’application reste alors une simple étiquette.',
	// source: da4880c1
	'settings.general.showHidden.description':
		'Une nouvelle fenêtre affiche les fichiers masqués. Les fenêtres déjà ouvertes gardent leur choix.',
	// source: ac54cdbd
	'settings.general.confirmTrash.label': 'Confirmer avant de déplacer vers la Corbeille',
	// source: fcf37ced
	'settings.general.confirmTrash.description':
		'Le déplacement vers la Corbeille peut être annulé; aucune confirmation n’est donc demandée, sauf si vous activez cette option. Supprimer définitivement demande toujours confirmation.',
	// source: 0bbdce6b
	'settings.operations.verify.label': 'Vérifier les copies après l’écriture',
	// source: 188770ed
	'settings.operations.verify.description':
		'Relit chaque fichier copié et le compare à l’original. Plus lent, mais utile pour les lecteurs amovibles.',
	// source: 8ceb4ed9
	'settings.operations.algorithm.label': 'Algorithme de somme de contrôle',
	// source: 4a8aeb54
	'settings.operations.algorithm.description': 'La somme de contrôle que la vérification compare.',
	// source: ce0985d9
	'settings.operations.algorithm.needsVerify':
		'Activez « Vérifier les copies après l’écriture » pour en choisir un.',
	// source: 80b5ebed
	'settings.operations.algorithm.blake3': 'BLAKE3 (plus rapide)',
	// source: bbd07c4f
	'settings.operations.algorithm.sha256': 'SHA-256',
	// source: 9e869d4d
	'settings.operations.concurrency.label': 'Opérations exécutées en même temps',
	// source: 9fd5e89a
	'settings.operations.concurrency.description':
		'Le nombre de copies, de déplacements et de suppressions exécutés ensemble; les autres attendent dans la file d’attente. Un changement s’applique à la prochaine opération.',
	// source: f1fac2cb
	'settings.operations.speedLimit.label': 'Limiter la vitesse des copies et des déplacements',
	// source: 3172f2e9
	'settings.operations.speedLimit.description':
		'Garde l’ensemble des copies et des déplacements sous la vitesse indiquée ci-dessous, ce qui aide sur un réseau ou pendant que vous travaillez. Un changement touche aussi les tâches déjà en cours.',
	// source: fc141f8a
	'settings.operations.speedLimitValue.label': 'Limite de vitesse',
	// source: 636ea90a
	'settings.operations.speedLimitValue.description':
		'La quantité maximale de données que les copies et les déplacements transfèrent chaque seconde, tous ensemble. Une tâche peut aussi avoir sa propre limite dans la fenêtre Opérations.',
	// source: 9559d7da
	'settings.operations.speedLimitValue.needsLimit':
		'Activez « Limiter la vitesse des copies et des déplacements » pour la définir.',
	// source: 15f75205
	'settings.operations.speedLimitValue.unit': 'Mo/s',
	// source: 7f82de8b
	'settings.operations.undoDepth.label': 'Profondeur de l’historique d’annulation',
	// source: 39eb28a9
	'settings.operations.undoDepth.description':
		'Le nombre d’opérations que la fonction Annuler peut remonter.',
	// source: a13cbe9e
	'settings.operations.trashExpiry.label': 'Vider les anciens éléments de la Corbeille',
	// source: 70a834dc
	'settings.operations.trashExpiry.description':
		'Supprime définitivement les éléments qui sont dans la Corbeille depuis plus longtemps que le nombre de jours ci-dessous. S’exécute au démarrage de Waypoint et est désactivé par défaut.',
	// source: d26a7b9f
	'settings.operations.trashDays.label': 'Supprimer les éléments de la Corbeille datant de plus de',
	// source: 9c5cd9ae
	'settings.operations.trashDays.description':
		'Compté à partir du jour où un élément a été déplacé vers la Corbeille.',
	// source: 82848f07
	'settings.operations.trashDays.needsExpiry':
		'Activez « Vider les anciens éléments de la Corbeille » pour le régler.',
	// source: ab51004e
	'settings.operations.trashDays.unit': 'jours',
	// source: 5ee25a32
	'settings.operations.archiveEntries.label':
		'Nombre maximal d’entrées d’une archive à extraire sans demander',
	// source: 4321ab2e
	'settings.operations.archiveEntries.description':
		'L’extraction d’une archive qui contient plus d’entrées que ce nombre demande d’abord confirmation.',
	// source: 87d05cd0
	'settings.operations.archiveEntries.unit': 'entrées',
	// source: ab83e81e
	'settings.operations.archiveBytes.label':
		'Taille maximale à laquelle une archive peut se déployer sans demander',
	// source: d4f3ddcd
	'settings.operations.archiveBytes.description':
		'L’extraction d’une archive dont le contenu dépasse cette taille demande d’abord confirmation.',
	// source: 66a1561b
	'settings.operations.archiveBytes.unit': 'Gio',
	// source: 90107fdb
	'settings.operations.archiveRatio.label':
		'Nombre maximal de fois sa taille qu’une archive peut atteindre',
	// source: 3ce2a2b4
	'settings.operations.archiveRatio.description':
		'Une archive qui se déploie en beaucoup plus que sa propre taille peut avoir été conçue pour remplir le disque; son extraction demande donc d’abord confirmation. Cette limite ne s’applique qu’au-delà de la taille ci-dessous.',
	// source: 2bf61a3b
	'settings.operations.archiveRatio.unit': 'fois',
	// source: 7f59a937
	'settings.operations.archiveFloor.label':
		'Vérifier le rapport de déploiement seulement au-delà de',
	// source: dc6d7677
	'settings.operations.archiveFloor.description':
		'Les petites archives très bien compressées ne posent pas de problème; le rapport n’est vérifié que lorsqu’une archive se déploie en au moins cette taille.',
	// source: dea6cb8d
	'settings.operations.archiveFloor.unit': 'Mio',
	// source: 34cee451
	'settings.operations.archiveReset.label': 'Limites des archives',
	// source: 1bfda070
	'settings.operations.archiveReset.description':
		'Rétablit les quatre limites ci-dessus : 1 000 000 d’entrées, 100 Gio, 1 000 fois la taille de l’archive, vérifié au-delà de 1 Gio.',
	// source: e240e635
	'settings.operations.archiveReset.action': 'Rétablir les valeurs par défaut',
	// source: 24ac24ba
	'settings.dnd.unavailable':
		'Le glissement de fichiers vers d’autres applications est indisponible : {reason}',
	// source: 33b7ab32
	'settings.dnd.unavailable.noReason': 'ce système ne le prend pas en charge.',
	// source: 0507f329
	'settings.dnd.rule.label': 'Action de dépôt par défaut',
	// source: e8bb978d
	'settings.dnd.rule.description':
		'Ce que fait le dépôt de fichiers quand aucune touche de modification n’est enfoncée. Ctrl copie, Maj déplace et Alt ouvre le sélecteur d’action, quel que soit ce réglage.',
	// source: ec757486
	'settings.dnd.rule.byVolume': 'Déplacer sur le même volume, copier sinon',
	// source: d6f17113
	'settings.dnd.rule.alwaysCopy': 'Toujours copier',
	// source: 3c427715
	'settings.dnd.rule.alwaysAsk': 'Toujours demander',
	// source: 4bd471cc
	'settings.dnd.spring.label': 'Délai d’ouverture au survol',
	// source: 98fe55ca
	'settings.dnd.spring.description':
		'La durée pendant laquelle un glissement survole un dossier, un onglet ou un élément de la barre latérale avant qu’il s’ouvre, de 200 à 2 000 millisecondes.',
	// source: f785c3ce
	'settings.dnd.spring.unit': 'ms',
	// source: 27c040c0
	'settings.dnd.shelf.label': 'Conserver l’Étagère d’une session à l’autre',
	// source: eb0c713a
	'settings.dnd.shelf.description':
		'L’Étagère garde ses éléments quand Waypoint se ferme puis redémarre.',
	// source: a9ff2643
	'settings.previews.unavailable': 'Les miniatures sont indisponibles sur ce système : {reason}',
	// source: 1025ebfc
	'settings.previews.unavailable.noReason': 'le service de miniatures n’a pas pu démarrer.',
	// source: 809997f6
	'settings.previews.show.label': 'Afficher les miniatures',
	// source: c994f3c3
	'settings.previews.show.description':
		'Dessine un aperçu d’une image, d’une vidéo ou d’un document dans la grille, dans les lignes hautes de la liste et sur l’Étagère, à la place de l’icône du type de fichier. Un fichier sans miniature garde son icône. Seuls les fichiers de cet ordinateur en reçoivent une.',
	// source: 4d681db9
	'settings.previews.max.label': 'Taille maximale d’un fichier pour créer une miniature',
	// source: b63c2654
	'settings.previews.max.description':
		'Un fichier plus gros garde son icône, afin qu’une image énorme ne ralentisse pas un dossier. Les miniatures déjà créées sont partagées avec d’autres gestionnaires de fichiers et s’affichent quelle que soit la taille.',
	// source: 1d09f6fa
	'settings.previews.max.unit': 'Mo',
	// source: cb2f4ea0
	'settings.previews.servers.none':
		'Aucun serveur enregistré. Les aperçus des fichiers d’un serveur se règlent pour chaque serveur enregistré; un serveur non enregistré n’en montre aucun.',
	// source: eabb2460
	'settings.previews.servers.changed': 'Aperçus de {server} : {choice}',
	// source: 2403a012
	'settings.previews.servers.failed': 'Les aperçus du serveur n’ont pas pu être changés.',
	// source: f76a11c3
	'settings.previews.measureHome.label':
		'Mesurer le dossier personnel à l’ouverture de la Vue d’ensemble',
	// source: 66b6863b
	'settings.previews.measureHome.description':
		'La Vue d’ensemble mesure votre dossier personnel pour lister ses plus gros dossiers et remplir « Vos fichiers » sur le volume qui le contient. Une mesure datant de moins d’une heure est réutilisée. Désactivez cette option pour ne mesurer que lorsque vous choisissez Mesurer maintenant.',
	// source: a96cf207
	'settings.transparency.unavailable': 'La transparence est indisponible sur ce système : {reason}',
	// source: 4a780a60
	'settings.transparency.unknown':
		'Waypoint n’a pas pu déterminer si ce système peut afficher le bureau à travers une fenêtre; les fenêtres restent donc opaques.',
	// source: 8a4066e7
	'settings.transparency.preview.caption':
		'Un exemple de fenêtre sur un fond d’écran clair et chargé, dessiné avec les réglages ci-dessous.',
	// source: f7efa7bc
	'settings.transparency.preview.sidebar': 'Barre latérale',
	// source: b4e929d8
	'settings.transparency.preview.title': 'Documents',
	// source: b3f857f6
	'settings.transparency.preview.file': 'Rapport.pdf',
	// source: ed077f3d
	'settings.transparency.preview.menu.open': 'Ouvrir',
	// source: 3064d79a
	'settings.transparency.preview.menu.rename': 'Renommer',
	// source: 9adcdf33
	'settings.transparency.preview.menu.trash': 'Déplacer vers la Corbeille',
	// source: 959f6d94
	'settings.transparency.preview.label': 'Aperçu d’une fenêtre translucide',
	// source: 3dc9f569
	'settings.transparency.experimental': 'Expérimental',
	// source: d5ca3b5f
	'settings.transparency.enable.label': 'Fenêtre transparente',
	// source: 61c9738e
	'settings.transparency.enable.description':
		'Laisse transparaître le bureau à travers la fenêtre. Désactivée tant que vous ne l’activez pas, et toujours désactivée en contraste élevé ou en transparence réduite.',
	// source: f79e4c86
	'settings.transparency.enable.experimental':
		'Sous Linux, cette fonction est expérimentale : son aspect dépend du compositeur et de WebKitGTK, et c’est la première chose à désactiver si une fenêtre paraît anormale.',
	// source: 556b40d7
	'settings.transparency.off.highContrast':
		'La transparence est désactivée tant que le contraste élevé est activé (Accessibilité).',
	// source: 8604baf4
	'settings.transparency.off.reducedTransparency':
		'La transparence est désactivée tant que la transparence réduite est activée (Accessibilité).',
	// source: 08379103
	'settings.transparency.off.unavailable':
		'La transparence est activée dans les paramètres, mais cette fenêtre ne peut pas être translucide.',
	// source: 3c0eaa9c
	'settings.transparency.off.unfocused':
		'Cette fenêtre est opaque tant qu’elle n’est pas au premier plan.',
	// source: ed104c1d
	'settings.transparency.opacity.label': 'Opacité de la barre de titre et de la barre de menus',
	// source: 1b3ebe3d
	'settings.transparency.opacity.description':
		'Le degré d’opacité de la barre de titre et de la barre de menus. Plus la valeur est basse, plus elles sont translucides.',
	// source: 8b92748b
	'settings.transparency.opacity.rows.label': 'Opacité des onglets et de la barre d’outils',
	// source: 84dba55f
	'settings.transparency.opacity.rows.description':
		'Le degré d’opacité des onglets, de la barre d’outils et de la barre d’état.',
	// source: b39491c7
	'settings.transparency.opacity.sidebar.label': 'Opacité de la barre latérale',
	// source: 1520a404
	'settings.transparency.opacity.sidebar.description':
		'Le degré d’opacité de la barre latérale, de l’Inspecteur et de l’Étagère.',
	// source: 42dc406b
	'settings.transparency.opacity.content.label': 'Opacité de la zone des fichiers',
	// source: 6704b6e2
	'settings.transparency.opacity.content.description':
		'Le degré d’opacité de la liste et de la grille.',
	// source: b43a26ec
	'settings.transparency.opacity.raised':
		'Pour que le texte reste lisible avec ce thème, cette partie reste opaque à au moins {percent} %.',
	// source: bbf3f11c
	'settings.transparency.opacity.unit': '%',
	// source: ff47e695
	'settings.transparency.blur.label': 'Flou derrière la fenêtre',
	// source: 88352b22
	'settings.transparency.blur.description':
		'Rend flou ce qui se trouve derrière la fenêtre là où il transparaît. Faible donne une matière douce, Élevé une matière plus marquée (Windows); sous Linux, les deux produisent le même flou.',
	// source: ca7981b4
	'settings.transparency.blur.off': 'Désactivé',
	// source: f793de20
	'settings.transparency.blur.low': 'Faible',
	// source: c4ebc6d4
	'settings.transparency.blur.high': 'Élevé',
	// source: 9b90ac35
	'settings.transparency.blur.unavailable': 'Le flou n’est pas disponible ici : {reason}',
	// source: 77abe354
	'settings.transparency.blur.unavailable.noReason':
		'ce système n’offre aucun moyen de rendre flou ce qui se trouve derrière une fenêtre.',
	// source: 19734a1b
	'settings.group.transparencyWindow': 'Fenêtre',
	// source: 324b134f
	'settings.group.transparencyPreview': 'Aperçu',
	// source: 6062652e
	'settings.group.transparencyOpacity': 'Opacité',
	// source: b608b7e1
	'settings.group.transparencyRegions': 'Parties de la fenêtre',
	// source: 610b7468
	'settings.group.transparencyReset': 'Valeurs par défaut',
	// source: e240e635
	'settings.transparency.reset.label': 'Rétablir les valeurs par défaut',
	// source: 31a594a4
	'settings.transparency.reset.description':
		'Remet chaque paramètre de cette page à sa valeur par défaut, sauf l’interrupteur de la fenêtre transparente.',
	// source: daee7606
	'settings.transparency.reset.action': 'Rétablir',
	// source: 93534e3e
	'settings.group.transparencyMenus': 'Menus',
	// source: 67afa7da
	'settings.group.transparencyFocus': 'Quand la fenêtre n’est pas au premier plan',
	// source: 2a85ee09
	'settings.transparency.regions.titleBar.label': 'Barre de titre, onglets et barre d’outils',
	// source: 483a7e58
	'settings.transparency.regions.titleBar.description':
		'La barre de titre, les onglets, la barre d’outils et la barre d’état. Désactivé, ils restent sur un arrière-plan opaque.',
	// source: a81e244d
	'settings.transparency.regions.sidebar.label': 'Barre latérale et volets latéraux',
	// source: 886cea35
	'settings.transparency.regions.sidebar.description':
		'La barre latérale, l’Inspecteur et l’Étagère. Désactivé, ils restent sur un arrière-plan opaque.',
	// source: f96275a1
	'settings.transparency.regions.content.label': 'Zone des fichiers',
	// source: ca521b02
	'settings.transparency.regions.content.description':
		'La liste et la grille. Désactivé, les fichiers restent sur un arrière-plan opaque.',
	// source: 34b1c65b
	'settings.transparency.menus.label': 'Menus translucides',
	// source: f3a31096
	'settings.transparency.menus.description':
		'Les menus contextuels et le menu de l’application laissent voir ce qui se trouve derrière eux, en flou.',
	// source: 4c0a89f5
	'settings.transparency.menuOpacity.label': 'Opacité des menus',
	// source: 2afa8f67
	'settings.transparency.menuOpacity.description':
		'Les menus restent presque opaques pour que leur texte soit facile à lire.',
	// source: 7e7e9009
	'settings.transparency.solidUnfocused.label': 'Opaque hors du premier plan',
	// source: 15bb64cd
	'settings.transparency.solidUnfocused.description':
		'Une fenêtre que vous n’utilisez pas est dessinée opaque et renonce à son flou, ce qui épargne aussi du travail au compositeur.',
	// source: 61a7a80b
	'settings.open.failed': 'Impossible d’ouvrir la fenêtre des paramètres.',
	// source: abc7e989
	'browse.list.label': 'Fichiers',
	// source: dcd1d522
	'browse.column.name': 'Nom',
	// source: 1af85190
	'browse.column.size': 'Taille',
	// source: e8ce5dca
	'browse.column.modified': 'Modifié',
	// source: f5387f9b
	'browse.column.kind': 'Genre',
	// source: d178c667
	'browse.column.original': 'Emplacement d’origine',
	// source: ce706088
	'browse.column.deleted': 'Date de suppression',
	// source: 71e7250c
	'browse.columns.label': 'Trier la liste',
	// source: 816773c6
	'browse.sort.ascending': 'tri croissant',
	// source: b8dcd319
	'browse.sort.descending': 'tri décroissant',
	// source: bda05058
	'browse.value.none': '—',
	// source: dc380888
	'browse.row.loading': 'Chargement',
	// source: 74ccd433
	'browse.group.folder': 'Dossier',
	// source: 1aa4cb0b
	'browse.group.image': 'Image',
	// source: bc1b8890
	'browse.group.audio': 'Audio',
	// source: d534be82
	'browse.group.video': 'Vidéo',
	// source: 66f4804e
	'browse.group.archive': 'Archive',
	// source: 340f4630
	'browse.group.code': 'Code',
	// source: d6bd8c0a
	'browse.group.document': 'Document',
	// source: 50009ce1
	'browse.group.other': 'Fichier',
	// source: 64d53e28
	'browse.group.pdf': 'Document PDF',
	// source: e7ad522e
	'browse.group.app': 'Application',
	// source: 0eaa5cb3
	'browse.group.text': 'Fichier texte',
	// source: 00271ff9
	'browse.group.markdown': 'Document Markdown',
	// source: f3befcc6
	'browse.group.spreadsheet': 'Chiffrier',
	// source: 5d5ee157
	'browse.group.presentation': 'Présentation',
	// source: 64d0b3ad
	'browse.group.font': 'Police de caractères',
	// source: 0f12c3ab
	'browse.group.diskImage': 'Image disque',
	// source: fa7fe671
	'browse.group.database': 'Base de données',
	// source: f1c216dd
	'browse.group.config': 'Fichier de configuration',
	// source: 82fbe842
	'browse.group.shellScript': 'Script shell',
	// source: 72cd8a64
	'browse.group.executable': 'Exécutable',
	// source: cedbee8e
	'browse.group.certificate': 'Certificat ou clé',
	// source: 752c2f3e
	'browse.group.ebook': 'Livre numérique',
	// source: 38149017
	'browse.group.torrent': 'Torrent',
	// source: d5d0a30b
	'browse.group.calendar': 'Calendrier',
	// source: 2b5c3d26
	'browse.group.contact': 'Contact',
	// source: a5e1b5fb
	'browse.group.log': 'Fichier journal',
	// source: f7dedb5c
	'browse.group.model3d': 'Modèle 3D',
	// source: 0ee695bd
	'browse.group.subtitles': 'Sous-titres',
	// source: 7af36c50
	'browse.group.playlist': 'Liste de lecture',
	// source: 59de121d
	'browse.group.package': 'Paquet',
	// source: 20cd41fc
	'browse.group.symlink': 'Lien symbolique',
	// source: c4d6bb20
	'browse.header.kind.folder': 'Dossiers',
	// source: be7e2f20
	'browse.header.kind.image': 'Images',
	// source: bc1b8890
	'browse.header.kind.audio': 'Audio',
	// source: c9a96394
	'browse.header.kind.video': 'Vidéos',
	// source: e404aa80
	'browse.header.kind.archive': 'Archives',
	// source: 340f4630
	'browse.header.kind.code': 'Code',
	// source: b4e929d8
	'browse.header.kind.document': 'Documents',
	// source: f456eff6
	'browse.header.kind.other': 'Autres fichiers',
	// source: ec1ff70c
	'browse.header.kind.pdf': 'Documents PDF',
	// source: 98e33b0f
	'browse.header.kind.app': 'Applications',
	// source: c5df156e
	'browse.header.kind.text': 'Fichiers texte',
	// source: 66343d4a
	'browse.header.kind.markdown': 'Documents Markdown',
	// source: fdae6602
	'browse.header.kind.spreadsheet': 'Chiffriers',
	// source: cf31f395
	'browse.header.kind.presentation': 'Présentations',
	// source: 3680fcad
	'browse.header.kind.font': 'Polices de caractères',
	// source: 56063f31
	'browse.header.kind.diskImage': 'Images disque',
	// source: 61d2afe2
	'browse.header.kind.database': 'Bases de données',
	// source: 1b82f5f5
	'browse.header.kind.config': 'Fichiers de configuration',
	// source: fa56112e
	'browse.header.kind.shellScript': 'Scripts shell',
	// source: 517ee2c8
	'browse.header.kind.executable': 'Exécutables',
	// source: bdb97523
	'browse.header.kind.certificate': 'Certificats et clés',
	// source: 38625b48
	'browse.header.kind.ebook': 'Livres numériques',
	// source: a2189b35
	'browse.header.kind.torrent': 'Torrents',
	// source: e0b2b00c
	'browse.header.kind.calendar': 'Calendriers',
	// source: b450645d
	'browse.header.kind.contact': 'Contacts',
	// source: 40986ada
	'browse.header.kind.log': 'Fichiers journaux',
	// source: e7f3a35d
	'browse.header.kind.model3d': 'Modèles 3D',
	// source: 0ee695bd
	'browse.header.kind.subtitles': 'Sous-titres',
	// source: dcd0a4d2
	'browse.header.kind.playlist': 'Listes de lecture',
	// source: 40a20853
	'browse.header.kind.package': 'Paquets',
	// source: c93f6883
	'browse.header.kind.symlink': 'Liens symboliques',
	// source: 2b065c7c
	'browse.header.modified.today': 'Aujourd’hui',
	// source: 56618125
	'browse.header.modified.yesterday': 'Hier',
	// source: ec2f4ce7
	'browse.header.modified.earlierThisWeek': 'Plus tôt cette semaine',
	// source: 0603deca
	'browse.header.modified.last7Days': '7 derniers jours',
	// source: f8f03fb4
	'browse.header.modified.last30Days': '30 derniers jours',
	// source: 98597183
	'browse.header.modified.thisYear': 'Cette année',
	// source: ad224bb8
	'browse.header.modified.unknown': 'Date inconnue',
	// source: 918a999d
	'browse.header.year': '{year}',
	// source: 5b8aa0bf
	'browse.header.size.unspecified': 'Non précisée',
	// source: c6c094bc
	'browse.header.size.empty': 'Vide',
	// source: 59c0aa41
	'browse.header.size.tiny': 'Minuscule (moins de 10 ko)',
	// source: 4667edf9
	'browse.header.size.small': 'Petite (10 à 100 ko)',
	// source: 86f1e6fc
	'browse.header.size.medium': 'Moyenne (100 ko à 1 Mo)',
	// source: 249df97f
	'browse.header.size.large': 'Grande (1 à 16 Mo)',
	// source: 554df513
	'browse.header.size.huge': 'Énorme (16 à 128 Mo)',
	// source: 933b853c
	'browse.header.size.gigantic': 'Gigantesque (plus de 128 Mo)',
	// source: f688e3a3
	'browse.header.name.symbols': 'Chiffres et symboles',
	// source: 3e6dce25
	'browse.header.type.none': 'Aucune extension',
	// source: a97dedba
	'browse.header.type.extension': 'Fichiers {extension}',
	// source: 208a19d5
	'browse.header.count.one': '{count} élément',
	// source: f65216b3
	'browse.header.count.many': '{count} d’éléments',
	// source: f65216b3
	'browse.header.count.other': '{count} éléments',
	// source: 85a43b58
	'browse.header.label.expanded': '{group}, {count}, développé',
	// source: 6351e8ab
	'browse.header.label.collapsed': '{group}, {count}, réduit',
	// source: c3ab98eb
	'browse.opening': 'Ouverture du dossier…',
	// source: d8a7d216
	'browse.scanning': 'Analyse en cours… {count} éléments trouvés jusqu’ici',
	// source: bd88d713
	'browse.empty': 'Ce dossier est vide.',
	// source: 37181158
	'browse.capped': 'Éléments affichés : {shown} sur {total}',
	// source: 05d0e5f0
	'browse.error.notFound.title': 'Dossier introuvable',
	// source: 0cf771f0
	'browse.error.notFound.detail': '{location} n’existe pas, ou a été déplacé ou supprimé.',
	// source: 59b77dbd
	'browse.error.permissionDenied.title': 'Permission refusée',
	// source: ee20e2b1
	'browse.error.permissionDenied.detail': 'Vous n’avez pas la permission d’ouvrir {location}.',
	// source: 78a92a5f
	'browse.error.notADirectory.title': 'Pas un dossier',
	// source: 565902db
	'browse.error.notADirectory.detail': '{location} est un fichier, pas un dossier.',
	// source: 31f72a24
	'browse.column.storageClass': 'Classe de stockage',
	// source: 574c7a90
	'browse.column.storageClass.show': 'Afficher la classe de stockage',
	// source: ef669154
	'browse.storageClass.standard': 'Standard',
	// source: 8139ea3a
	'browse.storageClass.standardIa': 'Standard-IA',
	// source: 5935b09f
	'browse.storageClass.onezoneIa': 'One Zone-IA',
	// source: 3ffe3450
	'browse.storageClass.intelligentTiering': 'Intelligent-Tiering',
	// source: e9e6bc18
	'browse.storageClass.reducedRedundancy': 'Redondance réduite',
	// source: f101b09b
	'browse.storageClass.glacierIr': 'Glacier Instant Retrieval',
	// source: f7a5dcb0
	'browse.storageClass.glacier': 'Glacier Flexible Retrieval',
	// source: 67ebe3a1
	'browse.storageClass.deepArchive': 'Glacier Deep Archive',
	// source: 92eee78c
	'browse.storageClass.expressOnezone': 'Express One Zone',
	// source: 28451b00
	'browse.storageClass.restoreNote': 'archivé : doit être restauré avant de pouvoir être lu',
	// source: 73da082d
	'browse.error.archived.title': 'Ce fichier est archivé',
	// source: 24c235f6
	'browse.error.archived.detail':
		'{location} se trouve dans une classe de stockage d’archives et doit être restauré avant de pouvoir être ouvert. Restaurez-le avec les outils du service; Waypoint ne lance jamais de restauration.',
	// source: fb86171c
	'browse.error.other.title': 'Ce dossier n’a pas pu être affiché',
	// source: dc6a434c
	'browse.error.other.detail': 'Une erreur s’est produite pendant la lecture du dossier.',
	// source: 3ff1d728
	'browse.error.corrupt.title': 'Cette archive est endommagée',
	// source: 330e6a0c
	'browse.error.corrupt.detail':
		'{location} ne peut pas être lue comme une archive. Elle est peut-être tronquée ou endommagée.',
	// source: 20f01ff9
	'browse.error.unsupported.title': 'Impossible de l’afficher comme un dossier',
	// source: 1a3bff82
	'browse.error.unsupported.detail': 'Waypoint ne peut pas l’ouvrir ici : {what}',
	// source: 6b411541
	'browse.selection.none': 'Aucun élément sélectionné',
	// source: f2015f5f
	'browse.selection.one': '{count} élément sélectionné',
	// source: b0bea8ba
	'browse.selection.many': '{count} d’éléments sélectionnés',
	// source: b0bea8ba
	'browse.selection.other': '{count} éléments sélectionnés',
	// source: 3db65f8c
	'nav.toolbar.label': 'Navigation',
	// source: d010ff39
	'nav.panels.label': 'Volets',
	// source: fe7178ff
	'nav.panels.more': 'Autres volets',
	// source: 76900f1b
	'nav.back': 'Précédent',
	// source: f1c65e14
	'nav.forward': 'Suivant',
	// source: 55490a4b
	'nav.up': 'Dossier parent',
	// source: 9e916de0
	'nav.history.back': 'Dossiers précédents',
	// source: cc318406
	'nav.history.forward': 'Dossiers suivants',
	// source: 15b61974
	'nav.path.crumbs': 'Emplacement',
	// source: d2aa1cf2
	'nav.path.edit': 'Modifier l’emplacement',
	// source: 5fec473a
	'nav.path.input': 'Saisissez un emplacement et appuyez sur Entrée',
	// source: e9a8f6ae
	'nav.path.invalid': '« {input} » n’est pas un emplacement que Waypoint peut ouvrir.',
	// source: ecaf97c4
	'nav.path.unsupported': 'Waypoint ne peut pas encore ouvrir les emplacements {what}.',
	// source: 602fffbd
	'nav.path.failed': 'Cet emplacement n’a pas pu être vérifié.',
	// source: 8e5ea509
	'tabs.strip.label': 'Onglets',
	// source: abc7e989
	'tabs.panel.label': 'Fichiers',
	// source: 1e08fda9
	'tabs.new': 'Nouvel onglet',
	// source: 6301612e
	'tabs.close': 'Fermer {title}',
	// source: 7a08c90e
	'tabs.scrollLeft': 'Faire défiler les onglets vers la gauche',
	// source: 449e09ff
	'tabs.scrollRight': 'Faire défiler les onglets vers la droite',
	// source: 4925fe5d
	'tabs.moved': '{title} déplacé à la position {position} sur {count}',
	// source: 219e92dc
	'tabs.count.one': '{count} onglet',
	// source: ba82764a
	'tabs.count.many': '{count} d’onglets',
	// source: ba82764a
	'tabs.count.other': '{count} onglets',
	// source: 6067c720
	'tabs.menu.label': 'Actions de l’onglet',
	// source: 2adf5c1e
	'tabs.menu.moveToNewWindow': 'Déplacer vers une nouvelle fenêtre',
	// source: 4fcdd857
	'tabs.menu.moveToWindow': 'Déplacer vers la fenêtre',
	// source: 8a0143d6
	'tabs.menu.windowEntry': '{title} — {tabs}',
	// source: 19734a1b
	'tabs.menu.untitledWindow': 'Fenêtre',
	// source: 39fe884b
	'tabs.announce.movedNewWindow': 'Déplacé vers une nouvelle fenêtre',
	// source: 0a2ba9bc
	'tabs.announce.openedWindow': 'Nouvelle fenêtre ouverte',
	// source: d0d0b67e
	'tabs.announce.movedToWindow': 'Déplacé : {name} vers {window}',
	// source: cc9a0dd1
	'tabs.announce.movedHere': 'Déplacé : {name} vers cette fenêtre',
	// source: dba3505d
	'tabs.announce.movedManyHere.one': '{count} onglet déplacé vers cette fenêtre',
	// source: 7ccfd079
	'tabs.announce.movedManyHere.many': '{count} d’onglets déplacés vers cette fenêtre',
	// source: 7ccfd079
	'tabs.announce.movedManyHere.other': '{count} onglets déplacés vers cette fenêtre',
	// source: 0cd781a6
	'window.notice.many':
		'De nombreuses fenêtres sont ouvertes. Chacune utilise de la mémoire; fermez celles dont vous n’avez plus besoin.',
	// source: b515269f
	'window.notice.limit':
		'Waypoint ne peut pas ouvrir plus de {limit} fenêtres. Fermez-en une d’abord.',
	// source: 5ea58049
	'window.notice.openFailed': 'Impossible d’ouvrir une nouvelle fenêtre.',
	// source: d1c8727d
	'window.notice.moveFailed': 'Impossible de le déplacer vers une autre fenêtre.',
	// source: 81386538
	'drag.pill.move': 'Relâcher pour déplacer {title} à la position {position} sur {count}',
	// source: 0b0a92af
	'drag.pill.moveGroup': 'Relâcher pour déplacer le groupe {name}',
	// source: 7ae94032
	'drag.pill.split': 'Relâcher pour diviser avec {name}',
	// source: 0b88a6ba
	'drag.pill.newGroup': 'Relâcher pour créer un nouveau groupe',
	// source: e6a314e7
	'drag.pill.addToGroup': 'Relâcher pour ajouter à {name}',
	// source: 457d2e89
	'drag.pill.leaveGroup': 'Relâcher pour quitter {name}',
	// source: 9483535e
	'drag.pill.splitPane': 'Diviser {edge} avec la vue actuelle',
	// source: 33b2b6e2
	'drag.pill.separate': 'Relâcher pour séparer la vue divisée',
	// source: 5462e746
	'drag.pill.newWindow': 'Relâcher pour ouvrir dans une nouvelle fenêtre',
	// source: e4fdd15b
	'drag.pill.mergeInto': 'Relâcher pour fusionner dans {name}',
	// source: 2155dc56
	'drag.pill.esc': 'Échap pour annuler',
	// source: 360f8403
	'drag.edge.left': 'à gauche',
	// source: 27042f4e
	'drag.edge.right': 'à droite',
	// source: 28720365
	'drag.edge.top': 'en haut',
	// source: be9b7607
	'drag.edge.bottom': 'en bas',
	// source: cd600a74
	'drag.zone.left': 'Diviser à gauche',
	// source: aa9997bb
	'drag.zone.right': 'Diviser à droite',
	// source: 906ea61e
	'drag.zone.top': 'Diviser en haut',
	// source: 7d9dc1fd
	'drag.zone.bottom': 'Diviser en bas',
	// source: a667335e
	'drag.announce.pill': 'Glissement : {text}',
	// source: bf2577ab
	'drag.announce.cancelled': 'Glissement annulé',
	// source: 19382c09
	'drag.announce.timedOut': 'Le glissement a pris fin parce qu’il durait trop longtemps',
	// source: 3c156ebd
	'drag.announce.movedNewWindow': 'Déplacé vers une nouvelle fenêtre : {name}',
	// source: b7a1b38d
	'drag.announce.merged': 'Fusionné : {name} dans {window}',
	// source: 95522fcd
	'drag.announce.landing':
		'Un onglet est glissé ici : relâchez pour l’ajouter à la position {position}',
	// source: a44b05c2
	'drag.announce.landingMany':
		'{count} onglets sont glissés ici : relâchez pour les ajouter à partir de la position {position}',
	// source: 208a19d5
	'dnd.items.one': '{count} élément',
	// source: f65216b3
	'dnd.items.many': '{count} d’éléments',
	// source: f65216b3
	'dnd.items.other': '{count} éléments',
	// source: 87446429
	'dnd.drag.one': 'Glissement de {name}',
	// source: 0cb4c07e
	'dnd.drag.items.one': 'Glissement de {count} élément',
	// source: b332ad92
	'dnd.drag.items.many': 'Glissement de {count} d’éléments',
	// source: b332ad92
	'dnd.drag.items.other': 'Glissement de {count} éléments',
	// source: 867a84e5
	'dnd.pill.copy': 'Copier {what} vers {target}',
	// source: 81ea6d06
	'dnd.pill.move': 'Déplacer {what} vers {target}',
	// source: 35f0f1dd
	'dnd.pill.upload': 'Téléverser {what} vers {target} sur {server}',
	// source: 3624414d
	'dnd.pill.download': 'Télécharger {what} de {server} vers {target}',
	// source: dd04625f
	'dnd.pill.across': 'Copier {what} vers {target} sur {server}',
	// source: fefb5e6a
	'dnd.pill.moveTo': 'Déplacer {what} vers {target} sur {server}',
	// source: e77fecea
	'dnd.pill.moveFrom': 'Déplacer {what} de {server} vers {target}',
	// source: b23b3ae7
	'dnd.pill.moveOrCopy': 'Déplacer ou copier {what} vers {target}',
	// source: baa189b1
	'dnd.pill.link': 'Créer un lien vers {what} dans {target}',
	// source: 3eb48039
	'dnd.pill.ask': 'Choisir quoi faire de {what} dans {target}',
	// source: 369c036a
	'dnd.pill.trash': 'Déplacer {what} vers la Corbeille',
	// source: dc190f24
	'dnd.pill.shelf': 'Ajouter {what} à l’Étagère',
	// source: 306ef19c
	'dnd.pill.openTab': 'Ouvrir dans un nouvel onglet',
	// source: fd166d05
	'dnd.pill.openTabs': 'Ouvrir dans de nouveaux onglets',
	// source: ab0222cc
	'dnd.pill.openSplit': 'Ouvrir dans une paire divisée',
	// source: c215b550
	'dnd.pill.openPane': 'Ouvrir {what} dans un nouveau panneau {edge}',
	// source: e28fda3c
	'dnd.pill.openInGroup': 'Ouvrir dans un nouvel onglet de {target}',
	// source: 7bb8ab98
	'dnd.pill.blocked': 'Non autorisé : {reason}',
	// source: 62134b0f
	'dnd.blocked.sameFolder': 'déjà dans {target}',
	// source: dc0fd8ca
	'dnd.blocked.intoItself': 'un dossier ne peut pas être placé dans lui-même',
	// source: 115c2e78
	'dnd.blocked.source': '{target} est en cours de glissement',
	// source: a0d8865f
	'dnd.blocked.readOnly': '{target} ne peut pas être modifié',
	// source: f31ba2fd
	'dnd.blocked.trashView': 'utilisez la Corbeille dans la barre latérale',
	// source: ea5089e8
	'dnd.blocked.trashSource': 'ces éléments ne peuvent pas être mis à la Corbeille d’ici',
	// source: 44b56bd9
	'dnd.blocked.shelfSource': 'les éléments de l’Étagère vont dans des dossiers',
	// source: b3390913
	'dnd.blocked.onShelf': 'ces éléments sont déjà sur l’Étagère',
	// source: 76b42b2f
	'dnd.blocked.alreadySplit': 'cet onglet est déjà divisé',
	// source: 7f808173
	'dnd.target.fileArea': 'la zone des fichiers',
	// source: cd2d06ba
	'dnd.blocked.unavailable': 'la Corbeille n’est pas disponible',
	// source: a4ba81b6
	'dnd.blocked.refused': '{reason}',
	// source: 6f5a6034
	'dnd.verb.copy': 'copier',
	// source: 683a62ce
	'dnd.verb.move': 'déplacer',
	// source: a87c991e
	'dnd.verb.moveOrCopy': 'déplacer ou copier',
	// source: b1b1bdb4
	'dnd.verb.link': 'créer un lien',
	// source: 9e4a70ed
	'dnd.verb.ask': 'demander quoi faire',
	// source: 5b88b228
	'dnd.verb.trash': 'déplacer vers la Corbeille',
	// source: 92eb0fca
	'dnd.verb.open': 'ouvrir dans un nouvel onglet',
	// source: 80b92eb2
	'dnd.verb.shelf': 'ajouter à l’Étagère',
	// source: fc52c665
	'dnd.announce.over': 'Au-dessus de {target} : action prévue, {action}',
	// source: 308a1926
	'dnd.announce.upload': 'Au-dessus de {target} sur {server} : action prévue, téléverser',
	// source: 1a77d5bb
	'dnd.announce.download': 'Au-dessus de {target} : action prévue, télécharger de {server}',
	// source: 67062914
	'dnd.announce.across': 'Au-dessus de {target} sur {server} : action prévue, copier',
	// source: 35d3078f
	'dnd.announce.blocked': 'Au-dessus de {target} : non autorisé, {reason}',
	// source: 7adc4570
	'dnd.announce.overSplit':
		'Au-dessus de la zone de division {edge} : action prévue, ouvrir {what} dans un nouveau panneau',
	// source: 76fa68e4
	'dnd.announce.sprungFolder': '{target} ouvert',
	// source: 4f7a4a81
	'dnd.announce.sprungTab': 'Affichage de {target}',
	// source: 1effc243
	'dnd.announce.sprungBack': 'Retour à l’endroit où le glissement a commencé',
	// source: 3eb48039
	'dnd.announce.picker': 'Choisir quoi faire de {what} dans {target}',
	// source: 83b635b3
	'dnd.announce.pickerCancelled': 'Dépôt annulé',
	// source: 88fb1a9c
	'dnd.announce.nothing': 'Rien n’a été déposé',
	// source: a50887d8
	'dnd.announce.left': 'Les fichiers ont quitté la fenêtre',
	// source: 4c6cb0de
	'dnd.out.started': 'Glissement de {what} hors de la fenêtre',
	// source: 36630b60
	'dnd.out.copied': '{what} déposé dans une autre application',
	// source: 4365d77b
	'dnd.out.moved': '{what} déplacé vers une autre application',
	// source: 77053198
	'dnd.out.linked': 'Lien vers {what} créé dans une autre application',
	// source: 4193e97c
	'dnd.out.failed': 'Le glissement hors de la fenêtre a échoué : {reason}',
	// source: 95977e26
	'dnd.out.refused':
		'Impossible de faire glisser {what} hors de la fenêtre; le glissement reste ici',
	// source: c9cc358e
	'dnd.out.unsupported': '{what} ne peut pas être glissé hors de la fenêtre',
	// source: deba875b
	'dnd.out.downloading': 'Téléchargement de {what} pour le glissement',
	// source: e8e2cb79
	'dnd.out.downloadFailed': '{what} n’a pas pu être téléchargé pour le glissement : {reason}',
	// source: 9cb5f3d4
	'dnd.picker.label': 'Action de dépôt',
	// source: 48f1b5e6
	'dnd.picker.copy': 'Copier ici',
	// source: 3768b486
	'dnd.picker.move': 'Déplacer ici',
	// source: 1541ac7d
	'dnd.picker.link': 'Créer un lien ici',
	// source: c5a7679b
	'dnd.picker.add': 'Ajouter à l’archive',
	// source: c95b4c2a
	'dnd.picker.compress': 'Compresser ici…',
	// source: f0064f1d
	'dnd.picker.extract': 'Extraire ici',
	// source: 19766ed6
	'dnd.picker.cancel': 'Annuler',
	// source: e21f935f
	'dnd.badge.copy': 'Copier',
	// source: 6ecc3df6
	'dnd.badge.move': 'Déplacer',
	// source: a6a32dbc
	'dnd.badge.link': 'Lier',
	// source: c7f93783
	'dnd.badge.ask': 'Choisir',
	// source: c560122a
	'dnd.badge.trash': 'Corbeille',
	// source: f0406a0e
	'dnd.badge.blocked': 'Non autorisé',
	// source: ed077f3d
	'dnd.badge.open': 'Ouvrir',
	// source: 338c8ac8
	'dnd.badge.shelf': 'Étagère',
	// source: 83bff4ce
	'dnd.open.failed': 'Impossible d’ouvrir les dossiers dans des onglets',
	// source: 58c4b3df
	'dnd.open.nothing': 'Aucun dossier à ouvrir',
	// source: 4327f6b9
	'dnd.open.tabs.one': '{count} onglet ouvert',
	// source: b31fac96
	'dnd.open.tabs.many': '{count} d’onglets ouverts',
	// source: b31fac96
	'dnd.open.tabs.other': '{count} onglets ouverts',
	// source: f20c8794
	'tabs.pinned': 'Épinglés',
	// source: 530ff53d
	'tabs.pinnedBadge': 'Onglet épinglé',
	// source: b9aa909d
	'tabs.colourDescription': 'Couleur : {colour}',
	// source: e3dab8aa
	'tabs.announce.pinned': 'Onglet {title} épinglé',
	// source: e8cd496c
	'tabs.announce.unpinned': 'Onglet {title} désépinglé',
	// source: 4e21c0df
	'tabs.announce.colour': 'Couleur de l’onglet {title} définie à {colour}',
	// source: 9a9ee7b9
	'tabs.announce.colourCleared': 'Couleur de l’onglet {title} retirée',
	// source: be791da3
	'tabs.announce.reopened': 'Onglet {title} rouvert',
	// source: eb1144d7
	'tabs.announce.noneClosed': 'Aucun onglet fermé à rouvrir',
	// source: 745242ce
	'tabs.announce.duplicated': 'Onglet {title} dupliqué',
	// source: c32266cf
	'tabs.announce.closedOthers': 'Les autres onglets ont été fermés',
	// source: ba9ac8ca
	'tabs.announce.closedRight': 'Les onglets à droite ont été fermés',
	// source: 49309b58
	'tabs.announce.nothingToClose': 'Aucun onglet à fermer',
	// source: 48845bff
	'notice.dismiss': 'Fermer',
	// source: a8283ade
	'notice.undo': 'Annuler',
	// source: 6201111b
	'pair.and': 'et',
	// source: df026268
	'pair.pill.label': 'Division : {first} et {second}',
	// source: ff22a46f
	'pair.divider.label': 'Redimensionner les panneaux',
	// source: b90311b2
	'pair.divider.value': '{first} % et {second} %',
	// source: 074a42d5
	'pair.pane.label': 'Panneau {position} sur {count} : {title}',
	// source: bde93980
	'pair.pane.close': 'Fermer le panneau {title}',
	// source: 92340695
	'pair.pane.active': 'Actif',
	// source: c9a3e2d6
	'pair.pane.grip': 'Glisser vers la bande d’onglets pour séparer',
	// source: b2f019fc
	'pair.layout.sideBySide': 'Côte à côte',
	// source: c2fed746
	'pair.layout.stacked': 'Empilés',
	// source: 92732568
	'pair.menu.label': 'Actions de la division',
	// source: 726c1269
	'pair.menu.separate': 'Séparer',
	// source: 56717b64
	'pair.menu.separateTabs': 'Séparer les onglets',
	// source: 285f601c
	'pair.menu.swap': 'Permuter les panneaux',
	// source: a831b4ca
	'pair.menu.duplicate': 'Dupliquer la division',
	// source: a5119091
	'pair.menu.layout': 'Disposition',
	// source: 2299ec0f
	'pair.menu.resetSizes': 'Rétablir les tailles',
	// source: 2adf5c1e
	'pair.menu.moveToNewWindow': 'Déplacer vers une nouvelle fenêtre',
	// source: 4fcdd857
	'pair.menu.moveToWindow': 'Déplacer vers la fenêtre',
	// source: b7e74da3
	'pair.menu.closeBoth': 'Fermer les deux',
	// source: 6de7f847
	'pair.menu.splitWith': 'Diviser avec',
	// source: c7fbdb62
	'pair.menu.splitWithNone': 'Aucun autre onglet',
	// source: a28568c5
	'pair.announce.split': '{title} divisé en deux panneaux',
	// source: 2c3b92f5
	'pair.announce.joined': 'Division de {first} et {second}',
	// source: 6b21d86d
	'pair.announce.separated': 'Séparés : {titles}',
	// source: 88a3b6e3
	'pair.announce.duplicateFailed': 'La division n’a pas pu être dupliquée',
	// source: 461716f9
	'pair.announce.splitUnavailable':
		'Impossible d’ouvrir dans un nouveau panneau : cet onglet est déjà divisé',
	// source: eb066e99
	'pair.announce.openedSplit': '{title} ouvert dans un nouveau panneau',
	// source: 30d20b34
	'pair.announce.splitFailed': '{title} n’a pas pu être ouvert dans un nouveau panneau',
	// source: 9b45e1ba
	'pair.announce.closedPane': 'Panneau {title} de la division fermé',
	// source: 8456f39e
	'pair.announce.closedPaneKept': '{closed} fermé; {kept} est maintenant un onglet seul',
	// source: 929c39be
	'pair.announce.closedBoth': 'Les deux onglets de la division ont été fermés',
	// source: c033fcb3
	'pair.announce.restored': 'Panneau {title} de la division restauré',
	// source: 4d94a715
	'pair.announce.undoFailed': 'Le panneau fermé n’a pas pu être restauré',
	// source: 23ced845
	'pair.announce.movedWindow': 'Déplacés vers une nouvelle fenêtre : {titles}',
	// source: 95e9e52b
	'pair.announce.movedToWindow': 'Déplacés : {titles} vers {window}',
	// source: 75e181c3
	'pair.announce.swapped': 'Panneaux permutés',
	// source: 2bcc0ff9
	'pair.announce.layout': 'Les panneaux sont maintenant {layout}',
	// source: f4bf7bd7
	'pair.announce.sizesReset': 'Tailles des panneaux rétablies',
	// source: 074a42d5
	'pair.announce.focused': 'Panneau {position} sur {count} : {title}',
	// source: f167afa9
	'pair.announce.pinned': 'Épinglés : {titles}',
	// source: 390223eb
	'pair.announce.unpinned': 'Désépinglés : {titles}',
	// source: 96650736
	'pair.announce.colour': 'Couleur de {titles} définie à {colour}',
	// source: 915b59e1
	'pair.announce.colourCleared': 'Couleur de {titles} retirée',
	// source: 6599b7e1
	'pair.announce.duplicated': 'Dupliqués : {titles}',
	// source: 223957f7
	'tabs.menu.pin': 'Épingler l’onglet',
	// source: 6d391af9
	'tabs.menu.unpin': 'Désépingler l’onglet',
	// source: 3a9dfa58
	'tabs.menu.colour': 'Couleur',
	// source: 70f8abaa
	'tabs.menu.pinGroup': 'Épingler le groupe « {name} »',
	// source: 471407f8
	'tabs.menu.unpinGroup': 'Désépingler le groupe « {name} »',
	// source: 12fc76e6
	'tabs.menu.addToGroup': 'Ajouter à un groupe',
	// source: 3726836b
	'tabs.menu.newGroup': 'Nouveau groupe',
	// source: 227d0352
	'tabs.menu.removeFromGroup': 'Retirer du groupe',
	// source: 57d3ddba
	'tabs.menu.duplicate': 'Dupliquer l’onglet',
	// source: f271892d
	'tabs.menu.close': 'Fermer l’onglet',
	// source: 29c8d716
	'tabs.menu.closeOthers': 'Fermer les autres onglets',
	// source: dc0c3647
	'tabs.menu.closeRight': 'Fermer les onglets à droite',
	// source: c5a521a2
	'tabs.menu.reopen': 'Rouvrir l’onglet fermé',
	// source: ec242bf5
	'tabs.menu.recentlyClosed': 'Fermés récemment',
	// source: 16b1bb7e
	'tabs.plusMenu.label': 'Actions de nouvel onglet',
	// source: b2f8fd63
	'tabs.plusMenu.newTab': 'Nouvel onglet',
	// source: 372d01ad
	'tabs.plusMenu.newTabHome': 'Nouvel onglet dans le dossier personnel',
	// source: 7469c2f6
	'tabs.plusMenu.newWindow': 'Nouvelle fenêtre',
	// source: dc937b59
	'tabs.colour.none': 'Aucune',
	// source: ba19e9c3
	'tabs.colour.red': 'Rouge',
	// source: 78e7771b
	'tabs.colour.orange': 'Orange',
	// source: 19dd83f1
	'tabs.colour.yellow': 'Jaune',
	// source: d486dfbd
	'tabs.colour.green': 'Vert',
	// source: fa15a5c1
	'tabs.colour.teal': 'Sarcelle',
	// source: ec7d56a0
	'tabs.colour.blue': 'Bleu',
	// source: 7d465fb9
	'tabs.colour.purple': 'Violet',
	// source: bd38ed77
	'tabs.colour.pink': 'Rose',
	// source: d4ac5809
	'tabs.colour.grey': 'Gris',
	// source: a9b44602
	'tabs.switcher.label': 'Changer d’onglet',
	// source: 149d9d29
	'tabs.switcher.candidate': '{title}, onglet {position} sur {count}',
	// source: 2845ce1e
	'tabs.switcher.cancelled': 'Changement d’onglet annulé',
	// source: 219e92dc
	'groups.tabCount.one': '{count} onglet',
	// source: ba82764a
	'groups.tabCount.many': '{count} d’onglets',
	// source: ba82764a
	'groups.tabCount.other': '{count} onglets',
	// source: 11ef7e52
	'groups.chip.label': '{name}, groupe d’onglets, {tabs}',
	// source: e286f4cf
	'groups.chip.activeInside': 'contient l’onglet actif',
	// source: 50555fe1
	'groups.chip.overLimit': 'plus de {limit} onglets',
	// source: f5664395
	'groups.tab.member': 'Groupe : {name}',
	// source: 762ebb70
	'groups.rename.label': 'Nom du groupe',
	// source: 1d5551da
	'groups.limit.warning':
		'{name} compte {count} onglets. Envisagez de le diviser en vue divisée ou en deuxième groupe.',
	// source: 9619649d
	'workspaces.save.label': 'Nom de l’espace de travail',
	// source: 8b70df34
	'workspaces.announce.saved': 'Espace de travail {name} enregistré',
	// source: 071381bb
	'workspaces.announce.nameTaken':
		'Un espace de travail nommé {name} existe déjà. Saisissez un autre nom.',
	// source: b8dfa51e
	'workspaces.announce.saveFailed': 'Impossible d’enregistrer l’espace de travail {name}.',
	// source: 72be053e
	'workspaces.announce.switched': 'Les favoris affichent maintenant l’espace de travail {name}',
	// source: b3661546
	'workspaces.announce.cleared': 'Les favoris affichent vos signets',
	// source: 695f06af
	'workspaces.announce.renamed': 'Espace de travail renommé en {name}',
	// source: 599abc91
	'workspaces.announce.deleted': 'Espace de travail {name} supprimé',
	// source: 80c0e06f
	'groups.announce.created': 'Groupe {name} créé',
	// source: e4d3d254
	'groups.announce.renamed': 'Groupe renommé en {name}',
	// source: ec0781ea
	'groups.announce.added': '{title} ajouté à {name}, qui compte maintenant {tabs}',
	// source: 6639fc60
	'groups.announce.removed': '{title} retiré de {name}',
	// source: 8d2bf8de
	'groups.announce.colour': 'Couleur du groupe {name} définie à {colour}',
	// source: aa1abace
	'groups.announce.colourCleared': 'Couleur du groupe {name} retirée',
	// source: 6b93bbbf
	'groups.announce.collapsed': 'Groupe {name} réduit, {tabs}',
	// source: ea71383f
	'groups.announce.expanded': 'Groupe {name} développé, {tabs}',
	// source: da2c7170
	'groups.announce.expandedForTab': 'Groupe {name} développé pour afficher {title}',
	// source: c5fcb987
	'groups.announce.collapsedOthers': 'Les autres groupes ont été réduits',
	// source: f4af2cb9
	'groups.announce.newTab': 'Nouvel onglet ouvert dans {name}',
	// source: 7596a603
	'groups.announce.pinned': 'Groupe {name} épinglé',
	// source: 1cad2ce4
	'groups.announce.unpinned': 'Groupe {name} désépinglé',
	// source: dc89acb4
	'groups.announce.sorted': 'Groupe {name} trié {by}',
	// source: 6ffcf433
	'groups.announce.duplicated': 'Groupe {name} dupliqué',
	// source: e4e9e3c0
	'groups.announce.movedWindow': 'Groupe {name} déplacé vers une nouvelle fenêtre',
	// source: 37135678
	'groups.announce.movedToWindow': 'Groupe {name} déplacé vers {window}',
	// source: 63e39984
	'groups.announce.ungrouped': 'Groupe {name} dissocié',
	// source: 3ab8e7c7
	'groups.announce.closed': 'Groupe {name} fermé',
	// source: 310a215e
	'groups.announce.moved': 'Groupe {name} déplacé à la position {position} sur {count}',
	// source: aa987cf1
	'groups.menu.label': 'Actions du groupe',
	// source: 42096aee
	'groups.menu.rename': 'Renommer le groupe',
	// source: 02a971e0
	'groups.menu.colour': 'Changer la couleur',
	// source: 35044c06
	'groups.menu.collapse': 'Réduire le groupe',
	// source: ed520c2c
	'groups.menu.expand': 'Développer le groupe',
	// source: c980778b
	'groups.menu.collapseOthers': 'Réduire tous les autres groupes',
	// source: 82c4b493
	'groups.menu.newTab': 'Nouvel onglet dans le groupe',
	// source: 03e50a7b
	'groups.menu.pin': 'Épingler le groupe',
	// source: 243d0152
	'groups.menu.unpin': 'Désépingler le groupe',
	// source: 0903e9b8
	'groups.menu.sort': 'Trier les onglets du groupe',
	// source: b393c1bc
	'groups.menu.sortName': 'Par nom',
	// source: 26a6b7d8
	'groups.menu.sortLocation': 'Par emplacement',
	// source: 959c6015
	'groups.menu.sortLocal': 'Locaux d’abord',
	// source: 85d2f64a
	'groups.menu.duplicate': 'Dupliquer le groupe',
	// source: ee71a012
	'groups.menu.saveWorkspace': 'Enregistrer le groupe comme espace de travail',
	// source: b227a3c4
	'groups.menu.moveWindow': 'Déplacer le groupe vers une nouvelle fenêtre',
	// source: 040b93e8
	'groups.menu.moveToWindow': 'Déplacer le groupe vers la fenêtre',
	// source: b57ef478
	'groups.menu.ungroup': 'Dissocier',
	// source: f0ef78be
	'groups.menu.close': 'Fermer le groupe',
	// source: 0bc4c2af
	'status.bar.label': 'Barre d’état',
	// source: 48a91de2
	'status.notWatched': 'Non surveillé',
	// source: 208a19d5
	'status.items.one': '{count} élément',
	// source: f65216b3
	'status.items.many': '{count} d’éléments',
	// source: f65216b3
	'status.items.other': '{count} éléments',
	// source: 9be87dc6
	'status.free': '{size} libres',
	// source: 67cab30d
	'status.measuringHome': 'Mesure du dossier personnel',
	// source: ed61df32
	'status.measuringHome.progress': 'Mesure du dossier personnel : {done} sur {total}',
	// source: 02c1d481
	'status.measuringHome.open': 'Afficher la Vue d’ensemble',
	// source: e9069c03
	'status.measuringHome.cancel': 'Annuler la mesure du dossier personnel',
	// source: 124ec382
	'status.openFailed': 'Impossible d’ouvrir {name}.',
	// source: 2fe7ff7b
	'status.copyPathFailed': 'Impossible de copier le chemin de {name}.',
	// source: a145120d
	'menu.entry.label': 'Actions de l’élément',
	// source: ed077f3d
	'menu.open': 'Ouvrir',
	// source: 35e74c3a
	'menu.openInNewTab': 'Ouvrir dans un nouvel onglet',
	// source: e8e0ccba
	'menu.openInNewWindow': 'Ouvrir dans une nouvelle fenêtre',
	// source: a034eb91
	'menu.openInSplit': 'Ouvrir dans un panneau divisé',
	// source: e7d0eeb7
	'menu.openWith': 'Ouvrir avec',
	// source: 0e026918
	'menu.copyPath': 'Copier le chemin',
	// source: e7d38be5
	'menu.addToShelf': 'Ajouter à l’Étagère',
	// source: ae43692b
	'menu.properties': 'Propriétés',
	// source: e03cc727
	'menu.propertiesInWindow': 'Propriétés dans une fenêtre',
	// source: 02cdaabf
	'menu.duplicate': 'Dupliquer',
	// source: f0064f1d
	'menu.extractHere': 'Extraire ici',
	// source: b9baace5
	'menu.extractTo': 'Extraire vers…',
	// source: 564208dc
	'menu.extractAll': 'Tout extraire',
	// source: 66b9d2cf
	'menu.compress': 'Compresser…',
	// source: 1f45f025
	'menu.cut': 'Couper',
	// source: e21f935f
	'menu.copy': 'Copier',
	// source: f3380f7b
	'menu.paste': 'Coller',
	// source: 700118c1
	'menu.pasteInto': 'Coller dans le dossier',
	// source: 30c11a8e
	'menu.copyTo': 'Copier vers…',
	// source: 3ace7f18
	'menu.moveTo': 'Déplacer vers…',
	// source: 1f97b2e6
	'menu.copyToOtherPane': 'Copier vers l’autre panneau',
	// source: 8ccd8e1f
	'menu.moveToOtherPane': 'Déplacer vers l’autre panneau',
	// source: 9adcdf33
	'menu.moveToTrash': 'Mettre à la Corbeille',
	// source: 48c015ad
	'menu.deletePermanently': 'Supprimer définitivement',
	// source: 18fdd549
	'menu.new': 'Nouveau',
	// source: 74ccd433
	'menu.new.folder': 'Dossier',
	// source: 50009ce1
	'menu.new.file': 'Fichier',
	// source: a8283ade
	'menu.undo': 'Annuler',
	// source: 74273989
	'menu.redo': 'Rétablir',
	// source: 2320aa1d
	'menu.undoNamed': 'Annuler {label}',
	// source: 3c9837fa
	'menu.redoNamed': 'Rétablir {label}',
	// source: dcc839a4
	'view.switcher.label': 'Affichage',
	// source: 6f202f54
	'view.list': 'Liste',
	// source: 0d7d12ac
	'view.grid': 'Grille',
	// source: 1882eb15
	'view.withShortcut': '{name} ({keys})',
	// source: a86cafc7
	'view.gridSize': 'Taille des icônes',
	// source: 3a15fd16
	'view.gridSize.value': '{size} pixels',
	// source: f7efa7bc
	'sidebar.label': 'Barre latérale',
	// source: eb5cfb73
	'sidebar.section.places': 'Emplacements',
	// source: d97d51d3
	'sidebar.section.favourites': 'Favoris',
	// source: 306fea87
	'sidebar.section.favouritesIn': 'Favoris · {name}',
	// source: 1377264b
	'sidebar.section.workspaces': 'Espaces de travail',
	// source: 785a1195
	'sidebar.view.label': 'Affichage de la barre latérale',
	// source: eb5cfb73
	'sidebar.view.places': 'Emplacements',
	// source: c4d6bb20
	'sidebar.view.folders': 'Dossiers',
	// source: d4b1ea57
	'sidebar.place.overview': 'Vue d’ensemble',
	// source: 3a786953
	'sidebar.place.home': 'Dossier personnel',
	// source: 9bd88f24
	'sidebar.place.desktop': 'Bureau',
	// source: b4e929d8
	'sidebar.place.documents': 'Documents',
	// source: d5fdc1af
	'sidebar.place.downloads': 'Téléchargements',
	// source: 1e624500
	'sidebar.place.pictures': 'Images',
	// source: 6eb00b4b
	'sidebar.place.music': 'Musique',
	// source: c9a96394
	'sidebar.place.videos': 'Vidéos',
	// source: c560122a
	'sidebar.place.trash': 'Corbeille',
	// source: e6a750b0
	'sidebar.trash.count.one': '{count} élément dans la Corbeille',
	// source: edb29ff0
	'sidebar.trash.count.many': '{count} d’éléments dans la Corbeille',
	// source: edb29ff0
	'sidebar.trash.count.other': '{count} éléments dans la Corbeille',
	// source: 5ec9185c
	'sidebar.trash.unavailable': 'La Corbeille ne peut pas être parcourue ici',
	// source: c0a188b3
	'sidebar.favourites.empty':
		'Aucun favori pour le moment. Ajoutez un dossier avec Ajouter aux favoris.',
	// source: c4d6bb20
	'sidebar.folders.tree': 'Dossiers',
	// source: 67e6116d
	'sidebar.folders.more': 'Dossiers affichés : {shown} sur {total}',
	// source: 5fd72d38
	'sidebar.rename.label': 'Nom du favori',
	// source: 3b81d9d8
	'sidebar.moved': '{name} déplacé à la position {position} sur {count}',
	// source: f4787f42
	'sidebar.favourites.failed': 'Impossible de modifier les favoris.',
	// source: 6f9a01d0
	'sidebar.workspaces.empty':
		'Aucun espace de travail pour le moment. Choisissez Enregistrer le groupe comme espace de travail sur un groupe d’onglets.',
	// source: dc937b59
	'sidebar.workspaces.none': 'Aucun',
	// source: 1377264b
	'sidebar.workspaces.list': 'Espaces de travail',
	// source: 9619649d
	'sidebar.workspaces.rename.label': 'Nom de l’espace de travail',
	// source: fc947772
	'sidebar.workspaces.failed': 'Impossible de modifier l’espace de travail.',
	// source: ce1f6133
	'sidebar.workspaces.noFolders': 'L’espace de travail {name} ne contient aucun dossier',
	// source: d6ed653c
	'sidebar.workspaces.openedAll.one': '{count} dossier ouvert depuis {name}',
	// source: dafe3383
	'sidebar.workspaces.openedAll.many': '{count} de dossiers ouverts depuis {name}',
	// source: dafe3383
	'sidebar.workspaces.openedAll.other': '{count} dossiers ouverts depuis {name}',
	// source: 8d16527c
	'sidebar.workspaces.openedSome': '{opened} dossiers sur {total} ouverts depuis {name}',
	// source: 66f56cc7
	'sidebar.menu.workspace': 'Actions de l’espace de travail',
	// source: 5a94468b
	'sidebar.menu.place': 'Actions de l’emplacement',
	// source: c7b9a857
	'sidebar.menu.trash': 'Actions de la Corbeille',
	// source: 1f25b95c
	'sidebar.menu.favourite': 'Actions du favori',
	// source: b7ea5346
	'sidebar.menu.folder': 'Actions du dossier',
	// source: 4ba5121d
	'sidebar.section.devices': 'Appareils',
	// source: 4ba5121d
	'devices.list': 'Appareils',
	// source: bf12e453
	'devices.empty': 'Aucun lecteur à afficher.',
	// source: a46cd4eb
	'devices.space': '{free} libres sur {total}',
	// source: c83caed5
	'devices.spaceUnknown': 'Espace libre indisponible',
	// source: 6ba9906e
	'devices.almostFull': 'Presque plein',
	// source: 75239499
	'devices.usage': '{percent} % utilisés',
	// source: a424e33d
	'devices.state.locked': 'Verrouillé',
	// source: 669cf449
	'devices.state.unmounted': 'Non monté',
	// source: eb5d5142
	'devices.state.unmountedSized': 'Non monté, {total}',
	// source: 5474eef8
	'devices.busy': 'En cours…',
	// source: d4536f25
	'devices.verb.mount': 'monter',
	// source: 6a7e1877
	'devices.verb.unmount': 'démonter',
	// source: bb9fec73
	'devices.verb.eject': 'éjecter',
	// source: 787600eb
	'devices.verb.unlock': 'déverrouiller',
	// source: 05637203
	'devices.action.mount': 'Monter {name}',
	// source: 659039bb
	'devices.action.unmount': 'Démonter {name}',
	// source: 61db18b7
	'devices.action.eject': 'Éjecter {name}',
	// source: cf2aac84
	'devices.action.unlock': 'Déverrouiller {name}',
	// source: 0ed2bbb7
	'devices.announce.mounted': '{name} monté',
	// source: d7ed1249
	'devices.announce.unmounted': '{name} démonté',
	// source: 7134efc0
	'devices.announce.ejected': '{name} éjecté. Vous pouvez le retirer sans risque.',
	// source: eec86972
	'devices.announce.unlocked': '{name} déverrouillé',
	// source: 2dca9f54
	'devices.announce.added': '{name} connecté',
	// source: bd422d50
	'devices.announce.removed': '{name} retiré',
	// source: 537e10ed
	'devices.error.busyBy':
		'Impossible d’effectuer l’action « {action} » sur {name}, car {by} l’utilise.',
	// source: 1646bc42
	'devices.error.busy':
		'Impossible d’effectuer l’action « {action} » sur {name}, car quelque chose l’utilise encore.',
	// source: 1641ca39
	'devices.error.notAuthorised':
		'Impossible d’effectuer l’action « {action} » sur {name} : l’autorisation n’a pas été accordée.',
	// source: 67a2875c
	'devices.error.unsupported':
		'Impossible d’effectuer l’action « {action} » sur {name} : ce système ne la prend pas en charge.',
	// source: 5f1f06be
	'devices.error.notFound':
		'Impossible d’effectuer l’action « {action} » sur {name} : il n’est plus là.',
	// source: 63ef3c83
	'devices.error.io': 'Impossible d’effectuer l’action « {action} » sur {name} : {message}',
	// source: 4caf4e4a
	'devices.error.unknown': 'Impossible d’effectuer l’action « {action} » sur {name}.',
	// source: 124ec382
	'devices.open.failed': 'Impossible d’ouvrir {name}.',
	// source: cf2aac84
	'devices.unlock.title': 'Déverrouiller {name}',
	// source: a20780f2
	'devices.unlock.description':
		'Saisissez la phrase secrète de ce volume chiffré. Waypoint ne l’enregistre pas.',
	// source: e7611f05
	'devices.unlock.field': 'Phrase secrète',
	// source: 4ac709aa
	'devices.unlock.confirm': 'Déverrouiller',
	// source: 19766ed6
	'devices.unlock.cancel': 'Annuler',
	// source: a4114da0
	'devices.unlock.working': 'Déverrouillage…',
	// source: a638d5d1
	'devices.unlock.wrong': 'Cette phrase secrète n’a pas déverrouillé le volume. Réessayez.',
	// source: 5bdc5e1d
	'devices.verb.forget': 'oublier la phrase secrète de',
	// source: 31aaa27a
	'devices.action.forget': 'Oublier la phrase secrète enregistrée de {name}',
	// source: b6afde60
	'devices.announce.forgotten': 'Phrase secrète enregistrée de {name} oubliée',
	// source: b84dd7c6
	'devices.announce.remembered': 'Phrase secrète de {name} enregistrée dans le trousseau',
	// source: 6a7e0a0d
	'devices.remember.reason.noKeyring': 'aucun trousseau n’est en cours d’exécution',
	// source: 1027e3ca
	'devices.remember.reason.keyringLocked': 'le trousseau est verrouillé',
	// source: 9e597ed9
	'devices.remember.failed':
		'{name} est déverrouillé, mais sa phrase secrète n’a pas été mémorisée : {reason}.',
	// source: 60007f11
	'devices.unlock.descriptionRemember':
		'Saisissez la phrase secrète de ce volume chiffré. Waypoint ne l’enregistre dans le trousseau que si vous cochez la case.',
	// source: c5e2885f
	'devices.unlock.remember': 'Mémoriser dans le trousseau',
	// source: 91bed1d7
	'devices.unlock.rememberHint':
		'Le volume se déverrouille ensuite de lui-même lorsqu’il est branché. Oubliez la phrase secrète depuis la ligne du volume.',
	// source: cf1068ce
	'devices.unlock.rememberUnavailable': 'La phrase secrète ne peut pas être mémorisée : {reason}.',
	// source: 62de0ed6
	'connect.title': 'Se connecter à un serveur',
	// source: 33756c40
	'connect.titleEdit': 'Modifier la connexion',
	// source: 444cc5d6
	'connect.description':
		'Tapez l’adresse d’un serveur ou remplissez les champs. Tester l’essaie sans l’enregistrer; Enregistrer le garde dans la section Réseau.',
	// source: 19766ed6
	'connect.cancel': 'Annuler',
	// source: 532eaabd
	'connect.test': 'Tester',
	// source: 407b7a04
	'connect.testing': 'Test en cours…',
	// source: 1509f561
	'connect.save': 'Enregistrer',
	// source: 35322b5b
	'connect.saveChanges': 'Enregistrer les modifications',
	// source: 1a2303ed
	'connect.connect': 'Se connecter',
	// source: 72021eb7
	'connect.connecting': 'Connexion…',
	// source: bc79cdff
	'connect.more': 'Plus d’options',
	// source: 83ad538b
	'connect.copyName': '{name} (copie)',
	// source: 56ef8f20
	'connect.field.address': 'Adresse',
	// source: cf088334
	'connect.field.protocol': 'Protocole',
	// source: 4a823118
	'connect.field.host': 'Hôte',
	// source: 72e9a59f
	'connect.field.port': 'Port',
	// source: b512d97e
	'connect.field.user': 'Utilisateur',
	// source: dcd1d522
	'connect.field.name': 'Nom',
	// source: 103475d6
	'connect.field.nameHint':
		'Le nom affiché dans la section Réseau. Laissez vide pour utiliser l’adresse.',
	// source: 11aaa142
	'connect.field.auth': 'Se connecter avec',
	// source: 33c7999c
	'connect.field.keyFile': 'Fichier de clé',
	// source: f1b38adb
	'connect.field.keyFileHint':
		'La clé privée, par exemple ~/.ssh/id_ed25519. Sa phrase secrète est demandée au besoin.',
	// source: e7cf3ef4
	'connect.field.password': 'Mot de passe',
	// source: 2cc09793
	'connect.field.passwordHint':
		'Laissez vide pour qu’il soit demandé à la connexion. Il n’est jamais enregistré avec la connexion.',
	// source: ce98d52f
	'connect.field.jumpHost': 'Hôte de rebond',
	// source: 6e97954f
	'connect.field.jumpHostHint':
		'Un serveur SSH par lequel passer d’abord, sous la forme utilisateur@hôte:port.',
	// source: ef05c31f
	'connect.field.startFolder': 'Dossier de départ',
	// source: ab9ceac3
	'connect.field.startFolderHint':
		'Le dossier ouvert au départ, par exemple /srv/media. Laissez vide pour le haut du serveur.',
	// source: 6daccb23
	'connect.field.refresh': 'Actualiser toutes les (secondes)',
	// source: ec848e04
	'connect.field.refreshHint':
		'Laissez vide pour actualiser seulement quand un dossier est affiché, avec Actualiser et après que Waypoint y écrit.',
	// source: 2ea0c143
	'connect.field.thumbnails': 'Afficher les aperçus des fichiers de ce serveur',
	// source: 118f5907
	'connect.field.thumbnailsHint':
		'Un aperçu lit le fichier sur le serveur : les aperçus sont donc désactivés à moins que vous les activiez.',
	// source: ca7981b4
	'connect.thumbnails.off': 'Désactivés',
	// source: 00ec8850
	'connect.thumbnails.smallFiles': 'Petits fichiers seulement',
	// source: b5115ac8
	'connect.thumbnails.always': 'Toujours (le petit aperçu intégré d’une grande photo)',
	// source: 3086c263
	'connect.field.thumbnailMaxMb': 'Plus grand fichier lu pour un aperçu (Mo)',
	// source: 14e659cc
	'connect.field.thumbnailMaxMbHint':
		'De 1 à 100; vide pour 2 Mo. Un fichier plus grand n’est jamais lu en entier pour un aperçu.',
	// source: a15c6adf
	'connect.auth.auto': 'Automatiquement',
	// source: 78a4438c
	'connect.auth.autoHint':
		'Essaie l’agent SSH et vos fichiers de clé, puis demande un mot de passe.',
	// source: e7cf3ef4
	'connect.auth.password': 'Mot de passe',
	// source: 394dd92f
	'connect.auth.passwordHint':
		'Se connecte avec un mot de passe, tapé ci-dessous ou demandé à la connexion.',
	// source: 33c7999c
	'connect.auth.keyFile': 'Fichier de clé',
	// source: c696dab1
	'connect.auth.keyFileHint': 'Se connecte avec le fichier de clé indiqué ci-dessous.',
	// source: cc3e0a54
	'connect.address.placeholder': 'sftp://utilisateur@hôte/dossier',
	// source: 30d79ebe
	'connect.address.hint':
		'Par exemple sftp://moi@nas.lan/srv. Les champs ci-dessous se remplissent pendant la saisie.',
	// source: f72619c9
	'connect.address.invalid': 'Ce n’est pas une adresse de serveur.',
	// source: 1a93af13
	'connect.address.unsupported': 'Waypoint ne peut pas se connecter aux serveurs {scheme} ici.',
	// source: be690623
	'connect.address.passwordDropped':
		'Le mot de passe de l’adresse n’a pas été gardé. Waypoint le demandera à la connexion.',
	// source: 0c667864
	'connect.saved.heading': 'Connexions enregistrées',
	// source: 729bceee
	'connect.saved.new': 'Nouvelle connexion',
	// source: 54aec582
	'connect.saved.empty': 'Aucune connexion n’est encore enregistrée.',
	// source: 2313fe21
	'connect.saved.edit': 'Modifier {name}',
	// source: fe06d16b
	'connect.saved.duplicate': 'Dupliquer {name}',
	// source: 02cdaabf
	'connect.saved.duplicateShort': 'Dupliquer',
	// source: 3cf8146e
	'connect.saved.remove': 'Supprimer {name}',
	// source: e2d0a549
	'connect.saved.removeShort': 'Supprimer',
	// source: 8e13216a
	'connect.forget.title': 'Supprimer « {name} »?',
	// source: f7097bc0
	'connect.forget.message':
		'La connexion est retirée de la section Réseau. Les onglets ouverts sur elle restent ouverts.',
	// source: 13f05d64
	'connect.forget.login': 'Oublier aussi son mot de passe enregistré',
	// source: e2d0a549
	'connect.forget.confirm': 'Supprimer',
	// source: c5e2885f
	'connect.remember.label': 'Mémoriser dans le trousseau',
	// source: f1830e4c
	'connect.remember.hint': 'Gardé seulement une fois accepté par le serveur.',
	// source: 56d20db3
	'connect.remember.unavailable':
		'Il ne peut pas être mémorisé : {reason}. Il dure jusqu’à la fermeture de Waypoint.',
	// source: 0327fb97
	'connect.remembered.kept': 'Le mot de passe est mémorisé dans le trousseau.',
	// source: 3646093f
	'connect.remembered.sessionOnly':
		'Le mot de passe n’a pas été mémorisé : {reason}. Il dure jusqu’à la fermeture de Waypoint.',
	// source: 6a7e0a0d
	'connect.keyring.noKeyring': 'aucun trousseau n’est en cours d’exécution',
	// source: 1027e3ca
	'connect.keyring.locked': 'le trousseau est verrouillé',
	// source: 9ffe6ac1
	'connect.keyring.failed': 'le trousseau n’a pas répondu',
	// source: 72021eb7
	'connect.result.trying': 'Connexion…',
	// source: bb4d8e69
	'connect.result.connected': 'Connecté à {server}.',
	// source: 7fee582f
	'connect.result.cancelled': 'Non connecté : annulé.',
	// source: 9148a257
	'connect.result.saved': '{name} enregistré.',
	// source: 5b076ba3
	'connect.result.updated': 'Modifications de {name} enregistrées.',
	// source: a0799a8a
	'connect.result.duplicated': 'Dupliqué sous le nom {name}.',
	// source: 1f9ebcfd
	'connect.result.removed': '{name} supprimé.',
	// source: fc25578d
	'connect.result.removedKeyring':
		'{name} supprimé. Son mot de passe n’a pas pu être oublié : {reason}.',
	// source: e34bd4c0
	'connect.problem.name':
		'Le nom est trop long ou contient des caractères qui ne peuvent pas être utilisés.',
	// source: c2a061d0
	'connect.problem.scheme': 'Choisissez un protocole avec lequel Waypoint peut se connecter.',
	// source: 5b7270d8
	'connect.problem.host': 'Tapez un nom d’hôte ou une adresse IP.',
	// source: 11014c22
	'connect.problem.user': 'Ce nom d’utilisateur ne peut pas être utilisé.',
	// source: 74289d15
	'connect.problem.password':
		'Un mot de passe n’a pas sa place dans l’adresse; Waypoint le demande à la connexion.',
	// source: 3475f551
	'connect.problem.port': 'Le port est un nombre de 1 à 65535.',
	// source: a6c60678
	'connect.problem.keyFile':
		'Indiquez le chemin complet du fichier de clé, ou un chemin commençant par ~/.',
	// source: ca592d84
	'connect.problem.jumpHost': 'Écrivez l’hôte de rebond sous la forme utilisateur@hôte:port.',
	// source: 70d04aac
	'connect.problem.startFolder':
		'Le dossier de départ est un chemin sur le serveur commençant par /.',
	// source: 9c534aa8
	'connect.problem.refresh': 'Actualisez toutes les 10 à 3600 secondes, ou laissez vide.',
	// source: bf2b151c
	'connect.problem.thumbnailMaxMb':
		'Lisez au plus de 1 à 100 Mo pour un aperçu, ou laissez le champ vide.',
	// source: 09d0d5fd
	'connect.problem.tooMany': 'Aucune autre connexion ne peut être enregistrée.',
	// source: c2ccccde
	'connect.problem.gone': 'Cette connexion n’est plus enregistrée.',
	// source: 98105ab5
	'connect.error.nameNotResolved': 'Non connecté : le nom d’hôte est introuvable.',
	// source: 7dca0896
	'connect.error.refused': 'Non connecté : le serveur a refusé la connexion.',
	// source: e0ad0a93
	'connect.error.noRoute': 'Non connecté : il n’y a pas de route vers le serveur.',
	// source: 48014f6e
	'connect.error.offline': 'Non connecté : cet ordinateur est hors ligne.',
	// source: ed4a8264
	'connect.error.clockSkew':
		'Non connecté : l’horloge de cet ordinateur est inexacte, donc le service a refusé la requête.',
	// source: 4ce5e64c
	'connect.error.archived': 'Ce fichier est archivé et doit d’abord être restauré.',
	// source: 2bdde388
	'connect.error.timeout': 'Non connecté : le serveur n’a pas répondu à temps.',
	// source: 7c6bac9a
	'connect.error.disconnected': 'La connexion a été perdue.',
	// source: 8af7408f
	'connect.error.authRequired': 'Non connecté : le serveur demande de vous connecter.',
	// source: 188fa9bb
	'connect.error.authFailed': 'Non connecté : le serveur n’a pas accepté l’identification.',
	// source: 72b3aa0e
	'connect.error.hostKeyUnknown': 'Non connecté : la clé du serveur n’a pas été approuvée.',
	// source: 52b64e99
	'connect.error.hostKeyChanged': 'Non connecté : la clé du serveur a changé.',
	// source: d6521db1
	'connect.error.certificate': 'Non connecté : le certificat du serveur n’est pas approuvé.',
	// source: 341f2ec5
	'connect.error.unsupported':
		'Non connecté : Waypoint ne peut pas se connecter aux serveurs {what} ici.',
	// source: 673f0784
	'connect.error.invalid': 'Non connecté : ce n’est pas une adresse de serveur.',
	// source: a9a569e1
	'connect.error.permissionDenied': 'Connecté, mais le dossier de départ ne peut pas être lu.',
	// source: 1f459ea8
	'connect.error.notFound': 'Connecté, mais le dossier de départ n’existe pas.',
	// source: 7fee582f
	'connect.error.cancelled': 'Non connecté : annulé.',
	// source: fc4e7217
	'connect.error.other': 'Non connecté : {detail}',
	// source: ad90c9b7
	'connect.scheme.sftp': 'SFTP (SSH)',
	// source: 12ba6a49
	'connect.scheme.smb': 'Partage Windows (SMB)',
	// source: fa3f6715
	'connect.scheme.davs': 'WebDAV (HTTPS)',
	// source: 26519330
	'connect.scheme.dav': 'WebDAV (HTTP, non chiffré)',
	// source: a4f63529
	'connect.s3.preset.aws': 'Amazon S3',
	// source: ed401873
	'connect.s3.preset.b2': 'Backblaze B2',
	// source: 9f2f1eda
	'connect.s3.preset.r2': 'Cloudflare R2',
	// source: 493861e0
	'connect.s3.preset.wasabi': 'Wasabi',
	// source: fb97d5d9
	'connect.s3.preset.minio': 'MinIO',
	// source: c3f48e3f
	'connect.s3.preset.spaces': 'DigitalOcean Spaces',
	// source: 05239b34
	'connect.s3.preset.custom': 'Autre service S3',
	// source: a7e7c005
	'connect.s3.moveNote':
		'S3 ne peut pas renommer : déplacer un fichier ici le copie sur le service, puis supprime l’original, ce qui n’est pas atomique.',
	// source: c83b6579
	'connect.field.bucket': 'Compartiment',
	// source: 14eb0f96
	'connect.field.keyId': 'ID de clé d’accès',
	// source: 3de6b754
	'connect.field.secretKey': 'Clé d’accès secrète',
	// source: 79b0aa9f
	'connect.field.secretKeyHint':
		'Envoyée au service pour signer les requêtes. Avec Mémoriser, elle va dans le trousseau du système, jamais dans les fichiers de Waypoint.',
	// source: 0cd7fb3e
	'connect.field.sessionToken': 'Jeton de session (facultatif)',
	// source: 78432a3a
	'connect.field.sessionTokenHint':
		'Pour des identifiants temporaires. Il n’est gardé que pour cette session et jamais mémorisé.',
	// source: d677190e
	'connect.field.s3Service': 'Service',
	// source: c59b0326
	'connect.field.s3Region': 'Région (facultatif)',
	// source: 539ed60c
	'connect.field.s3RegionHint':
		'Laissez vide pour que le service l’indique. Un compartiment Amazon S3 situé dans une autre région est trouvé automatiquement.',
	// source: d3a008ef
	'connect.field.s3ServiceRegion': 'Région',
	// source: f85e9b87
	'connect.field.s3ServiceRegionHint':
		'La région du service, par exemple us-west-004 ou eu-central-1.',
	// source: 919bb4cb
	'connect.field.s3Account': 'ID de compte',
	// source: 8466a295
	'connect.field.s3AccountHint': 'Votre ID de compte Cloudflare, indiqué dans le tableau de bord.',
	// source: aef7de28
	'connect.field.s3Host': 'Serveur',
	// source: faaf09b9
	'connect.field.s3HostHint':
		'L’adresse du serveur, par exemple minio.lan:9000. Écrivez d’abord http:// pour un serveur sans TLS.',
	// source: 3df9726c
	'connect.field.s3Endpoint': 'Point de terminaison',
	// source: d24f39c4
	'connect.field.s3EndpointHint':
		'L’adresse du service, par exemple https://s3.example.com. Écrivez http:// pour un serveur sans TLS.',
	// source: b52f65ad
	'connect.field.s3PathStyle': 'Mettre le compartiment dans le chemin',
	// source: 77b9470c
	'connect.field.s3PathStyleHint':
		'La plupart des services autres qu’Amazon S3 en ont besoin; Amazon S3 met le compartiment dans le nom d’hôte. Laissez ce réglage tel quel, sauf avis contraire du service.',
	// source: ecae8c1f
	'connect.problem.s3Bucket': 'Saisissez le nom du compartiment.',
	// source: a67e29b4
	'connect.problem.s3Endpoint': 'Saisissez l’adresse du service, par exemple minio.lan:9000.',
	// source: 1bdd5ad2
	'connect.problem.s3Region': 'Saisissez la région du service, par exemple us-west-004.',
	// source: 430dcf8f
	'connect.problem.s3Account': 'Saisissez votre ID de compte.',
	// source: 08b0263c
	'connect.problem.s3RegionName':
		'Une région est composée de lettres minuscules, de chiffres et de traits d’union, par exemple eu-west-1.',
	// source: 632d25f7
	'connect.address.placeholderS3': 's3://compartiment/dossier',
	// source: 80b078ca
	'connect.scheme.s3': 'Stockage S3',
	// source: f69ee155
	'connect.signIn.title': 'Se connecter à {server}',
	// source: 33215114
	'connect.signIn.passphraseTitle': 'Déverrouiller {subject}',
	// source: 023242f8
	'connect.signIn.description': 'Le serveur demande de vous connecter.',
	// source: 7adfac1b
	'connect.signIn.refused': 'Le serveur n’a pas accepté ces informations. Réessayez.',
	// source: e7cf3ef4
	'connect.signIn.password': 'Mot de passe',
	// source: e7611f05
	'connect.signIn.passphrase': 'Phrase secrète',
	// source: 14eb0f96
	'connect.signIn.keyId': 'ID de clé d’accès',
	// source: f47a99eb
	'connect.signIn.secretKey': 'Clé secrète',
	// source: bcc0bcc9
	'connect.signIn.confirm': 'Se connecter',
	// source: 2018c997
	'connect.hostKey.title': 'Faire confiance à {host}?',
	// source: 71c653f7
	'connect.hostKey.description':
		'Waypoint ne s’est jamais connecté à ce serveur. Comparez l’empreinte avec celle que vous donne l’administrateur du serveur avant de lui faire confiance.',
	// source: 4a823118
	'connect.hostKey.host': 'Hôte',
	// source: 90bb1f8c
	'connect.hostKey.algorithm': 'Type de clé',
	// source: ba7af0b7
	'connect.hostKey.fingerprint': 'Empreinte',
	// source: 3dc0fb22
	'connect.hostKey.once': 'Faire confiance une fois',
	// source: fe4435d5
	'connect.hostKey.remember': 'Faire confiance et mémoriser',
	// source: 726596c3
	'connect.hostKeyChanged.title': 'La clé de {host} a changé',
	// source: 75e4def7
	'connect.hostKeyChanged.description':
		'Le serveur présente une clé différente de celle enregistrée pour lui.',
	// source: 18777d45
	'connect.hostKeyChanged.warningTitle': 'Quelqu’un pourrait se faire passer pour ce serveur.',
	// source: e744fedf
	'connect.hostKeyChanged.warning':
		'Une clé modifiée peut signifier que le serveur a été réinstallé, ou que la connexion est interceptée. Ne vous connectez pas sans que l’administrateur du serveur confirme la nouvelle empreinte.',
	// source: d99646ed
	'connect.hostKeyChanged.recorded': 'Clé enregistrée',
	// source: 83d15e0f
	'connect.hostKeyChanged.offered': 'Clé présentée maintenant',
	// source: 31797dce
	'connect.hostKeyChanged.trust': 'Faire confiance à la nouvelle clé',
	// source: c1e1e958
	'connect.certificate.title': 'Faire confiance au certificat de {server}?',
	// source: 7c5ab765
	'connect.certificate.description':
		'Cet ordinateur ne fait pas confiance au certificat du serveur.',
	// source: 17b043ed
	'connect.certificate.subject': 'Délivré à',
	// source: 5f06f118
	'connect.certificate.issuer': 'Délivré par',
	// source: ed17874e
	'connect.certificate.reason': 'Raison du refus',
	// source: 3dc0fb22
	'connect.certificate.once': 'Faire confiance une fois',
	// source: aec527b7
	'connect.certificate.remember': 'Faire confiance pour cette connexion',
	// source: 1744b964
	'sidebar.section.network': 'Réseau',
	// source: 68d7beb6
	'network.list': 'Serveurs',
	// source: 17c3ced0
	'network.empty': 'Aucun serveur pour l’instant.',
	// source: aebd8a03
	'network.recent': 'Serveurs récents',
	// source: 8f755cb8
	'network.disconnect': 'Déconnecter {name}',
	// source: aac6defa
	'network.menu.label': 'Actions pour {name}',
	// source: ed077f3d
	'network.menu.open': 'Ouvrir',
	// source: 35e74c3a
	'network.menu.openInNewTab': 'Ouvrir dans un nouvel onglet',
	// source: 1a2303ed
	'network.menu.connect': 'Se connecter',
	// source: acfc5be7
	'network.menu.disconnect': 'Déconnecter',
	// source: 2b8a1a00
	'network.menu.edit': 'Modifier…',
	// source: 1359626e
	'network.menu.moveUp': 'Monter',
	// source: b58330ac
	'network.menu.moveDown': 'Descendre',
	// source: 9ce78fe3
	'network.menu.delete': 'Supprimer…',
	// source: a5d0d97e
	'network.menu.save': 'Enregistrer…',
	// source: a6bd489d
	'network.menu.forget': 'Oublier',
	// source: 12191a17
	'network.announce.state': '{name} : {state}',
	// source: 63f9679a
	'network.announce.disconnected': 'Déconnecté de {name}',
	// source: 6f44c71c
	'network.announce.moved': '{name} déplacé à la position {position}',
	// source: e835e6cb
	'network.announce.forgotten': '{name} oublié',
	// source: 5928abe3
	'network.announce.forgottenKeyring':
		'{name} oublié. Son mot de passe n’a pas pu être oublié : {reason}.',
	// source: 0303e182
	'remote.state.idle': 'Non connecté',
	// source: 72021eb7
	'remote.state.connecting': 'Connexion…',
	// source: 22965568
	'remote.state.connected': 'Connecté',
	// source: a1794783
	'remote.state.offline': 'Hors ligne',
	// source: 31e23ff0
	'remote.state.signIn': 'Identification requise',
	// source: 62c1939a
	'remote.state.trust': 'Clé à vérifier',
	// source: 54a0e8c1
	'remote.state.error': 'Erreur',
	// source: 420a97ee
	'remote.connecting': 'Connexion à {server}…',
	// source: bf8a9eab
	'remote.action.reconnect': 'Se reconnecter',
	// source: 1b4caa32
	'remote.action.signIn': 'Se connecter…',
	// source: aa06c59b
	'remote.action.review': 'Vérifier…',
	// source: 72021eb7
	'remote.action.connecting': 'Connexion…',
	// source: fc454c50
	'remote.announce.connected': 'Connecté à {server}',
	// source: cf1dd99d
	'remote.disconnected.title': 'Déconnecté de {server}',
	// source: 3c7680cd
	'remote.disconnected.detail':
		'La connexion a été perdue. Reconnectez-vous pour afficher le dossier de nouveau.',
	// source: 15e54df4
	'remote.unreachable.title': 'Impossible de joindre {server}',
	// source: fb727720
	'remote.unreachable.nameNotResolved':
		'Le nom d’hôte est introuvable. Vérifiez l’adresse ou votre réseau.',
	// source: 18a0f89d
	'remote.unreachable.refused': 'Le serveur a refusé la connexion : rien ne répond sur son port.',
	// source: 79b11368
	'remote.unreachable.noRoute': 'Il n’y a pas de route vers le serveur depuis ce réseau.',
	// source: d6ffcf69
	'remote.unreachable.offline': 'Cet ordinateur est hors ligne.',
	// source: f8c15c83
	'remote.clockSkew.title': 'L’horloge de cet ordinateur est inexacte',
	// source: 339bc34e
	'remote.clockSkew.detail':
		'{server} a refusé la requête parce que l’horloge de cet ordinateur diffère de la sienne. Réglez la date et l’heure automatiquement, puis reconnectez-vous.',
	// source: 6279ed15
	'remote.timeout.title': '{server} ne répond pas',
	// source: 8392b044
	'remote.timeout.detail': 'Le serveur n’a pas répondu à temps.',
	// source: f69ee155
	'remote.signIn.title': 'Se connecter à {server}',
	// source: dfc5634c
	'remote.signIn.detail': 'Le serveur demande de vous connecter avant d’afficher ce dossier.',
	// source: 774f9d0c
	'remote.signIn.refused': 'Le serveur n’a pas accepté l’identification. Réessayez.',
	// source: c3209923
	'remote.hostKey.title': '{server} n’est pas encore approuvé',
	// source: 22cb7210
	'remote.hostKey.detail':
		'Waypoint ne s’est jamais connecté à ce serveur. Vérifiez sa clé avant de lui faire confiance.',
	// source: 2118f0ba
	'remote.hostKeyChanged.title': 'La clé de {server} a changé',
	// source: d2d8a249
	'remote.hostKeyChanged.detail':
		'Quelqu’un pourrait se faire passer pour ce serveur. Waypoint ne s’est pas connecté.',
	// source: 37702fbc
	'remote.certificate.title': 'Le certificat de {server} n’est pas approuvé',
	// source: 998b069b
	'remote.certificate.detail': 'Vérifiez le certificat avant de lui faire confiance.',
	// source: 2a0ed3de
	'remote.error.title': 'Impossible d’afficher ce dossier sur {server}',
	// source: be69cd59
	'remote.error.detail': 'Un problème est survenu avec la connexion.',
	// source: 353dab45
	'tabs.remote': 'Sur un serveur : {state}',
	// source: c9fa6e7e
	'nav.path.passwordDropped':
		'Le mot de passe n’a pas été gardé; Waypoint le demandera à la connexion. Appuyez sur Entrée pour y aller.',
	// source: 3064d79a
	'menu.rename': 'Renommer',
	// source: 298167e2
	'menu.renameSelected': 'Renommer la sélection…',
	// source: a76e13b9
	'menu.restore': 'Restaurer',
	// source: b85cf088
	'menu.emptyTrash': 'Vider la Corbeille',
	// source: ce706088
	'menu.sort.deleted': 'Date de suppression',
	// source: 865d27ef
	'menu.removeFromFavourites': 'Retirer des favoris',
	// source: cbba1e60
	'menu.addToFavourites': 'Ajouter aux favoris',
	// source: 31cacc71
	'menu.openAllInTabs': 'Tout ouvrir dans des onglets',
	// source: 9efc82d7
	'menu.deleteWorkspace': 'Supprimer l’espace de travail',
	// source: 1359626e
	'menu.moveUp': 'Monter',
	// source: b58330ac
	'menu.moveDown': 'Descendre',
	// source: b7ea5346
	'menu.background.label': 'Actions du dossier',
	// source: c9129025
	'menu.sortBy': 'Trier par',
	// source: dcd1d522
	'menu.sort.name': 'Nom',
	// source: 1af85190
	'menu.sort.size': 'Taille',
	// source: e8ce5dca
	'menu.sort.modified': 'Modifié',
	// source: f5387f9b
	'menu.sort.kind': 'Genre',
	// source: 79479a6c
	'menu.sort.descending': 'Décroissant',
	// source: 6f94fb69
	'menu.sort.foldersFirst': 'Dossiers en premier',
	// source: 956a51f6
	'menu.groupBy': 'Regrouper par',
	// source: a638b9f6
	'menu.group.none': 'Aucun regroupement',
	// source: f5387f9b
	'menu.group.kind': 'Genre',
	// source: e8ce5dca
	'menu.group.modified': 'Modifié',
	// source: 1af85190
	'menu.group.size': 'Taille',
	// source: dcd1d522
	'menu.group.name': 'Nom',
	// source: baaddf70
	'menu.group.type': 'Type',
	// source: 0e916101
	'cmd.refresh': 'Actualiser',
	// source: ee003ee2
	'cmd.resetFolderView': 'Réinitialiser la vue de ce dossier',
	// source: a638b9f6
	'cmd.group.none': 'Aucun regroupement',
	// source: c8677275
	'cmd.group.kind': 'Regrouper par genre',
	// source: de0ae2b3
	'cmd.group.modified': 'Regrouper par date de modification',
	// source: 93bdc053
	'cmd.group.size': 'Regrouper par taille',
	// source: 7d7fa750
	'cmd.group.name': 'Regrouper par nom',
	// source: ae9db192
	'cmd.group.type': 'Regrouper par type',
	// source: b5ede7fb
	'menu.showHidden': 'Afficher les fichiers masqués',
	// source: addfc2c5
	'files.default.folder': 'dossier sans titre',
	// source: 19be8b57
	'files.default.file': 'fichier sans titre',
	// source: 8fcfae1d
	'files.readOnly': 'Cet emplacement ne peut pas être modifié.',
	// source: 84adb9fa
	'files.extract.nothing': 'Aucun des éléments sélectionnés n’est une archive.',
	// source: 034bb0fb
	'files.extract.limit.title': 'Extraire une très grosse archive?',
	// source: 54549af9
	'files.extract.limit.message':
		'{name} dépasse les limites prévues pour les archives ({reason}). L’extraire pourrait remplir le disque.',
	// source: 05f8be5b
	'files.extract.limit.action': 'Extraire quand même',
	// source: 3608510b
	'files.extract.leftOut.one':
		'{count} entrée de l’archive a été laissée de côté : son nom ou son lien n’était pas sûr.',
	// source: a82b46a4
	'files.extract.leftOut.many':
		'{count} d’entrées de l’archive ont été laissées de côté : leurs noms ou leurs liens n’étaient pas sûrs.',
	// source: a82b46a4
	'files.extract.leftOut.other':
		'{count} entrées de l’archive ont été laissées de côté : leurs noms ou leurs liens n’étaient pas sûrs.',
	// source: b5351143
	'files.extract.unlockFailed': 'Le mot de passe de l’archive n’a pas pu être transmis : {reason}',
	// source: d92e94e8
	'files.extract.stillLocked': 'L’archive est toujours verrouillée.',
	// source: 1097d4a6
	'files.compress.nothing': 'Sélectionnez d’abord ce qu’il faut compresser.',
	// source: 3007b678
	'files.nothingSelected': 'Rien n’est sélectionné.',
	// source: 9da785e5
	'files.nothingFocused': 'Rien à renommer. Placez-vous d’abord sur un élément.',
	// source: 0892193c
	'files.noQueue': 'Les opérations ne sont pas disponibles dans cette fenêtre.',
	// source: 7be1a00f
	'files.failed': 'Impossible de terminer : {reason}',
	// source: 9f3ece29
	'files.undo.nothing': 'Il n’y a rien à annuler.',
	// source: 00247184
	'files.redo.nothing': 'Il n’y a rien à rétablir.',
	// source: 19766ed6
	'files.cancel': 'Annuler',
	// source: 8e4f688c
	'files.trash.confirm.title': 'Mettre à la Corbeille?',
	// source: b827ade6
	'files.trash.confirm.named': 'Mettre {name} à la Corbeille? Vous pouvez annuler cette action.',
	// source: 02c3089d
	'files.trash.confirm.one':
		'Mettre {count} élément à la Corbeille? Vous pouvez annuler cette action.',
	// source: 5f673729
	'files.trash.confirm.many':
		'Mettre {count} d’éléments à la Corbeille? Vous pouvez annuler cette action.',
	// source: 5f673729
	'files.trash.confirm.other':
		'Mettre {count} éléments à la Corbeille? Vous pouvez annuler cette action.',
	// source: 9adcdf33
	'files.trash.confirm.action': 'Mettre à la Corbeille',
	// source: c46b644a
	'files.trash.unavailable.title': 'La Corbeille n’est pas disponible ici',
	// source: 90cc61df
	'files.trash.unavailable.message':
		'Ces éléments ne peuvent pas être mis à la Corbeille ({reason}). Vous pouvez les supprimer définitivement à la place, mais cette action est irréversible.',
	// source: c1b15a37
	'files.delete.title': 'Supprimer définitivement?',
	// source: 60f3dc08
	'files.delete.intro.one':
		'Ceci supprime définitivement l’élément ci-dessous. Cette action est irréversible.',
	// source: 76019696
	'files.delete.intro.many':
		'Ceci supprime définitivement {count} d’éléments. Cette action est irréversible.',
	// source: 76019696
	'files.delete.intro.other':
		'Ceci supprime définitivement {count} éléments. Cette action est irréversible.',
	// source: 1a261270
	'files.delete.more': 'et {count} de plus',
	// source: f10a6251
	'files.delete.size': 'Taille totale : {size}',
	// source: 0fcb5ed7
	'files.delete.list.label': 'Éléments à supprimer',
	// source: 726d6087
	'files.paste.intoArchive':
		'Les éléments sont ajoutés à une archive, et non déplacés dedans : les originaux restent où ils sont.',
	// source: 48c015ad
	'files.delete.action': 'Supprimer définitivement',
	// source: 491ac3fa
	'archive.rewrite.delete.title': 'Supprimer de l’archive?',
	// source: 6b5a1a01
	'archive.rewrite.delete.message':
		'Supprimer {what} de {archive}? Cela réécrit toute l’archive ({size}). L’archive précédente va dans la Corbeille, et l’annulation la rétablit.',
	// source: e2d0a549
	'archive.rewrite.delete.action': 'Supprimer',
	// source: 52d64331
	'archive.rewrite.rename.title': 'Renommer dans l’archive?',
	// source: 4b8a9c78
	'archive.rewrite.rename.message':
		'Renommer {name} en {newName}? Cela réécrit toute l’archive {archive} ({size}). L’archive précédente va dans la Corbeille, et l’annulation la rétablit.',
	// source: 3064d79a
	'archive.rewrite.rename.action': 'Renommer',
	// source: 50519c99
	'archive.rewrite.rename.declined': 'Le changement de nom n’a pas été confirmé.',
	// source: 117d55fc
	'archive.rewrite.limit.title': 'Modifier une très grande archive?',
	// source: 885da2e7
	'archive.rewrite.limit.message':
		'{name} dépasse les limites prévues pour les archives ({reason}). La modifier la réécrit en entier, ce qui peut être long et remplir le disque.',
	// source: 0e8aba0c
	'archive.rewrite.limit.action': 'Modifier quand même',
	// source: c6dd4fe8
	'files.copied.one': '{count} élément copié',
	// source: 6a1597b6
	'files.copied.many': '{count} d’éléments copiés',
	// source: 6a1597b6
	'files.copied.other': '{count} éléments copiés',
	// source: 3b24cd98
	'files.cut.one': '{count} élément coupé',
	// source: 908a446f
	'files.cut.many': '{count} d’éléments coupés',
	// source: 908a446f
	'files.cut.other': '{count} éléments coupés',
	// source: b4010d33
	'files.paste.nothing': 'Il n’y a rien à coller.',
	// source: c8c993de
	'destination.title.copy.one': 'Copier {count} élément vers…',
	// source: 8a42ae1a
	'destination.title.copy.many': 'Copier {count} d’éléments vers…',
	// source: 8a42ae1a
	'destination.title.copy.other': 'Copier {count} éléments vers…',
	// source: ed17e799
	'destination.title.move.one': 'Déplacer {count} élément vers…',
	// source: 71662d0e
	'destination.title.move.many': 'Déplacer {count} d’éléments vers…',
	// source: 71662d0e
	'destination.title.move.other': 'Déplacer {count} éléments vers…',
	// source: f4803569
	'destination.title.link.one': 'Créer un lien vers {count} élément dans…',
	// source: e3f29c0b
	'destination.title.link.many': 'Créer un lien vers {count} d’éléments dans…',
	// source: e3f29c0b
	'destination.title.link.other': 'Créer un lien vers {count} éléments dans…',
	// source: 5c71b8cd
	'destination.title.choose': 'Choisir un dossier',
	// source: e21f935f
	'destination.copy': 'Copier',
	// source: cb97201b
	'destination.title.extract.one': 'Extraire {count} archive vers…',
	// source: 3260b12c
	'destination.title.extract.many': 'Extraire {count} d’archives vers…',
	// source: 3260b12c
	'destination.title.extract.other': 'Extraire {count} archives vers…',
	// source: c15301c0
	'destination.extract': 'Extraire',
	// source: 6ecc3df6
	'destination.move': 'Déplacer',
	// source: a6a32dbc
	'destination.link': 'Lier',
	// source: c7f93783
	'destination.choose': 'Choisir',
	// source: 19766ed6
	'destination.cancel': 'Annuler',
	// source: 74ccd433
	'destination.path.label': 'Dossier',
	// source: fefc0d1f
	'destination.path.description': 'Saisissez un chemin ou choisissez-en un ci-dessous.',
	// source: 01e38309
	'destination.choices.label': 'Emplacements au choix',
	// source: eb5cfb73
	'destination.section.places': 'Emplacements',
	// source: d97d51d3
	'destination.section.favourites': 'Favoris',
	// source: 68d7beb6
	'destination.section.servers': 'Serveurs',
	// source: 33c2eb40
	'destination.section.tabs': 'Onglets ouverts',
	// source: 690dbe9d
	'destination.section.recent': 'Récents',
	// source: 633810e9
	'destination.newFolder': 'Nouveau dossier…',
	// source: d9335930
	'destination.newFolder.working': 'Création du dossier…',
	// source: 0f382e60
	'destination.newFolder.made': '« {name} » créé et choisi.',
	// source: 2d028378
	'destination.newFolder.failed': 'Impossible de créer le dossier : {reason}',
	// source: ec963ffc
	'destination.check.working': 'Vérification…',
	// source: 8075cfc3
	'destination.check.ok': 'Prêt : il est possible d’écrire dans {name}.',
	// source: 8d8ab28c
	'destination.check.sameFolder': 'Les éléments sont déjà dans ce dossier.',
	// source: 599a094f
	'destination.check.empty': 'Saisissez un dossier ou choisissez-en un dans les listes.',
	// source: 152e3ad6
	'destination.check.invalid': '« {input} » n’est pas un emplacement.',
	// source: cf966130
	'destination.check.unsupported':
		'Les emplacements de type {what} ne peuvent pas encore être utilisés.',
	// source: a5366598
	'destination.check.notFound': '« {name} » n’existe pas.',
	// source: 6caa19cd
	'destination.check.notFolder': '« {name} » n’est pas un dossier.',
	// source: 0763d44c
	'destination.check.readOnly': 'Il n’est pas possible d’écrire dans « {name} ».',
	// source: ec5aeec6
	'destination.check.denied': 'Vous n’avez pas la permission d’utiliser « {name} ».',
	// source: cd722701
	'destination.check.failed': 'Impossible de vérifier ce dossier.',
	// source: e84e258e
	'rename.field.label': 'Renommer {name}',
	// source: 197267ef
	'rename.hint': 'Entrée renomme, Échap annule.',
	// source: 7de38812
	'rename.done': '{from} renommé en {to}',
	// source: 03ca7ff7
	'rename.error.empty': 'Un nom ne peut pas être vide.',
	// source: c99153fb
	'rename.error.dots': 'Un nom ne peut pas être « . » ni « .. ».',
	// source: 7c1ea7e9
	'rename.error.nul': 'Un nom ne peut pas contenir de caractère nul.',
	// source: 573065af
	'rename.error.slash': 'Un nom ne peut pas contenir « / ».',
	// source: bc81a2f0
	'rename.error.backslash': 'Un nom ne peut pas contenir « \\ » sous Windows.',
	// source: f6d7980c
	'rename.error.tooLong': 'Un nom peut compter au plus {limit} caractères.',
	// source: 2f21d2b6
	'rename.error.forbidden': 'Un nom ne peut pas contenir {character} sous Windows.',
	// source: 1694c5f3
	'rename.error.trailing':
		'Un nom ne peut pas se terminer par un point ou une espace sous Windows.',
	// source: 6030ff7a
	'rename.error.reserved': 'Ce nom est réservé par Windows.',
	// source: 67d5480c
	'rename.error.exists': 'Un fichier nommé « {name} » existe déjà.',
	// source: d21ccd1e
	'rename.error.invalid': 'Ce nom n’est pas permis : {reason}',
	// source: f2086795
	'rename.error.failed': 'Impossible de renommer : {reason}',
	// source: 5af3a875
	'rename.extension.title': 'Changer l’extension?',
	// source: 3f67f7fd
	'rename.extension.change':
		'Changer l’extension de {from} à {to}? Le fichier pourrait ne plus s’ouvrir dans le programme qui gère les fichiers {from}.',
	// source: b086aaea
	'rename.extension.remove':
		'Retirer l’extension {from}? Le fichier pourrait ne plus s’ouvrir dans le programme qui gère les fichiers {from}.',
	// source: b173cc72
	'rename.extension.keep': 'Garder {from}',
	// source: 6bc87c87
	'rename.extension.use': 'Utiliser {to}',
	// source: 20d94d47
	'rename.extension.drop': 'Retirer {from}',
	// source: 3bea3abf
	'tabs.closeGuard.title': 'Fermer cet onglet?',
	// source: 2b7cce46
	'tabs.closeGuard.message':
		'Une opération écrit encore dans ce dossier. Fermer l’onglet ne l’arrête pas, mais ce panneau n’en affichera plus la progression.',
	// source: f271892d
	'tabs.closeGuard.action': 'Fermer l’onglet',
	// source: 71596e43
	'tabs.closeGuard.keep': 'Garder ouvert',
	// source: ee34df51
	'dev.live.label': 'Commandes de mise à jour en direct (développement seulement)',
	// source: c8e4c718
	'dev.live.add': 'Ajouter 5 fichiers',
	// source: ede175da
	'dev.live.addTop': 'Ajouter 5 en haut',
	// source: 879ac2bd
	'dev.live.remove': 'Retirer 5 fichiers',
	// source: c8bb1277
	'dev.live.touch': 'Toucher 5 fichiers',
	// source: 0ffb30c4
	'dev.live.auto': 'Changer en continu',
	// source: b553d47d
	'batchRename.title': 'Renommage en lot',
	// source: 959d93f2
	'batchRename.selection.one':
		'Renommer {count} élément avec une pile de règles, appliquées dans l’ordre.',
	// source: a7967a41
	'batchRename.selection.many':
		'Renommer {count} d’éléments avec une pile de règles, appliquées dans l’ordre.',
	// source: a7967a41
	'batchRename.selection.other':
		'Renommer {count} éléments avec une pile de règles, appliquées dans l’ordre.',
	// source: 4228aeb0
	'batchRename.rules.label': 'Règles',
	// source: f3e95c52
	'batchRename.rule.heading': 'Règle {number}',
	// source: 5678e7b5
	'batchRename.rule.type': 'Type de règle',
	// source: 5367cdbd
	'batchRename.rule.moveUp': 'Monter la règle {number}',
	// source: cfcf0f58
	'batchRename.rule.moveDown': 'Descendre la règle {number}',
	// source: 0ffbcc1c
	'batchRename.rule.remove': 'Retirer la règle {number}',
	// source: a27cff51
	'batchRename.rule.add': 'Ajouter une règle',
	// source: dbb1d5c9
	'batchRename.rule.addType': 'Type de règle à ajouter',
	// source: becf2363
	'batchRename.rule.error': 'Règle {number} : {reason}',
	// source: 9898ad03
	'batchRename.kind.findReplace': 'Rechercher et remplacer',
	// source: bd82cf16
	'batchRename.kind.counter': 'Numérotation',
	// source: f97fe580
	'batchRename.kind.case': 'Changer la casse',
	// source: acd43562
	'batchRename.kind.dateToken': 'Insérer la date',
	// source: 62a09dd6
	'batchRename.kind.insert': 'Insérer du texte',
	// source: 5f462179
	'batchRename.kind.remove': 'Retirer des caractères',
	// source: be4fe0b6
	'batchRename.kind.trimWhitespace': 'Supprimer les espaces',
	// source: be5ca387
	'batchRename.kind.changeExtension': 'Changer l’extension',
	// source: aab0d28e
	'batchRename.scope.label': 'Appliquer à',
	// source: aef3c304
	'batchRename.scope.name': 'Nom complet',
	// source: 0cd4b938
	'batchRename.scope.stem': 'Nom sans l’extension',
	// source: 395aeb95
	'batchRename.scope.extension': 'Extension',
	// source: 822b2ae4
	'batchRename.find': 'Rechercher',
	// source: 8382d317
	'batchRename.replace': 'Remplacer par',
	// source: 9f896c35
	'batchRename.regex': 'Expression régulière',
	// source: bfc2893d
	'batchRename.regex.hint': 'Utilisez $1 ou ${1} dans le remplacement pour désigner un groupe.',
	// source: 61988011
	'batchRename.caseSensitive': 'Respecter la casse',
	// source: 09e8b2ff
	'batchRename.all': 'Remplacer chaque occurrence',
	// source: 334c284f
	'batchRename.counter.start': 'Commencer à',
	// source: 8e6a6cca
	'batchRename.counter.step': 'Pas',
	// source: 9cd500d3
	'batchRename.counter.width': 'Chiffres',
	// source: 6d031af1
	'batchRename.position.label': 'Position',
	// source: fd8a7586
	'batchRename.position.prefix': 'Avant le nom',
	// source: 842707a7
	'batchRename.position.suffix': 'Après le nom',
	// source: 234e50b7
	'batchRename.position.replaceStem': 'À la place du nom',
	// source: be237eda
	'batchRename.separator': 'Séparateur',
	// source: aecc3f30
	'batchRename.case.mode': 'Changer en',
	// source: cc4c2e40
	'batchRename.case.upper': 'MAJUSCULES',
	// source: 99f71806
	'batchRename.case.lower': 'minuscules',
	// source: c897dd86
	'batchRename.case.title': 'Majuscule À Chaque Mot',
	// source: 2f29e300
	'batchRename.case.sentence': 'Majuscule initiale',
	// source: 61df12fa
	'batchRename.date.source': 'Date de',
	// source: e8ce5dca
	'batchRename.date.modified': 'Modification',
	// source: d70b9e24
	'batchRename.date.created': 'Création',
	// source: 2b065c7c
	'batchRename.date.today': 'Aujourd’hui',
	// source: 2f343666
	'batchRename.date.format': 'Format',
	// source: f03eeac1
	'batchRename.date.hint': 'Année %Y, mois %m, jour %d, heure %H, minute %M, seconde %S.',
	// source: 71988c4d
	'batchRename.insert.text': 'Texte',
	// source: 9fc231cd
	'batchRename.insert.at': 'Insérer à',
	// source: 3eadbac6
	'batchRename.insert.start': 'Début du nom',
	// source: a46b95a3
	'batchRename.insert.end': 'Fin du nom',
	// source: 15f8e715
	'batchRename.insert.index': 'Avant le caractère',
	// source: 85355a8a
	'batchRename.insert.position': 'Position du caractère',
	// source: 739354b6
	'batchRename.remove.from': 'À partir du caractère',
	// source: c28c758e
	'batchRename.remove.to': 'Jusqu’au caractère',
	// source: ab23e032
	'batchRename.remove.hint': 'Numérotation à partir de 1; les deux extrémités sont retirées.',
	// source: 5d0bbdef
	'batchRename.trim.note': 'Supprime les espaces aux deux extrémités du nom.',
	// source: 0a564061
	'batchRename.extension.to': 'Nouvelle extension',
	// source: f8e5bfb3
	'batchRename.extension.hint': 'Laissez vide pour la retirer. Les dossiers sont ignorés.',
	// source: c631a00a
	'batchRename.preview.label': 'Nouveaux noms',
	// source: 9bb72500
	'batchRename.preview.before': 'Avant',
	// source: 7b68fe55
	'batchRename.preview.after': 'Après',
	// source: d8da2c49
	'batchRename.preview.note': 'Remarque',
	// source: d3b7ec6b
	'batchRename.preview.checking': 'Vérification des nouveaux noms…',
	// source: 78996325
	'batchRename.preview.failed': 'Les nouveaux noms n’ont pas pu être calculés : {reason}',
	// source: 37181158
	'batchRename.preview.capped': 'Éléments affichés : {shown} sur {total}',
	// source: 41f9d57c
	'batchRename.row.unchanged': 'Aucun changement',
	// source: d2320975
	'batchRename.row.extension': 'L’extension change',
	// source: a1c5ae7b
	'batchRename.problem.label': 'Problème',
	// source: ca6b8580
	'batchRename.problem.duplicate': 'Même nom que « {other} »',
	// source: 457adf2e
	'batchRename.problem.exists': 'Un fichier ou un dossier portant ce nom est déjà là',
	// source: a4ba81b6
	'batchRename.problem.invalid': '{reason}',
	// source: bb4bfc63
	'batchRename.problem.nested':
		'Dans « {other} », qui est aussi renommé; sélectionnez l’un ou l’autre',
	// source: 9a1fb385
	'batchRename.summary.problems.one': '{count} problème',
	// source: de83f43e
	'batchRename.summary.problems.many': '{count} de problèmes',
	// source: de83f43e
	'batchRename.summary.problems.other': '{count} problèmes',
	// source: c490e5d7
	'batchRename.summary.changes.one': '{count} élément sera renommé',
	// source: 9c6f1485
	'batchRename.summary.changes.many': '{count} d’éléments seront renommés',
	// source: 9c6f1485
	'batchRename.summary.changes.other': '{count} éléments seront renommés',
	// source: 4cc810af
	'batchRename.summary.none': 'Aucun nom ne change',
	// source: ab916c8b
	'batchRename.extensionNote':
		'Une règle change une extension de fichier. Les extensions sont ignorées, sauf si une règle les vise.',
	// source: 31e392d1
	'batchRename.apply': 'Appliquer',
	// source: 19766ed6
	'batchRename.cancel': 'Annuler',
	// source: 40aef98d
	'batchRename.applying': 'Renommage…',
	// source: 7382b532
	'batchRename.applied.one': 'Renommage de {count} élément',
	// source: 3d43595d
	'batchRename.applied.many': 'Renommage de {count} d’éléments',
	// source: 3d43595d
	'batchRename.applied.other': 'Renommage de {count} éléments',
	// source: 0cbc60f6
	'batchRename.applyFailed': 'Le renommage n’a pas pu démarrer : {reason}',
	// source: a76e13b9
	'chrome.restore': 'Restaurer',
	// source: 1c9fcada
	'chrome.maximise': 'Agrandir',
	// source: 20f082e9
	'chrome.minimise': 'Réduire',
	// source: 9a8e3f39
	'ops.ring.idle': 'Opérations, aucune en cours',
	// source: f8cd67e9
	'ops.ring.active.one': 'Opérations, {count} en cours',
	// source: f8cd67e9
	'ops.ring.active.many': 'Opérations, {count} en cours',
	// source: f8cd67e9
	'ops.ring.active.other': 'Opérations, {count} en cours',
	// source: f332cf80
	'ops.ring.done': 'Opérations, toutes terminées',
	// source: 8a5bee86
	'ops.ring.percent': '{percent} % terminé',
	// source: 7d49e284
	'ops.ring.waiting': 'en attente de vous',
	// source: c5fa723b
	'ops.ring.failed': 'une tâche a échoué',
	// source: d36be649
	'ops.ring.queued': 'en file d’attente',
	// source: 358cc201
	'ops.popover.label': 'Opérations',
	// source: 358cc201
	'ops.panel.label': 'Opérations',
	// source: 2f17a0f8
	'ops.list.label': 'Tâches',
	// source: b1ef1bfe
	'ops.list.empty': 'Rien n’est en cours.',
	// source: 00d0cb7f
	'ops.list.hint': 'Alt+Haut et Alt+Bas déplacent une tâche en file d’attente',
	// source: 07d08fca
	'ops.interrupted.heading': 'Transferts interrompus',
	// source: 086924f6
	'ops.interrupted.detail.one':
		'Arrêté quand sa connexion a été perdue; {count} fichier a été envoyé en partie',
	// source: 33c7320f
	'ops.interrupted.detail.many':
		'Arrêté quand sa connexion a été perdue; {count} de fichiers ont été envoyés en partie',
	// source: 33c7320f
	'ops.interrupted.detail.other':
		'Arrêté quand sa connexion a été perdue; {count} fichiers ont été envoyés en partie',
	// source: eb1a70e3
	'ops.interrupted.discard': 'Abandonner',
	// source: b163bbda
	'ops.interrupted.discardEllipsis': 'Abandonner…',
	// source: c78ba562
	'ops.interrupted.discard.title': 'Abandonner « {label} »?',
	// source: af085c53
	'ops.interrupted.discard.message.one':
		'Ce qui a été envoyé de ce fichier est supprimé du serveur, et le transfert ne pourra plus reprendre.',
	// source: 9d989d98
	'ops.interrupted.discard.message.many':
		'Ce qui a été envoyé de ces {count} de fichiers est supprimé du serveur, et le transfert ne pourra plus reprendre.',
	// source: 9d989d98
	'ops.interrupted.discard.message.other':
		'Ce qui a été envoyé de ces {count} fichiers est supprimé du serveur, et le transfert ne pourra plus reprendre.',
	// source: 4054474b
	'ops.interrupted.resumed': 'Repris : {label}',
	// source: eb85027b
	'ops.interrupted.discarded': 'Abandonné : {label}',
	// source: 4b458481
	'ops.interrupted.discardFailed': 'Le transfert n’a pas pu être abandonné : {reason}',
	// source: 2a16b4f8
	'ops.clearFinished': 'Effacer les terminées',
	// source: 7bdde36a
	'ops.popOut': 'Détacher',
	// source: 7fb4a204
	'ops.popOut.failed': 'Impossible d’ouvrir la fenêtre des opérations.',
	// source: 911bed8c
	'ops.reordered': '{title} déplacée à la position {position}',
	// source: fc141f8a
	'ops.speedLimit.label': 'Limite de vitesse',
	// source: f7fcff0d
	'ops.speedLimit.none': 'Aucune limite',
	// source: d19e98ce
	'ops.speedLimit.value': '{speed} Mo/s',
	// source: 0dfbd261
	'ops.speedLimit.announce': 'Limite de vitesse de {title} : {limit}',
	// source: d60dbba0
	'ops.priority.label': 'Priorité',
	// source: c4ebc6d4
	'ops.priority.high': 'Élevée',
	// source: a7248eeb
	'ops.priority.normal': 'Normale',
	// source: f793de20
	'ops.priority.low': 'Faible',
	// source: ff227ba0
	'ops.priority.announce': 'Priorité de {title} : {priority}',
	// source: 10e27907
	'ops.schedule.button': 'Schedule…',
	// source: d31b451c
	'ops.schedule.title': 'Planifier : {title}',
	// source: 28073ebe
	'ops.schedule.mode.label': 'Quand cette tâche peut démarrer',
	// source: 334c284f
	'ops.schedule.mode.startAt': 'Démarrer à',
	// source: cee86e23
	'ops.schedule.mode.window': 'Seulement entre',
	// source: babe9dda
	'ops.schedule.startAt.label': 'Heure de début',
	// source: 21819769
	'ops.schedule.from': 'De',
	// source: 7caf856e
	'ops.schedule.until': 'À',
	// source: 22185675
	'ops.schedule.apply': 'Enregistrer',
	// source: 09913977
	'ops.schedule.runNow': 'Exécuter maintenant',
	// source: 19766ed6
	'ops.schedule.cancel': 'Annuler',
	// source: 7fd7bbf4
	'ops.schedule.error.past': 'Choisissez une heure dans le futur.',
	// source: 8fec943e
	'ops.schedule.error.same': 'Choisissez deux heures différentes.',
	// source: 22cac4f9
	'ops.schedule.error.incomplete': 'Remplissez chaque heure.',
	// source: 098cb7e7
	'ops.schedule.startsAt': 'Planifiée pour le {time}',
	// source: 7a0e48ce
	'ops.schedule.window': 'Planifiée chaque jour entre {from} et {to}',
	// source: 9dcea3c1
	'ops.schedule.announce': 'Planification de {title} : {when}',
	// source: ced5ff5e
	'ops.schedule.cleared': '{title} démarrera dès que possible',
	// source: 0b6258b5
	'ops.pauseAll': 'Tout suspendre',
	// source: 43e673c5
	'ops.resumeAll': 'Tout reprendre',
	// source: a6a9e28d
	'ops.pauseAll.announce': 'Toutes les tâches sont suspendues',
	// source: fcef9dab
	'ops.resumeAll.announce': 'Les tâches ont repris',
	// source: 24ac92fc
	'ops.pausedAll.note': 'Suspendu : rien ne démarre avant la reprise.',
	// source: c66feb5e
	'ops.moveUp': 'Monter',
	// source: 40bb50da
	'ops.moveDown': 'Descendre',
	// source: 0bcc686f
	'ops.route': '{from} → {to}',
	// source: 825b82e6
	'ops.server.to': 'Vers {server}',
	// source: bd07c630
	'ops.server.from': 'De {server}',
	// source: 3de93538
	'ops.server.on': 'Sur {server}',
	// source: 6eeb9b3a
	'ops.server.between': 'De {from} vers {to}',
	// source: 26cb7e63
	'ops.dropped.modifiedTimes':
		'Dates de modification non conservées : la destination ne peut pas les garder',
	// source: 38476048
	'ops.dropped.permissions': 'Permissions non conservées : la destination n’en a pas',
	// source: 362a0767
	'ops.dropped.both':
		'Dates de modification et permissions non conservées : la destination ne peut pas les garder',
	// source: 208a19d5
	'ops.sources.one': '{count} élément',
	// source: f65216b3
	'ops.sources.many': '{count} d’éléments',
	// source: f65216b3
	'ops.sources.other': '{count} éléments',
	// source: 4fa892b2
	'ops.title.copy.one': 'Copie de {count} élément',
	// source: 2feba608
	'ops.title.copy.many': 'Copie de {count} d’éléments',
	// source: 2feba608
	'ops.title.copy.other': 'Copie de {count} éléments',
	// source: a26e7f39
	'ops.title.copy.named': 'Copie de {name}',
	// source: 05bcad27
	'ops.title.move.one': 'Déplacement de {count} élément',
	// source: 481254e8
	'ops.title.move.many': 'Déplacement de {count} d’éléments',
	// source: 481254e8
	'ops.title.move.other': 'Déplacement de {count} éléments',
	// source: c9ff6388
	'ops.title.move.named': 'Déplacement de {name}',
	// source: bd5ccbfc
	'ops.title.link.one': 'Création d’un lien vers {count} élément',
	// source: a778f6b7
	'ops.title.link.many': 'Création d’un lien vers {count} d’éléments',
	// source: a778f6b7
	'ops.title.link.other': 'Création d’un lien vers {count} éléments',
	// source: 8b3ac233
	'ops.title.link.named': 'Création d’un lien vers {name}',
	// source: a2fc5d05
	'ops.title.trash.one': 'Mise à la Corbeille de {count} élément',
	// source: 9907c6b4
	'ops.title.trash.many': 'Mise à la Corbeille de {count} d’éléments',
	// source: 9907c6b4
	'ops.title.trash.other': 'Mise à la Corbeille de {count} éléments',
	// source: 919f683b
	'ops.title.trash.named': 'Mise à la Corbeille de {name}',
	// source: 285c2a07
	'ops.title.delete.one': 'Suppression de {count} élément',
	// source: 61f697f0
	'ops.title.delete.many': 'Suppression de {count} d’éléments',
	// source: 61f697f0
	'ops.title.delete.other': 'Suppression de {count} éléments',
	// source: b8e18867
	'ops.title.delete.named': 'Suppression de {name}',
	// source: 5d6962fe
	'ops.title.duplicate.one': 'Duplication de {count} élément',
	// source: 1cbf0c96
	'ops.title.duplicate.many': 'Duplication de {count} d’éléments',
	// source: 1cbf0c96
	'ops.title.duplicate.other': 'Duplication de {count} éléments',
	// source: 1157d9e3
	'ops.title.duplicate.named': 'Duplication de {name}',
	// source: 154f1572
	'ops.title.restore.one': 'Restauration de {count} élément',
	// source: 7ef6e728
	'ops.title.restore.many': 'Restauration de {count} d’éléments',
	// source: 7ef6e728
	'ops.title.restore.other': 'Restauration de {count} éléments',
	// source: fd311a31
	'ops.title.restore.named': 'Restauration de {name}',
	// source: c6dd4fe8
	'ops.done.copy.one': '{count} élément copié',
	// source: 6a1597b6
	'ops.done.copy.many': '{count} d’éléments copiés',
	// source: 6a1597b6
	'ops.done.copy.other': '{count} éléments copiés',
	// source: d1f79e6c
	'ops.done.copy.named': '{name} copié',
	// source: 01fa61e9
	'ops.done.move.one': '{count} élément déplacé',
	// source: 9c8c184c
	'ops.done.move.many': '{count} d’éléments déplacés',
	// source: 9c8c184c
	'ops.done.move.other': '{count} éléments déplacés',
	// source: 89d749e5
	'ops.done.move.named': '{name} déplacé',
	// source: 0d2fcb47
	'ops.done.link.one': 'Lien créé vers {count} élément',
	// source: baf9a919
	'ops.done.link.many': 'Lien créé vers {count} d’éléments',
	// source: baf9a919
	'ops.done.link.other': 'Lien créé vers {count} éléments',
	// source: 7832c175
	'ops.done.link.named': 'Lien créé vers {name}',
	// source: 9ab9f18f
	'ops.done.trash.one': '{count} élément mis à la Corbeille',
	// source: 83562d46
	'ops.done.trash.many': '{count} d’éléments mis à la Corbeille',
	// source: 83562d46
	'ops.done.trash.other': '{count} éléments mis à la Corbeille',
	// source: 9ce945b8
	'ops.done.trash.named': '{name} mis à la Corbeille',
	// source: e6380f89
	'ops.done.duplicate.one': '{count} élément dupliqué',
	// source: 174d44a7
	'ops.done.duplicate.many': '{count} d’éléments dupliqués',
	// source: 174d44a7
	'ops.done.duplicate.other': '{count} éléments dupliqués',
	// source: 5a314b29
	'ops.done.duplicate.named': '{name} dupliqué',
	// source: 144c58d0
	'ops.done.restore.one': '{count} élément restauré',
	// source: 59cb1792
	'ops.done.restore.many': '{count} d’éléments restaurés',
	// source: 59cb1792
	'ops.done.restore.other': '{count} éléments restaurés',
	// source: b7ceb1cf
	'ops.done.restore.named': '{name} restauré',
	// source: d1b1a8da
	'ops.title.extract.one': 'Extraction de {count} archive',
	// source: 9322402d
	'ops.title.extract.many': 'Extraction de {count} d’archives',
	// source: 9322402d
	'ops.title.extract.other': 'Extraction de {count} archives',
	// source: 5ebe3d1b
	'ops.title.extract.named': 'Extraction de {name}',
	// source: 533f2790
	'ops.done.extract.one': '{count} archive extraite',
	// source: 5f005fd1
	'ops.done.extract.many': '{count} d’archives extraites',
	// source: 5f005fd1
	'ops.done.extract.other': '{count} archives extraites',
	// source: 18b944ce
	'ops.done.extract.named': '{name} extraite',
	// source: 3a469567
	'ops.title.compress.one': 'Compression de {count} élément',
	// source: d86ab739
	'ops.title.compress.many': 'Compression de {count} d’éléments',
	// source: d86ab739
	'ops.title.compress.other': 'Compression de {count} éléments',
	// source: 7718f8d1
	'ops.title.compress.named': 'Compression de {name}',
	// source: 362021f1
	'ops.done.compress.one': '{count} élément compressé',
	// source: 2e049436
	'ops.done.compress.many': '{count} d’éléments compressés',
	// source: 2e049436
	'ops.done.compress.other': '{count} éléments compressés',
	// source: 8d6d5320
	'ops.done.compress.named': '{name} compressé',
	// source: b3f812b1
	'ops.done.delete.one': '{count} élément supprimé',
	// source: 3566861d
	'ops.done.delete.many': '{count} d’éléments supprimés',
	// source: 3566861d
	'ops.done.delete.other': '{count} éléments supprimés',
	// source: abaa7e46
	'ops.done.delete.named': '{name} supprimé',
	// source: d793e100
	'ops.done.generic': 'Terminé : {title}',
	// source: 5d1fa38b
	'ops.state.planning': 'Préparation…',
	// source: 28cc95cd
	'ops.state.queued': 'En attente',
	// source: f4ccae29
	'ops.state.running': 'En cours',
	// source: e159b061
	'ops.state.paused': 'En pause',
	// source: 91b104db
	'ops.state.cancelling': 'Annulation…',
	// source: d353a99e
	'ops.state.cancelled': 'Annulé',
	// source: 11a6767d
	'ops.state.done': 'Terminé',
	// source: 7ba32522
	'ops.state.failed': 'Échec : {reason}',
	// source: 4c1046f0
	'ops.state.offline': 'Connexion perdue, nouvel essai automatique (essai {attempt}) : {reason}',
	// source: 202a0714
	'ops.state.waiting.conflicts.one': 'En attente de vous : {count} nom est déjà pris',
	// source: 558b9905
	'ops.state.waiting.conflicts.many': 'En attente de vous : {count} de noms sont déjà pris',
	// source: 558b9905
	'ops.state.waiting.conflicts.other': 'En attente de vous : {count} noms sont déjà pris',
	// source: 6d1d428a
	'ops.state.waiting.error': 'En attente de vous : {reason}',
	// source: e163fd7e
	'ops.progress.items': '{done} sur {total} éléments',
	// source: e7a467ec
	'ops.progress.bytes': '{done} sur {total}',
	// source: 5feb4089
	'ops.progress.speed': '{speed}/s',
	// source: 230d1740
	'ops.progress.eta': '{time} restantes',
	// source: 8e6df4a0
	'ops.duration.seconds': '{n} s',
	// source: 913185ba
	'ops.duration.minutes': '{n} min',
	// source: 99422830
	'ops.duration.hours': '{h} h {m} min',
	// source: 084189ee
	'ops.skipped.one': '{count} ignoré',
	// source: 084189ee
	'ops.skipped.many': '{count} ignorés',
	// source: 084189ee
	'ops.skipped.other': '{count} ignorés',
	// source: 858e4ba7
	'ops.action.pause': 'Mettre en pause',
	// source: d640c742
	'ops.action.resume': 'Reprendre',
	// source: 19766ed6
	'ops.action.cancel': 'Annuler',
	// source: 942087cc
	'ops.action.retry': 'Réessayer',
	// source: 48845bff
	'ops.action.dismiss': 'Fermer',
	// source: a55fea56
	'ops.action.resolve': 'Résoudre…',
	// source: da478765
	'ops.action.showInFolder': 'Afficher dans le dossier',
	// source: edc480cc
	'ops.action.for': '{action} : {title}',
	// source: ed0b7469
	'ops.resolve.unavailable': 'Il n’est pas encore possible de répondre d’ici.',
	// source: 0bdf197b
	'ops.resolve.notWaiting': 'Cette tâche n’attend plus de réponse.',
	// source: 43d68f56
	'ops.resolve.failed': 'Impossible d’envoyer la réponse : {reason}',
	// source: fd9849d8
	'ops.conflict.title.one': '{count} élément existe déjà dans {destination}',
	// source: 4a80d858
	'ops.conflict.title.many': '{count} d’éléments existent déjà dans {destination}',
	// source: 4a80d858
	'ops.conflict.title.other': '{count} éléments existent déjà dans {destination}',
	// source: 7a4860f3
	'ops.conflict.title.restore.one': '{count} élément existe déjà dans son dossier d’origine',
	// source: 29392dc9
	'ops.conflict.title.restore.many': '{count} d’éléments existent déjà dans leur dossier d’origine',
	// source: 29392dc9
	'ops.conflict.title.restore.other': '{count} éléments existent déjà dans leur dossier d’origine',
	// source: 2dbdcae8
	'ops.conflict.destination.unknown': 'la destination',
	// source: afcb88ec
	'ops.conflict.description':
		'Choisissez ce qui arrive à chacun. Rien n’est écrasé avant que vous continuiez.',
	// source: cb18fb43
	'ops.conflict.description.restore':
		'Choisissez ce qui arrive à chacun. Rien n’est modifié avant que vous continuiez.',
	// source: 630870b0
	'ops.conflict.table.label': 'Éléments qui existent déjà',
	// source: dcd1d522
	'ops.conflict.col.name': 'Nom',
	// source: b3245801
	'ops.conflict.col.existing': 'Déjà présent',
	// source: e301820a
	'ops.conflict.col.incoming': 'Entrant',
	// source: e0781672
	'ops.conflict.col.incoming.restore': 'Dans la Corbeille',
	// source: f47f1edc
	'ops.conflict.col.choice': 'Action',
	// source: 50009ce1
	'ops.conflict.kind.file': 'Fichier',
	// source: 74ccd433
	'ops.conflict.kind.folder': 'Dossier',
	// source: 87339554
	'ops.conflict.sizeUnknown': 'Taille inconnue',
	// source: bc11be8c
	'ops.conflict.modifiedUnknown': 'Date inconnue',
	// source: 7e8229c4
	'ops.conflict.hint.newer': 'Plus récent que l’existant',
	// source: 856968da
	'ops.conflict.hint.older': 'Plus ancien que l’existant',
	// source: c8929748
	'ops.conflict.hint.same': 'Même date que l’existant',
	// source: f81c7cc8
	'ops.conflict.batch': 'Pas encore là : un autre élément de cette tâche porte le même nom',
	// source: 232c550c
	'ops.conflict.mismatch.fileOverFolder':
		'Un fichier porte le nom d’un dossier existant; il ne peut donc qu’être ignoré ou conservé à côté.',
	// source: 2451b298
	'ops.conflict.mismatch.folderOverFile':
		'Un dossier porte le nom d’un fichier existant; il ne peut donc qu’être ignoré ou conservé à côté.',
	// source: 95e15439
	'ops.conflict.choice.replace': 'Remplacer',
	// source: 28d03596
	'ops.conflict.choice.skip': 'Ignorer',
	// source: 93672212
	'ops.conflict.choice.keepBoth': 'Garder les deux',
	// source: 62c63bb6
	'ops.conflict.choice.mergeFolders': 'Fusionner les dossiers',
	// source: de6bf8d1
	'ops.conflict.choice.replaceIfNewer': 'Remplacer si plus récent',
	// source: 7ca41615
	'ops.conflict.choice.placeholder': 'Choisir…',
	// source: 110365bd
	'ops.conflict.choice.bulk': 'Comme « Appliquer à tous les restants » : {choice}',
	// source: 9febbfc2
	'ops.conflict.choice.for': 'Choix pour {name}',
	// source: 0336abb0
	'ops.conflict.note.replace.file': 'Remplace le fichier existant.',
	// source: 23de2473
	'ops.conflict.note.replace.folder':
		'Remplace tout le dossier : tout ce qui s’y trouve et n’est pas dans le dossier entrant est perdu.',
	// source: cd3dccaa
	'ops.conflict.note.skip': 'Laissé de côté; l’existant reste tel quel.',
	// source: aaa68418
	'ops.conflict.note.keepBoth': 'Conservé à côté de l’existant sous forme de copie numérotée.',
	// source: bd8e6726
	'ops.conflict.note.keepBoth.restore': 'Restauré sous un nom libre.',
	// source: 16725a48
	'ops.conflict.note.mergeFolders':
		'Intégré au dossier existant. Les conflits à l’intérieur sont réglés par le choix appliqué à tous, et vous sont demandés quand celui-ci ne peut pas les régler.',
	// source: ab616b03
	'ops.conflict.note.replaceIfNewer':
		'Remplace le fichier existant seulement si l’entrant est nettement plus récent.',
	// source: 1de98c92
	'ops.conflict.bulk.label': 'Appliquer à tous les restants',
	// source: 7ca41615
	'ops.conflict.bulk.placeholder': 'Choisir…',
	// source: c9df2d68
	'ops.conflict.bulk.covers':
		'Couvre {covered} des {open} sans réponse. Les autres ne peuvent pas l’utiliser et ont besoin de leur propre choix.',
	// source: 909d640c
	'ops.conflict.bulk.coversAll': 'Couvre les {open} sans réponse.',
	// source: 67ff21fa
	'ops.conflict.later.label': 'Appliquer à tous les conflits semblables à celui-ci',
	// source: 9d4572e9
	'ops.conflict.later.hint':
		'Règle aussi ceux qui surviendront plus tard dans cette tâche, par exemple dans des dossiers fusionnés.',
	// source: 81c8b6dd
	'ops.conflict.progress': '{answered} sur {total} traités',
	// source: 1a261270
	'ops.conflict.more.one': 'et {count} de plus',
	// source: 1a261270
	'ops.conflict.more.many': 'et {count} de plus',
	// source: 1a261270
	'ops.conflict.more.other': 'et {count} de plus',
	// source: 2c3d9112
	'ops.conflict.more.hint':
		'« Appliquer à tous les restants » couvre ceux qui ne sont pas affichés; vous pouvez aussi tous les afficher pour répondre un à un.',
	// source: 2150d8df
	'ops.conflict.showAll': 'Tout afficher',
	// source: 31fbef16
	'ops.conflict.continue': 'Continuer',
	// source: 5dcca55b
	'ops.conflict.cancel': 'Annuler l’opération',
	// source: 892c45a2
	'ops.conflict.later': 'Décider plus tard',
	// source: 2c0a76c0
	'ops.conflict.cancelConfirm.title': 'Annuler l’opération?',
	// source: f312435d
	'ops.conflict.cancelConfirm.message': 'La tâche s’arrête et vos réponses sont abandonnées.',
	// source: 5dcca55b
	'ops.conflict.cancelConfirm.confirm': 'Annuler l’opération',
	// source: 5e45d70f
	'ops.conflict.cancelConfirm.keep': 'Continuer de décider',
	// source: d00e200a
	'ops.conflict.announce': 'On continue : {summary}',
	// source: 7bb54d82
	'ops.conflict.summary.replace': 'Remplacer {count}',
	// source: a59a63b1
	'ops.conflict.summary.skip': 'Ignorer {count}',
	// source: 4cb85911
	'ops.conflict.summary.keepBoth': 'Garder les deux pour {count}',
	// source: 2a2e00a5
	'ops.conflict.summary.mergeFolders': 'Fusionner {count}',
	// source: 290993b2
	'ops.conflict.summary.replaceIfNewer': 'Remplacer si plus récent pour {count}',
	// source: 67771a23
	'ops.conflict.sizeHint.larger': 'Plus gros que l’existant',
	// source: c037bc63
	'ops.conflict.sizeHint.smaller': 'Plus petit que l’existant',
	// source: 279ba4f5
	'ops.conflict.sizeHint.same': 'Même taille que l’existant',
	// source: 641ca950
	'ops.conflict.preview.loading': 'Comparaison des fichiers…',
	// source: 53417fc3
	'ops.conflict.preview.compare': 'Comparer les fichiers',
	// source: 5c5f45e0
	'ops.conflict.preview.compareNamed': 'Comparer les fichiers nommés {name}',
	// source: 9d5acd90
	'ops.conflict.preview.show': 'Afficher les différences',
	// source: 780f7ac4
	'ops.conflict.preview.hide': 'Masquer les différences',
	// source: a0248f80
	'ops.conflict.preview.showNamed': 'Afficher les différences dans {name}',
	// source: dd6bd6b7
	'ops.conflict.preview.hideNamed': 'Masquer les différences dans {name}',
	// source: d82927a9
	'ops.conflict.preview.identical':
		'Contenu identique. Ignorer est probablement ce que vous voulez.',
	// source: 0917de46
	'ops.conflict.preview.same': 'Le texte est le même; il n’y a donc rien à afficher.',
	// source: 470c6f60
	'ops.conflict.preview.binary':
		'Le contenu diffère. Ce ne sont pas des fichiers texte; aucune ligne n’est donc affichée.',
	// source: 619eb366
	'ops.conflict.preview.tooLarge': 'Trop volumineux pour être comparé ici.',
	// source: 8f642091
	'ops.conflict.preview.added.one': '{count} ligne ajoutée',
	// source: c6c96bfa
	'ops.conflict.preview.added.many': '{count} de lignes ajoutées',
	// source: c6c96bfa
	'ops.conflict.preview.added.other': '{count} lignes ajoutées',
	// source: 8ca2f2cd
	'ops.conflict.preview.removed.one': '{count} ligne retirée',
	// source: 0948a8ea
	'ops.conflict.preview.removed.many': '{count} de lignes retirées',
	// source: 0948a8ea
	'ops.conflict.preview.removed.other': '{count} lignes retirées',
	// source: c2287541
	'ops.conflict.preview.lossy':
		'Certains octets ne sont pas du texte valide et sont affichés sous la forme �.',
	// source: 5b1a2cbe
	'ops.conflict.preview.approximate':
		'La comparaison s’est arrêtée plus tôt; les changements listés peuvent donc être plus nombreux que nécessaire.',
	// source: e9a25d2d
	'ops.conflict.diff.label': 'Différences dans {name}',
	// source: 58f01269
	'ops.conflict.diff.added': 'Ajoutée, ligne {line} : ',
	// source: 88516b3c
	'ops.conflict.diff.removed': 'Retirée, ligne {line} : ',
	// source: 91fdfd9b
	'ops.conflict.diff.context': 'Inchangée, ligne {line} : ',
	// source: e8ac4089
	'ops.conflict.diff.gap.one': '{count} ligne inchangée',
	// source: 7febc71d
	'ops.conflict.diff.gap.many': '{count} de lignes inchangées',
	// source: 7febc71d
	'ops.conflict.diff.gap.other': '{count} lignes inchangées',
	// source: 9967f0f4
	'ops.conflict.diff.more.one': '{count} autre ligne',
	// source: fc34f62c
	'ops.conflict.diff.more.many': '{count} d’autres lignes',
	// source: fc34f62c
	'ops.conflict.diff.more.other': '{count} autres lignes',
	// source: a26fabfd
	'ops.problem.title': 'Un élément n’a pas pu être traité',
	// source: f53d00fd
	'ops.problem.title.parent': 'Le dossier d’origine n’existe plus',
	// source: 8b165d04
	'ops.problem.item': 'Élément : {item}',
	// source: 74b185f5
	'ops.problem.job': 'Tâche : {title}',
	// source: 0f63d5cb
	'ops.problem.partial.resume': 'Réessayer reprend {name} là où il s’est arrêté.',
	// source: 1bc9da44
	'ops.problem.partial.resumeFrom':
		'Réessayer reprend {name} là où il s’est arrêté ({size} déjà envoyés).',
	// source: 8436ebbb
	'ops.problem.partial.restart':
		'Réessayer recommence {name} : ce serveur ne peut pas reprendre un fichier en cours.',
	// source: b25ec662
	'ops.problem.message.notFound':
		'{location} est introuvable. Il a peut-être été déplacé ou supprimé.',
	// source: 10b7edcf
	'ops.problem.message.permissionDenied':
		'Permission refusée pour {location}. Vérifiez ses permissions, et celles du dossier qui le contient ou de destination.',
	// source: b801ce54
	'ops.problem.message.notEnoughSpace':
		'Il n’y a pas assez d’espace libre : {needed} requis, {free} libres.',
	// source: 9fce9ab2
	'ops.problem.message.notEnoughSpace.unknown':
		'Il n’y a pas assez d’espace libre à la destination.',
	// source: 6d33b655
	'ops.problem.message.invalidName': '« {name} » ne peut pas servir de nom ici : {reason}',
	// source: 4e5abb7d
	'ops.problem.message.nameInUse': '{location} est déjà pris.',
	// source: 8d8ab28c
	'ops.problem.message.sameFolder': 'Les éléments sont déjà dans ce dossier.',
	// source: be2202ec
	'ops.problem.message.intoItself': 'Un dossier ne peut pas être placé dans lui-même.',
	// source: de822e69
	'ops.problem.message.protected': '{location} est protégé et n’est jamais modifié.',
	// source: 2a6e7fba
	'ops.problem.message.trashUnavailable': 'La Corbeille n’est pas disponible : {reason}',
	// source: 5ff173b9
	'ops.problem.message.originMissingParent':
		'{folder} n’existe plus. Le recréer et y restaurer l’élément?',
	// source: abb4a62b
	'ops.problem.message.cancelled': 'L’opération a été annulée.',
	// source: 4672a8e6
	'ops.problem.message.unsupported': 'Cela n’est pas encore pris en charge : {what}',
	// source: 85d55d59
	'ops.problem.message.changedSince':
		'{location} a changé après la planification de la tâche; il a donc été laissé tel quel.',
	// source: 20d43857
	'ops.problem.message.verifyFailed':
		'{location} a été copié, mais sa relecture ne correspondait pas; la copie a donc été supprimée.',
	// source: 1c60bac9
	'ops.problem.message.cannotReplace':
		'{location} ne peut pas être remplacé par une entrée d’un autre genre. Garder les deux ou l’ignorer fonctionnera.',
	// source: bfaae3ae
	'ops.problem.message.undoStale': 'Cette action ne peut pas être annulée : {reason}',
	// source: a4ba81b6
	'ops.problem.message.undoUnavailable': '{reason}',
	// source: d72bcf52
	'ops.problem.message.archived':
		'{location} est archivé et doit être restauré avant de pouvoir être lu. Restaurez-le avec les outils du service, puis réessayez; Waypoint ne lance jamais de restauration.',
	// source: dcbc5e23
	'ops.problem.message.clockSkew':
		'Le service a refusé la requête parce que l’horloge de cet ordinateur est inexacte. Réglez la date et l’heure automatiquement, puis réessayez.',
	// source: 8f11c96b
	'ops.problem.message.clockSkew.ahead':
		'Le service a refusé la requête parce que l’horloge de cet ordinateur avance d’environ {minutes} minutes sur la sienne. Réglez la date et l’heure automatiquement, puis réessayez.',
	// source: 18524504
	'ops.problem.message.clockSkew.behind':
		'Le service a refusé la requête parce que l’horloge de cet ordinateur retarde d’environ {minutes} minutes sur la sienne. Réglez la date et l’heure automatiquement, puis réessayez.',
	// source: 89438320
	'ops.problem.message.io': 'Le système a signalé un problème : {message}',
	// source: 79bc82df
	'ops.problem.message.connection':
		'{reason} L’arrêt s’est produit à {location}. Réessayer rétablit la connexion.',
	// source: c24681d1
	'ops.problem.message.archiveLimit':
		'{location} dépasse les limites prévues pour les archives; son extraction a donc été arrêtée. Les limites se règlent dans les paramètres, sous Opérations.',
	// source: bbf68959
	'ops.problem.details.archiveEntries': 'Elle contient {found} entrées; la limite est de {max}.',
	// source: dd194e68
	'ops.problem.details.archiveBytes': 'Elle se déploie en {found}; la limite est de {max}.',
	// source: ba3a3a19
	'ops.problem.details.archiveRatio':
		'Elle se déploie en {ratio} fois sa propre taille; la limite est de {max} fois.',
	// source: 45989de4
	'ops.problem.details': 'Détails',
	// source: f924415e
	'ops.problem.details.expected': 'Somme de contrôle attendue : {digest}',
	// source: 62b68cd0
	'ops.problem.details.actual': 'Somme de contrôle obtenue : {digest}',
	// source: 942087cc
	'ops.problem.retry': 'Réessayer',
	// source: 28d03596
	'ops.problem.skip': 'Ignorer',
	// source: 73b8abe4
	'ops.problem.skipAll': 'Ignorer tous les cas semblables',
	// source: 5dcca55b
	'ops.problem.cancel': 'Annuler l’opération',
	// source: 3e6f5d41
	'ops.problem.createParents': 'Recréer les dossiers',
	// source: 2b110b9c
	'ops.problem.chooseLocation': 'Choisir un autre emplacement…',
	// source: 892c45a2
	'ops.problem.later': 'Décider plus tard',
	// source: 0cdccb51
	'ops.problem.skipAll.hint':
		'« Ignorer tous les cas semblables » ignore cet élément et tous les suivants qui échouent de la même façon.',
	// source: a935357d
	'ops.error.notFound': '{name} est introuvable',
	// source: 9ae563c2
	'ops.error.permissionDenied': 'Permission refusée pour {name}',
	// source: 2c0e35ae
	'ops.error.notEnoughSpace': 'Espace insuffisant à la destination',
	// source: ec4659f7
	'ops.error.invalidName': '« {name} » n’est pas un nom valide',
	// source: 296a81f9
	'ops.error.nameInUse': '{name} est déjà pris',
	// source: 23eb5ed7
	'ops.error.sameFolder': 'Les éléments sont déjà dans ce dossier',
	// source: a41c854e
	'ops.error.intoItself': 'Un dossier ne peut pas être placé dans lui-même',
	// source: a23d1e87
	'ops.error.protected': '{name} est protégé',
	// source: 9b5c8968
	'ops.error.trashUnavailable': 'La Corbeille est indisponible',
	// source: d353a99e
	'ops.error.cancelled': 'Annulé',
	// source: 8f611909
	'ops.error.unsupported': 'Pas encore pris en charge',
	// source: 718e8069
	'ops.error.changedSince': '{name} a changé depuis la planification',
	// source: f6f6e288
	'ops.error.verifyFailed': 'La vérification de {name} a échoué',
	// source: 4897a77f
	'ops.error.cannotReplace': '{name} ne peut pas être remplacé par une entrée d’un autre genre',
	// source: dd236500
	'ops.error.connection': 'Un serveur est injoignable ou demande une connexion.',
	// source: afa7bb0a
	'ops.error.archiveLimit': '{name} dépasse les limites prévues pour les archives',
	// source: db126aa9
	'ops.error.archiveNotWritable.readOnlyFormat':
		'{name} est une archive {format}, qui peut être lue mais pas modifiée',
	// source: 2105f548
	'ops.error.archiveNotWritable.encrypted': '{name} est chiffrée : elle ne peut pas être modifiée',
	// source: 7b93f9d3
	'ops.error.archiveNotWritable.nested':
		'{name} est dans une autre archive : elle ne peut pas être modifiée',
	// source: dd54b6cb
	'ops.error.archiveNotWritable.unsafeNames':
		'{name} contient des noms qui ont été modifiés pour être affichés en toute sécurité : la modifier les renommerait',
	// source: 84b9503f
	'ops.error.archiveNotWritable.noAtomicReplace':
		'À l’endroit où se trouve {name}, un fichier ne peut pas être remplacé en une seule étape : l’archive ne peut pas être modifiée',
	// source: 0538569d
	'ops.error.archiveNotWritable.containerReadOnly':
		'Il n’est pas possible d’écrire à l’endroit où se trouve {name} : l’archive ne peut pas être modifiée',
	// source: 45da8fd3
	'ops.error.undoStale.missing': '{name} n’est plus à son emplacement',
	// source: e71102d4
	'ops.error.undoStale.changed': '{name} a été modifié depuis',
	// source: f06f0516
	'ops.error.undoStale.nameTaken': 'autre chose porte maintenant le nom qu’avait {name}',
	// source: bc6645d9
	'ops.error.undoStale.trashEmptied': '{name} n’est plus dans la Corbeille',
	// source: fce43c71
	'ops.error.undoStale.unverified': '{name} n’a pas pu être vérifié; il a donc été laissé tel quel',
	// source: 1e11588a
	'ops.error.undoUnavailable': 'Il n’y a rien à annuler',
	// source: ab827e3f
	'ops.error.io': 'Une erreur s’est produite',
	// source: 7943f3b4
	'ops.announce.started': 'Démarré : {title}',
	// source: d793e100
	'ops.announce.finished': 'Terminé : {title}',
	// source: 6907eb19
	'ops.announce.failed': 'Échec : {title}. {reason}',
	// source: 9b511a8c
	'ops.announce.waiting': '{title} attend votre réponse',
	// source: 4c66b528
	'ops.announce.offline': '{title} a perdu sa connexion et réessaiera automatiquement',
	// source: 37d03e99
	'ops.announce.cancelled': 'Annulé : {title}',
	// source: 2f8caff2
	'ops.announce.milestone': '{title} : {percent} % terminé, {done} sur {total} éléments',
	// source: 5b43da80
	'ops.announce.remaining.one': '{count} opération toujours en cours',
	// source: f8ea8d50
	'ops.announce.remaining.many': '{count} d’opérations toujours en cours',
	// source: f8ea8d50
	'ops.announce.remaining.other': '{count} opérations toujours en cours',
	// source: 377ffab6
	'ops.undo.failed': 'Impossible d’annuler : {reason}',
	// source: a418093a
	'ops.undo.entryGone': 'Ce changement a déjà été annulé ou ne figure plus dans l’historique',
	// source: bf249234
	'ops.redo.failed': 'Impossible de rétablir : {reason}',
	// source: 7371662e
	'ops.title.createFolder.named': 'Création du dossier {name}',
	// source: 897ac7a5
	'ops.title.createFile.named': 'Création du fichier {name}',
	// source: 81d4334a
	'ops.title.rename.named': 'Renommage de {name}',
	// source: 8a60b229
	'ops.done.createFolder.named': 'Dossier {name} créé',
	// source: 2c180bc0
	'ops.done.createFile.named': 'Fichier {name} créé',
	// source: d78a8f57
	'ops.done.rename.named': '{name} renommé',
	// source: 0b8a79ff
	'ops.recovery.one': 'Une opération a été interrompue : {label}',
	// source: 126885d4
	'ops.recovery.many': '{count} d’opérations ont été interrompues, dont : {label}',
	// source: 126885d4
	'ops.recovery.other': '{count} opérations ont été interrompues, dont : {label}',
	// source: a046a810
	'ops.recovery.resumable':
		'Un transfert s’est arrêté quand sa connexion a été perdue : {label}. Reprendre le poursuit là où il s’est arrêté.',
	// source: 85dbc7b0
	'ops.recovery.resumableOf':
		'Un transfert s’est arrêté quand sa connexion a été perdue ({n} sur {count}) : {label}. Reprendre le poursuit là où il s’est arrêté.',
	// source: d640c742
	'ops.recovery.resume': 'Reprendre',
	// source: a3611297
	'ops.recovery.resumeFailed': 'Le transfert n’a pas pu reprendre : {reason}',
	// source: 3b7f0324
	'ops.recovery.unnamed': 'Une opération a été interrompue.',
	// source: 6ecc3df6
	'chrome.move': 'Déplacer',
	// source: 25b01bd5
	'chrome.alwaysOnTop': 'Toujours au premier plan',
	// source: b7a7dc8e
	'chrome.systemWindowMenu': 'Plus d’options…',
	// source: 7d9eb7ac
	'chrome.close': 'Fermer',
	// source: 7878b343
	'chrome.windowMenu': 'Menu de la fenêtre',
	// source: 9cea69fd
	'chrome.windowControls': 'Commandes de la fenêtre',
	// source: d4b1ea57
	'overview.title': 'Vue d’ensemble',
	// source: 1d067d8d
	'overview.loading': 'Lecture des volumes…',
	// source: 6bd6df68
	'overview.stats.label': 'Totaux',
	// source: ae65d096
	'overview.stat.capacity': 'Capacité',
	// source: ed2c8dc5
	'overview.stat.capacity.note.one':
		'Compte {count} volume local. Les partages réseau et les images disque sont exclus.',
	// source: 8a5481b1
	'overview.stat.capacity.note.many':
		'Compte {count} de volumes locaux. Les partages réseau et les images disque sont exclus.',
	// source: 8a5481b1
	'overview.stat.capacity.note.other':
		'Compte {count} volumes locaux. Les partages réseau et les images disque sont exclus.',
	// source: 8922e83d
	'overview.stat.capacity.none': 'Aucun volume local n’a indiqué sa taille.',
	// source: f411a1fb
	'overview.stat.free': 'Libre',
	// source: 22222acb
	'overview.stat.free.note': 'Sur les mêmes volumes',
	// source: 3a786953
	'overview.stat.home': 'Dossier personnel',
	// source: baf7f6c7
	'overview.stat.home.notMeasured': 'Pas encore mesuré',
	// source: a1f421df
	'overview.stat.home.measuring': 'Mesure en cours…',
	// source: 1539c30c
	'overview.stat.home.note': '{percent} % de l’espace utilisé sur {volume}',
	// source: 40ae9e7f
	'overview.stat.home.noteUnknown': 'Sa part de l’espace utilisé n’est pas encore connue',
	// source: 53a480e3
	'overview.stat.volumes': 'Volumes',
	// source: 0b583ba0
	'overview.stat.volumes.noSystems': 'Aucun système de fichiers signalé',
	// source: 45a94cc5
	'overview.unavailable.title': 'La liste des volumes n’est pas disponible',
	// source: 9f554951
	'overview.unavailable.fallback': 'Ce système ne permet pas à Waypoint de lister ses volumes.',
	// source: fbf4879e
	'overview.unavailable.showing':
		'Affichage du volume qui contient votre dossier personnel à la place.',
	// source: 53a480e3
	'overview.volumes.label': 'Volumes',
	// source: 11bc3b7d
	'overview.volumes.empty': 'Aucun volume à afficher.',
	// source: 78fad13c
	'overview.volume.unknownFileSystem': 'Système de fichiers non signalé',
	// source: fdb58ac8
	'overview.volume.homeVolume': 'Volume du dossier personnel',
	// source: 6725e7bb
	'overview.volume.badge.system': 'Système',
	// source: 8d81d2ff
	'overview.volume.badge.removable': 'Amovible',
	// source: 1744b964
	'overview.volume.badge.network': 'Réseau',
	// source: 8d321d84
	'overview.volume.badge.optical': 'Optique',
	// source: 1af85190
	'overview.volume.size': 'Capacité',
	// source: f411a1fb
	'overview.volume.free': 'Libre',
	// source: d7d557fa
	'overview.volume.unavailable': 'Taille indisponible',
	// source: 669cf449
	'overview.volume.notMounted': 'Non monté',
	// source: a47ec4c8
	'overview.volume.locked': 'Verrouillé. Déverrouillez-le pour voir son espace.',
	// source: 64dff6c3
	'overview.volume.unmeasured': 'Non mesuré. L’accès à un partage réseau peut être lent.',
	// source: a1f421df
	'overview.volume.measuring': 'Mesure en cours…',
	// source: 7ce78838
	'overview.volume.measureFailed': 'Impossible de mesurer {name}.',
	// source: 6ba9906e
	'overview.volume.almostFull': 'Presque plein',
	// source: f687d181
	'overview.volume.open': 'Ouvrir {name}',
	// source: ebf940f2
	'overview.action.measure': 'Mesurer',
	// source: 278868ce
	'overview.action.measureVolume': 'Mesurer {name}',
	// source: d5605821
	'overview.action.mount': 'Monter',
	// source: 05637203
	'overview.action.mountVolume': 'Monter {name}',
	// source: 4ac709aa
	'overview.action.unlock': 'Déverrouiller',
	// source: cf2aac84
	'overview.action.unlockVolume': 'Déverrouiller {name}',
	// source: 6f4f4130
	'overview.bar.label':
		'{name} : {used} utilisés et {free} libres sur {total} ({percent} % utilisés)',
	// source: 9e11800e
	'overview.bar.labelAlmostFull':
		'{name} : {used} utilisés et {free} libres sur {total} ({percent} % utilisés). Presque plein.',
	// source: 3b113533
	'overview.bar.labelHome':
		'{name} : {files} de vos fichiers et {other} de tout le reste, {free} libres sur {total} ({percent} % utilisés)',
	// source: 1871d453
	'overview.bar.legend': 'Espace sur {name}',
	// source: ae7d8dfa
	'overview.bar.legend.used': 'Utilisé',
	// source: f411a1fb
	'overview.bar.legend.free': 'Libre',
	// source: a2b8baaa
	'overview.bar.legend.files': 'Vos fichiers',
	// source: cd6c5f59
	'overview.bar.legend.other': 'Tout le reste',
	// source: 0d914b2d
	'overview.bar.legend.filesPending': 'Vos fichiers : pas encore mesurés',
	// source: 8b9b7f33
	'overview.bar.value': '{size} ({percent} %)',
	// source: 2fda222e
	'overview.announce.measured': '{name} mesuré',
	// source: daa8c086
	'overview.home.title': 'Plus gros dossiers du dossier personnel',
	// source: cb7b4f75
	'overview.home.rows.label': 'Plus gros dossiers du dossier personnel, du plus gros au plus petit',
	// source: 056de2f7
	'overview.home.measureNow': 'Mesurer maintenant',
	// source: 90b4991c
	'overview.home.measureNow.label': 'Mesurer le dossier personnel maintenant',
	// source: 19766ed6
	'overview.home.cancel': 'Annuler',
	// source: e9069c03
	'overview.home.cancel.label': 'Annuler la mesure du dossier personnel',
	// source: 7941c8b5
	'overview.home.asOf': 'en date de {time}',
	// source: aa92f04e
	'overview.home.notMeasured':
		'Le dossier personnel n’a pas encore été mesuré. Choisissez Mesurer maintenant pour voir où est passé son espace.',
	// source: bdc5653f
	'overview.home.progress': 'Mesure du dossier personnel : {done} dossiers sur {total} terminés',
	// source: b59bf762
	'overview.home.progress.unknown': 'Mesure du dossier personnel…',
	// source: 552da7b7
	'overview.home.cancelled': 'La mesure s’est arrêtée. Voici les dossiers mesurés jusqu’ici.',
	// source: 1e2ddb5e
	'overview.home.failed': 'Le dossier personnel n’a pas pu être mesuré : {reason}',
	// source: 7cfef0ea
	'overview.home.empty': 'Aucun dossier trouvé dans le dossier personnel.',
	// source: f2b1d58a
	'overview.home.remainder': 'Autres fichiers et dossiers, y compris les masqués',
	// source: c115bff2
	'overview.home.row.label': '{name}, {size}, {percent} % du dossier personnel',
	// source: f687d181
	'overview.home.row.open': 'Ouvrir {name}',
	// source: 02a35530
	'overview.home.row.share': '{percent} %',
	// source: 355a9c6d
	'overview.home.row.shareSmall': 'moins de 1 %',
	// source: 93655533
	'overview.home.row.underOne': 'moins de 1',
	// source: cb2eddad
	'overview.home.placeholders.one':
		'{count} fichier uniquement infonuagique a été compté comme vide et n’a pas été téléchargé.',
	// source: a63a8e4f
	'overview.home.placeholders.many':
		'{count} de fichiers uniquement infonuagiques ont été comptés comme vides et n’ont pas été téléchargés.',
	// source: a63a8e4f
	'overview.home.placeholders.other':
		'{count} fichiers uniquement infonuagiques ont été comptés comme vides et n’ont pas été téléchargés.',
	// source: 67cab30d
	'overview.home.announce.start': 'Mesure du dossier personnel',
	// source: e0699c72
	'overview.home.announce.progress': 'Mesure du dossier personnel : {percent} % terminé',
	// source: 2b3dfcfe
	'overview.home.announce.done': 'Dossier personnel mesuré : {size}',
	// source: 03732d52
	'overview.home.announce.cancelled': 'Mesure du dossier personnel arrêtée',
	// source: c560122a
	'overview.trash.title': 'Corbeille',
	// source: f6b684f2
	'overview.trash.reading': 'Lecture de la Corbeille…',
	// source: fb8e7a1a
	'overview.trash.items': 'Éléments',
	// source: c6c094bc
	'overview.trash.empty': 'Vide',
	// source: 208a19d5
	'overview.trash.count.one': '{count} élément',
	// source: f65216b3
	'overview.trash.count.many': '{count} d’éléments',
	// source: f65216b3
	'overview.trash.count.other': '{count} éléments',
	// source: 040f7884
	'overview.trash.sizeUnknown': 'Non mesurée',
	// source: ababfa97
	'overview.trash.open': 'Ouvrir la Corbeille',
	// source: b85cf088
	'overview.trash.emptyAction': 'Vider la Corbeille',
	// source: 5329df0a
	'overview.trash.unavailable': 'La Corbeille ne peut pas être parcourue ici.',
	// source: 81dff898
	'overview.trash.unverified':
		'Compte aussi la Corbeille des autres lecteurs, ce qui n’a pas encore été vérifié sur tous les types de lecteurs.',
	// source: c7b9a857
	'trash.bar.label': 'Actions de la Corbeille',
	// source: a76e13b9
	'trash.restore': 'Restaurer',
	// source: 48c015ad
	'trash.delete': 'Supprimer définitivement',
	// source: b85cf088
	'trash.empty': 'Vider la Corbeille',
	// source: a57414fb
	'trash.view.empty': 'La Corbeille est vide.',
	// source: cdb42ed4
	'trash.hint.open':
		'Les éléments de la Corbeille ne peuvent pas être ouverts. Restaurez un élément pour l’ouvrir.',
	// source: 5ec9185c
	'trash.unavailable.title': 'La Corbeille ne peut pas être parcourue ici',
	// source: f58a00f2
	'trash.unavailable.fallback':
		'Ce système ne permet pas à Waypoint de lister le contenu de la Corbeille.',
	// source: c1b15a37
	'trash.confirm.delete.title': 'Supprimer définitivement?',
	// source: f9217738
	'trash.confirm.delete.message.one':
		'{count} élément sera supprimé définitivement. Cette action est irréversible.',
	// source: c331d390
	'trash.confirm.delete.message.many':
		'{count} d’éléments seront supprimés définitivement. Cette action est irréversible.',
	// source: c331d390
	'trash.confirm.delete.message.other':
		'{count} éléments seront supprimés définitivement. Cette action est irréversible.',
	// source: 2fe3933a
	'trash.confirm.empty.title': 'Vider la Corbeille?',
	// source: 0c5a0027
	'trash.confirm.empty.message.one':
		'{count} élément de la Corbeille sera supprimé définitivement. Cette action est irréversible.',
	// source: 3e545b69
	'trash.confirm.empty.message.many':
		'{count} d’éléments de la Corbeille seront supprimés définitivement. Cette action est irréversible.',
	// source: 3e545b69
	'trash.confirm.empty.message.other':
		'{count} éléments de la Corbeille seront supprimés définitivement. Cette action est irréversible.',
	// source: 19766ed6
	'trash.confirm.cancel': 'Annuler',
	// source: 144c58d0
	'trash.done.restored.one': '{count} élément restauré',
	// source: 59cb1792
	'trash.done.restored.many': '{count} d’éléments restaurés',
	// source: 59cb1792
	'trash.done.restored.other': '{count} éléments restaurés',
	// source: ef8a4450
	'trash.done.deleted.one': '{count} élément supprimé définitivement',
	// source: a885bb0a
	'trash.done.deleted.many': '{count} d’éléments supprimés définitivement',
	// source: a885bb0a
	'trash.done.deleted.other': '{count} éléments supprimés définitivement',
	// source: d7fcf7d6
	'trash.done.emptied': 'Corbeille vidée',
	// source: 62510927
	'trash.done.skipped.one': '{count} élément est resté dans la Corbeille',
	// source: 4114d868
	'trash.done.skipped.many': '{count} d’éléments sont restés dans la Corbeille',
	// source: 4114d868
	'trash.done.skipped.other': '{count} éléments sont restés dans la Corbeille',
	// source: a9358b94
	'trash.failed.restore': 'Impossible de restaurer : {reason}',
	// source: 6de9b511
	'trash.failed.delete': 'Impossible de supprimer définitivement : {reason}',
	// source: de13a334
	'trash.failed.empty': 'Impossible de vider la Corbeille : {reason}',
	// source: 43a9179a
	'trash.failed.submit': 'Impossible de démarrer la tâche : {reason}',
	// source: 52d90550
	'trash.reason.notFound': '{location} est introuvable',
	// source: ae51c3c2
	'trash.reason.permissionDenied': 'permission refusée pour {location}',
	// source: ef7f4951
	'trash.reason.nameInUse': '{location} existe déjà',
	// source: b345f80e
	'trash.reason.cannotReplace':
		'{location} ne peut pas être remplacé par un élément d’un autre genre',
	// source: c340dc01
	'trash.reason.generic': 'une erreur s’est produite',
	// source: d7cfab3c
	'app.name': 'Waypoint',
	// source: d7919455
	'appMenu.label': 'Menu de l’application',
	// source: 50009ce1
	'appMenu.file': 'Fichier',
	// source: 464c4ffd
	'appMenu.edit': 'Édition',
	// source: dcc839a4
	'appMenu.view': 'Affichage',
	// source: 19734a1b
	'appMenu.window': 'Fenêtre',
	// source: f40ef806
	'appMenu.history': 'Historique des annulations',
	// source: 35d54994
	'appMenu.history.empty': 'Rien dans l’historique pour le moment',
	// source: d2516a84
	'appMenu.history.entry': '{label} — {time}',
	// source: 9f179170
	'appMenu.history.undone': '{label} — {time} (annulé)',
	// source: 5766fbaf
	'appMenu.history.later': 'Seul le changement le plus récent peut être annulé, puis le suivant',
	// source: 7469c2f6
	'cmd.newWindow': 'Nouvelle fenêtre',
	// source: b2f8fd63
	'cmd.newTab': 'Nouvel onglet',
	// source: 661cc7f7
	'cmd.connectToServer': 'Se connecter à un serveur…',
	// source: c75ba807
	'cmd.newFolder': 'Nouveau dossier',
	// source: d23b5dc6
	'cmd.newFile': 'Nouveau fichier',
	// source: fa400322
	'cmd.batchRename': 'Renommage en lot…',
	// source: a73477d7
	'cmd.openWith': 'Ouvrir avec…',
	// source: d1ec69e6
	'cmd.selectAll': 'Tout sélectionner',
	// source: 995fe6fd
	'cmd.invertSelection': 'Inverser la sélection',
	// source: f7efa7bc
	'cmd.sidebar': 'Barre latérale',
	// source: 338c8ac8
	'cmd.shelf': 'Étagère',
	// source: e7d38be5
	'cmd.addToShelf': 'Ajouter à l’Étagère',
	// source: 46a03326
	'cmd.focusShelf': 'Placer le focus sur l’Étagère',
	// source: 363f85ec
	'cmd.undockShelf': 'Détacher l’Étagère',
	// source: 29f74b5c
	'cmd.dockShelf': 'Ancrer l’Étagère',
	// source: da188e3b
	'cmd.inspector': 'Inspecteur',
	// source: ae43692b
	'cmd.properties': 'Propriétés',
	// source: e03cc727
	'cmd.propertiesInWindow': 'Propriétés dans une fenêtre',
	// source: f543a9f9
	'cmd.actionBar': 'Barre d’actions',
	// source: 0084cf9d
	'cmd.splitView': 'Vue divisée',
	// source: 25b01bd5
	'cmd.alwaysOnTop': 'Toujours au premier plan',
	// source: 7a04083b
	'cmd.settings': 'Paramètres…',
	// source: 32fd144c
	'cmd.closeWindow': 'Fermer la fenêtre',
	// source: 8fd3bc48
	'cmd.pauseAll': 'Suspendre toutes les opérations',
	// source: 0d889459
	'cmd.resumeAll': 'Reprendre toutes les opérations',
	// source: 8bc148d3
	'openWith.default': '{name} (par défaut)',
	// source: 81fc1029
	'openWith.other': 'Autre application…',
	// source: 0746b782
	'openWith.loading': 'Recherche des applications…',
	// source: e7d0eeb7
	'openWith.dialog.title': 'Ouvrir avec',
	// source: 95ac58cb
	'openWith.dialog.description.one': 'Choisissez une application pour ouvrir cet élément.',
	// source: 7db2e1af
	'openWith.dialog.description.many':
		'Choisissez une application pour ouvrir ces {count} d’éléments.',
	// source: 7db2e1af
	'openWith.dialog.description.other':
		'Choisissez une application pour ouvrir ces {count} éléments.',
	// source: a3218fa6
	'openWith.filter.label': 'Rechercher une application',
	// source: 98e33b0f
	'openWith.apps.label': 'Applications',
	// source: d70604e8
	'openWith.section.recommended': 'Recommandées',
	// source: b2be04d2
	'openWith.section.others': 'Autres applications',
	// source: 675959ec
	'openWith.empty': 'Aucune application ne correspond.',
	// source: 19766ed6
	'openWith.cancel': 'Annuler',
	// source: b00c803b
	'openWith.mixed': 'Ouvrir avec exige des éléments d’un seul type.',
	// source: 848dd474
	'openWith.noHandler': 'Aucune application n’est définie pour ouvrir ce type de fichier.',
	// source: fe9689e5
	'openWith.failed': 'Impossible d’ouvrir l’élément.',
	// source: 4ba29408
	'openWith.failed.app': 'Impossible d’ouvrir l’élément avec {app}.',
	// source: c99bc47e
	'openWith.failed.list': 'Impossible de trouver les applications pour cet élément.',
	// source: ed1d4b77
	'quickLook.position': '{position} sur {total}',
	// source: 9ea844f4
	'quickLook.positionAnnouncement': '{name}, {position} sur {total}',
	// source: ed077f3d
	'quickLook.open': 'Ouvrir',
	// source: a73477d7
	'quickLook.openWith': 'Ouvrir avec…',
	// source: 7d9eb7ac
	'quickLook.close': 'Fermer',
	// source: 81b35f1b
	'quickLook.previous': 'Élément précédent',
	// source: 1e47d4f7
	'quickLook.next': 'Élément suivant',
	// source: b737abb4
	'quickLook.loading': 'Chargement de l’aperçu…',
	// source: 2d632934
	'quickLook.truncated': 'Seul le début du fichier est affiché.',
	// source: a2210b79
	'quickLook.lossy':
		'Ce fichier n’est peut-être pas en UTF-8; certains caractères sont donc remplacés.',
	// source: 8512ea45
	'quickLook.empty': 'Ce fichier est vide.',
	// source: 12097792
	'quickLook.failed': 'Cet élément ne peut pas être prévisualisé.',
	// source: 1c15f791
	'quickLook.failed.text': 'Le texte n’a pas pu être lu.',
	// source: 74543dd4
	'quickLook.failed.media': 'Ce média ne peut pas être lu ici.',
	// source: 167d9d02
	'quickLook.noPreview': 'Aucun aperçu n’est disponible pour ce type d’élément.',
	// source: f5387f9b
	'quickLook.fact.kind': 'Genre',
	// source: 1af85190
	'quickLook.fact.size': 'Taille',
	// source: e8ce5dca
	'quickLook.fact.modified': 'Modifié',
	// source: 74ccd433
	'quickLook.kind.folder': 'Dossier',
	// source: 1aa4cb0b
	'quickLook.kind.image': 'Image',
	// source: 71988c4d
	'quickLook.kind.text': 'Texte',
	// source: bc1b8890
	'quickLook.kind.audio': 'Audio',
	// source: d534be82
	'quickLook.kind.video': 'Vidéo',
	// source: 64d53e28
	'quickLook.kind.pdf': 'Document PDF',
	// source: 64d0b3ad
	'quickLook.kind.font': 'Police',
	// source: 66f4804e
	'quickLook.kind.archive': 'Archive',
	// source: d6bd8c0a
	'quickLook.kind.document': 'Document',
	// source: 50009ce1
	'quickLook.kind.file': 'Fichier',
	// source: a6a32dbc
	'quickLook.kind.link': 'Lien',
	// source: a869446a
	'cmd.reason.nothingSelected': 'Sélectionnez d’abord quelque chose',
	// source: d09370ec
	'cmd.reason.notAnArchive': 'Sélectionnez d’abord une archive',
	// source: 213a52b1
	'cmd.reason.nothingFocused': 'Sélectionnez un élément à renommer',
	// source: 678a49e2
	'cmd.reason.clipboardEmpty': 'Le presse-papiers est vide',
	// source: 38e67fb1
	'cmd.reason.nothingToPause': 'Aucune opération en cours',
	// source: c6adadf7
	'cmd.reason.nothingToResume': 'Aucune opération suspendue',
	// source: 1e11588a
	'cmd.reason.nothingToUndo': 'Il n’y a rien à annuler',
	// source: 55c3f0d5
	'cmd.reason.nothingToRedo': 'Il n’y a rien à rétablir',
	// source: 188e76e3
	'cmd.reason.otherPaneReadOnly': 'L’autre panneau ne peut pas être modifié',
	// source: b6473640
	'cmd.reason.noTab': 'Aucun onglet n’est ouvert',
	// source: 218f3140
	'cmd.reason.folderViewDefault': 'Ce dossier affiche la vue de la fenêtre',
	// source: 027d2ebf
	'cmd.reason.selectOne': 'Sélectionnez un élément, ou aucun pour le dossier',
	// source: 23f939b1
	'cmd.reason.noListing': 'Aucun dossier n’est ouvert',
	// source: 555bcdf0
	'cmd.reason.noQueue': 'Les opérations ne sont pas disponibles dans cette fenêtre',
	// source: ff8059dc
	'actionBar.label': 'Actions',
	// source: 18fdd549
	'actionBar.new': 'Nouveau',
	// source: bec69036
	'actionBar.sort': 'Trier',
	// source: dcc839a4
	'actionBar.view': 'Affichage',
	// source: e2d0a549
	'actionBar.delete': 'Supprimer',
	// source: 5998f3f9
	'actionBar.extractAll': 'Tout extraire',
	// source: d47d7cb0
	'actionBar.more': 'Plus',
	// source: 1a484fd3
	'actionBar.viewTo': 'Affichage : passer à {name}',
	// source: 54beef94
	'actionBar.menu.label': 'Options de la barre d’actions',
	// source: 3415bd8a
	'actionBar.hideLabels': 'Masquer les libellés',
	// source: eb064adf
	'actionBar.showLabels': 'Afficher les libellés',
	// source: 27705918
	'actionBar.hide': 'Masquer la barre d’actions',
	// source: 1882eb15
	'actionBar.withShortcut': '{name} ({keys})',
	// source: f2ac100d
	'actionBar.disabledBecause': '{name} — {reason}',
	// source: 8b26cf0d
	'cmd.commandPalette': 'Palette de commandes…',
	// source: c7b6b040
	'cmd.linkTo': 'Créer un lien vers…',
	// source: eb3455cd
	'cmd.openOverview': 'Ouvrir la Vue d’ensemble',
	// source: 746c8138
	'cmd.goTo.home': 'Aller au dossier personnel',
	// source: 0c7ce29a
	'cmd.goTo.desktop': 'Aller au Bureau',
	// source: dae02813
	'cmd.goTo.documents': 'Aller aux Documents',
	// source: 5c72aebf
	'cmd.goTo.downloads': 'Aller aux Téléchargements',
	// source: f8191576
	'cmd.goTo.pictures': 'Aller aux Images',
	// source: a33a9c07
	'cmd.goTo.music': 'Aller à la Musique',
	// source: fa66f0ee
	'cmd.goTo.videos': 'Aller aux Vidéos',
	// source: 60742381
	'cmd.goTo.trash': 'Aller à la Corbeille',
	// source: 9819b01b
	'appMenu.history.more': 'Plus dans la palette de commandes…',
	// source: b79cac92
	'appMenu.help': 'Aide',
	// source: b79cac92
	'cmd.help': 'Aide',
	// source: 59cdaa26
	'cmd.keyboardShortcuts': 'Raccourcis clavier',
	// source: 9434332e
	'cmd.tour': 'Faire la visite guidée',
	// source: 29914b78
	'cmd.about': 'À propos de Waypoint',
	// source: b79cac92
	'help.title': 'Aide',
	// source: 7edddd51
	'help.subtitle': 'Quelques repères utiles.',
	// source: 3f63057d
	'help.rows.label': 'Touches utiles',
	// source: 2fee1e28
	'help.row.commandPalette': 'Ouvrir la palette de commandes',
	// source: 2b75279f
	'help.row.newTab': 'Ouvrir un nouvel onglet',
	// source: 98e1da09
	'help.row.splitView': 'Diviser la vue en paire',
	// source: c09bb329
	'help.row.undo': 'Annuler la dernière modification de fichier',
	// source: 26758f84
	'help.row.toggleShelf': 'Afficher ou masquer l’étagère',
	// source: 68f45384
	'help.row.keyboardShortcuts': 'Voir tous les raccourcis',
	// source: 7d9eb7ac
	'help.close': 'Fermer',
	// source: 4efca0d1
	'help.about': 'À propos',
	// source: d78c78e0
	'help.art.caption': 'Chaque lieu est un point de passage',
	// source: 59cdaa26
	'shortcuts.title': 'Raccourcis clavier',
	// source: 9dd1e63b
	'shortcuts.subtitle': 'Toutes les commandes qui ont une touche, comme les menus les montrent.',
	// source: 11a6767d
	'shortcuts.done': 'Terminé',
	// source: 408bb3f1
	'shortcuts.empty': 'Aucune commande avec raccourci n’est disponible ici.',
	// source: 2821159e
	'shortcuts.list.label': 'Raccourcis : {group}',
	// source: f24116ae
	'tour.title.welcome': 'Bienvenue dans Waypoint',
	// source: 249e61ae
	'tour.text.welcome':
		'Waypoint est un gestionnaire de fichiers à onglets pour Linux et Windows. Cette courte visite présente les idées qui le distinguent. Vous pouvez la quitter en tout temps et la retrouver dans le menu Aide.',
	// source: 8f4037f6
	'tour.title.tabs': 'Les onglets sont des espaces de travail',
	// source: 2aac0416
	'tour.text.tabs':
		'Épinglez, colorez et regroupez les onglets, et divisez un onglet en paire avec {split} pour parcourir deux dossiers côte à côte. Faites glisser un onglet hors de la fenêtre pour le détacher, ou sur une autre fenêtre pour le fusionner. {reopen} rouvre un onglet fermé.',
	// source: 26412f37
	'tour.title.dnd': 'Glisser-déposer, partout',
	// source: dd9c5b98
	'tour.text.dnd':
		'Faites glisser des fichiers vers les dossiers, les onglets et la barre latérale, ainsi que vers d’autres applications et depuis celles-ci. Maintenez Ctrl pour copier, Maj pour déplacer, ou Alt pour choisir. Déposez des fichiers sur l’étagère ({shelf}) pendant que vous naviguez.',
	// source: b269dc4e
	'tour.title.commands': 'Commandes',
	// source: 3f317b64
	'tour.text.commands':
		'Appuyez sur {palette} pour la palette de commandes, où chaque commande des menus se trouve par son nom. Appuyez sur {shortcuts} pour la liste des raccourcis.',
	// source: 24feed87
	'tour.title.desktop': 'S’accorde avec votre bureau',
	// source: 6d4c834f
	'tour.text.desktop':
		'Waypoint suit le thème de votre bureau, sa couleur d’accent et l’horloge de 12 ou de 24 heures. Les fichiers supprimés vont dans la corbeille du bureau, d’où on peut les restaurer, et on peut copier et coller des fichiers, ou les glisser, entre Waypoint et d’autres applications.',
	// source: 0560ca6c
	'tour.progress': 'Étape {step} sur {total}',
	// source: 28d03596
	'tour.skip': 'Passer',
	// source: 76900f1b
	'tour.back': 'Précédent',
	// source: 1ff57a29
	'tour.next': 'Suivant',
	// source: 983f3110
	'tour.done': 'Commencer',
	// source: d7cfab3c
	'about.title': 'Waypoint',
	// source: a1874512
	'about.version': 'Version {version}',
	// source: 4f5b7f4c
	'about.versionUnavailable': 'La version n’a pas pu être lue',
	// source: 72328968
	'about.versionLoading': 'Lecture de la version…',
	// source: 29914b78
	'about.rows.label': 'À propos de Waypoint',
	// source: f3ec8e88
	'about.licence': 'Licence',
	// source: 12c4662d
	'about.licence.value': 'Apache-2.0 OR MIT',
	// source: 39e511f4
	'about.builtWith': 'Conçu avec',
	// source: 7caa1d85
	'about.builtWith.value': 'Tauri, React et Rust',
	// source: 9a6ae6bb
	'about.credit': '© 2026 Liminal HQ, Scott Morris',
	// source: 7d9eb7ac
	'about.close': 'Fermer',
	// source: 56d80c4b
	'palette.title': 'Palette de commandes',
	// source: f37cf202
	'palette.placeholder': 'Saisissez une commande',
	// source: b269dc4e
	'palette.list.label': 'Commandes',
	// source: 50009ce1
	'palette.group.file': 'Fichier',
	// source: 464c4ffd
	'palette.group.edit': 'Édition',
	// source: dcc839a4
	'palette.group.view': 'Affichage',
	// source: 6cc8519b
	'palette.group.go': 'Aller',
	// source: 8e5ea509
	'palette.group.tabs': 'Onglets',
	// source: 19734a1b
	'palette.group.window': 'Fenêtre',
	// source: 0d04bfeb
	'palette.group.app': 'Application',
	// source: 0e769600
	'palette.group.history': 'Historique',
	// source: 690dbe9d
	'palette.group.recent': 'Récentes',
	// source: 5ecc5b1e
	'palette.shortcut': 'raccourci {keys}',
	// source: 72c605d3
	'palette.unavailable': 'indisponible : {reason}',
	// source: b8d31e85
	'palette.checked': 'activé',
	// source: b4dc66dd
	'palette.unchecked': 'désactivé',
	// source: 9b06f511
	'palette.count.one': '{count} commande',
	// source: f66ed489
	'palette.count.many': '{count} de commandes',
	// source: f66ed489
	'palette.count.other': '{count} commandes',
	// source: a800a1bc
	'palette.count.none': 'Aucune commande ne correspond',
	// source: 6ac99d99
	'palette.cannotRun': '{name} est indisponible : {reason}',
	// source: 514eef02
	'history.row.undo': 'Annuler : {label}',
	// source: 83b94a92
	'history.row.undoMany': 'Annuler {count} changements jusqu’à : {label}',
	// source: 9912395c
	'history.row.redo': 'Rétablir : {label}',
	// source: 9df7fa66
	'history.row.redoMany': 'Rétablir {count} changements jusqu’à : {label}',
	// source: 251cc794
	'history.row.partly': 'partiellement annulé',
	// source: a70e7249
	'history.row.name': '{row}, {time}',
	// source: 3e79fc04
	'history.confirm.undo.title': 'Annuler {count} changements?',
	// source: 86292082
	'history.confirm.redo.title': 'Rétablir {count} changements?',
	// source: 44de4ec7
	'history.confirm.undo.message': 'Ces changements seront annulés, du plus récent au plus ancien :',
	// source: e6786b8f
	'history.confirm.redo.message':
		'Ces changements seront rétablis, du plus ancien au plus récent :',
	// source: b2ab622f
	'history.confirm.partly.title': 'Terminer une annulation interrompue en cours de route?',
	// source: d98f167f
	'history.confirm.partly.message':
		'Une annulation antérieure de ce changement s’est arrêtée en cours de route. Annuler de nouveau termine ce qui restait :',
	// source: a8283ade
	'history.confirm.undo.confirm': 'Annuler',
	// source: 74273989
	'history.confirm.redo.confirm': 'Rétablir',
	// source: 28b7d71d
	'history.done.undoOne': 'Annulation de {label}',
	// source: 499af41a
	'history.done.undoMany': '{count} changements annulés, jusqu’à : {label}',
	// source: c8973293
	'history.done.redoOne': 'Rétablissement de {label}',
	// source: 07d39863
	'history.done.redoMany': '{count} changements rétablis, jusqu’à : {label}',
	// source: b26aea77
	'history.stopped.undo': '{done} changements sur {total} annulés. Arrêt à {label} : {reason}',
	// source: 2804cef8
	'history.stopped.redo': '{done} changements sur {total} rétablis. Arrêt à {label} : {reason}',
	// source: f4b06446
	'history.stopped.first.undo': 'Rien n’a été annulé. {label} n’a pas pu être annulé : {reason}',
	// source: c8078a15
	'history.stopped.first.redo': 'Rien n’a été rétabli. {label} n’a pas pu être rétabli : {reason}',
	// source: 9d7de7fc
	'history.busy':
		'Une annulation ou un rétablissement est encore en cours. Attendez qu’il se termine.',
	// source: 77659b33
	'history.changed':
		'L’historique a changé pendant que vous décidiez; rien n’a donc été annulé ni rétabli.',
	// source: 3224144d
	'history.step.unknown': 'il ne s’est pas terminé à temps',
	// source: 4329d8cf
	'history.step.cancelled': 'il a été annulé',
	// source: f6f1fef9
	'history.confirm.partly.note':
		'L’un d’eux s’est arrêté en cours de route lors d’une annulation précédente; l’annuler de nouveau le termine.',
	// source: 338c8ac8
	'shelf.title': 'Étagère',
	// source: 338c8ac8
	'shelf.label': 'Étagère',
	// source: 338c8ac8
	'shelf.toggle': 'Étagère',
	// source: 658c840b
	'shelf.list.label': 'Éléments de l’Étagère',
	// source: 9afb0456
	'shelf.empty.title': 'L’Étagère est vide',
	// source: 200f044c
	'shelf.empty.body':
		'Faites glisser des fichiers ici, ou choisissez Ajouter à l’Étagère dans le menu d’un fichier, pour les garder à portée de main pendant que vous naviguez. L’Étagère contient des références, jamais des copies, et toutes les fenêtres la partagent.',
	// source: 3ca9f686
	'shelf.group.label': '{name} ({count})',
	// source: e1d2fc0e
	'shelf.remove': 'Retirer de l’Étagère',
	// source: 0bf35617
	'shelf.removeNamed': 'Retirer {name} de l’Étagère',
	// source: 4ab5030d
	'shelf.remove.title': 'Retirer de l’Étagère (le fichier n’est pas supprimé)',
	// source: 44e5e4bd
	'shelf.clear': 'Vider l’Étagère',
	// source: ca6d49fa
	'shelf.clear.confirm.title': 'Vider l’Étagère?',
	// source: 5f46e7c3
	'shelf.clear.confirm.message.one':
		'{count} élément sera retiré de l’Étagère. Le fichier n’est pas supprimé.',
	// source: 79c548e7
	'shelf.clear.confirm.message.many':
		'Les {count} d’éléments seront retirés de l’Étagère. Les fichiers ne sont pas supprimés.',
	// source: 79c548e7
	'shelf.clear.confirm.message.other':
		'Les {count} éléments seront retirés de l’Étagère. Les fichiers ne sont pas supprimés.',
	// source: 44e5e4bd
	'shelf.clear.confirm.confirm': 'Vider l’Étagère',
	// source: 19766ed6
	'shelf.clear.confirm.cancel': 'Annuler',
	// source: ef823931
	'shelf.menu.label': 'Élément de l’Étagère',
	// source: ed077f3d
	'shelf.menu.open': 'Ouvrir',
	// source: 075d3e5e
	'shelf.menu.reveal': 'Afficher dans le dossier',
	// source: 0e026918
	'shelf.menu.copyPath': 'Copier le chemin',
	// source: e1d2fc0e
	'shelf.menu.remove': 'Retirer de l’Étagère',
	// source: 6c689a32
	'shelf.options.label': 'Options de l’Étagère',
	// source: 3bd7e8ec
	'shelf.options.menu': 'Menu de l’Étagère',
	// source: 5e8f1145
	'shelf.close': 'Masquer l’Étagère',
	// source: 93d0b79a
	'shelf.undock': 'Détacher l’Étagère dans sa propre fenêtre',
	// source: 29bdba30
	'shelf.dock': 'Ancrer l’Étagère de nouveau dans la fenêtre',
	// source: 1261c641
	'shelf.undocked': 'L’Étagère est dans sa propre fenêtre',
	// source: 1f79d829
	'shelf.docked': 'L’Étagère est ancrée',
	// source: 4854dfdf
	'shelf.undock.failed': 'Impossible d’ouvrir l’Étagère dans sa propre fenêtre',
	// source: a3fe5291
	'shelf.dock.failed': 'Impossible d’ancrer l’Étagère',
	// source: 8a97c71f
	'shelf.window.failed': 'Impossible d’afficher ou de masquer la fenêtre de l’Étagère',
	// source: 758a458c
	'shelf.toggle.window': 'Afficher ou masquer la fenêtre de l’Étagère',
	// source: 6be36ca4
	'shelf.missing': 'Manquant',
	// source: 599e33fc
	'shelf.missing.title': 'Cet élément n’est plus à {path}',
	// source: 4f4d906f
	'shelf.divider.label': 'Redimensionner l’Étagère',
	// source: a2e379c3
	'shelf.divider.value': '{height} pixels de hauteur',
	// source: c1b21d89
	'shelf.added.one': '{count} élément ajouté à l’Étagère',
	// source: 1103a35a
	'shelf.added.many': '{count} d’éléments ajoutés à l’Étagère',
	// source: 1103a35a
	'shelf.added.other': '{count} éléments ajoutés à l’Étagère',
	// source: ac2275b1
	'shelf.added.already': 'Déjà sur l’Étagère',
	// source: da552a63
	'shelf.full': 'L’Étagère est pleine : elle contient au plus {limit} éléments',
	// source: 697345e6
	'shelf.failed': 'Impossible de modifier l’Étagère : {reason}',
	// source: 4f1f370f
	'shelf.removed.one': '{count} élément retiré de l’Étagère',
	// source: 21dd18dd
	'shelf.removed.many': '{count} d’éléments retirés de l’Étagère',
	// source: 21dd18dd
	'shelf.removed.other': '{count} éléments retirés de l’Étagère',
	// source: b796c541
	'shelf.cleared': 'Étagère vidée',
	// source: da188e3b
	'inspector.label': 'Inspecteur',
	// source: 455d5e6a
	'inspector.tabs.label': 'Onglets de l’Inspecteur',
	// source: 324b134f
	'inspector.tab.preview': 'Aperçu',
	// source: ae43692b
	'inspector.tab.properties': 'Propriétés',
	// source: 07d19d00
	'inspector.close': 'Masquer l’Inspecteur',
	// source: e218fac0
	'inspector.divider.label': 'Redimensionner l’Inspecteur',
	// source: a3fa0138
	'inspector.divider.value': '{width} pixels de largeur',
	// source: f8c10424
	'inspector.empty.title': 'Aucune sélection',
	// source: b2e5bab2
	'inspector.empty.hint': 'Sélectionnez un fichier pour voir son aperçu et ses propriétés.',
	// source: e8371112
	'inspector.preview.imageAlt': 'Aperçu de {name}',
	// source: 24ad51f6
	'inspector.preview.audioLabel': 'Lecteur audio pour {name}',
	// source: fbac7fa0
	'inspector.preview.videoLabel': 'Lecteur vidéo pour {name}',
	// source: d2f24972
	'inspector.preview.textLabel': 'Début de {name}',
	// source: 754490ec
	'inspector.preview.truncated': 'Affichage des premiers {size} du fichier',
	// source: 632e55bf
	'inspector.preview.lossy':
		'Certains caractères n’ont pas pu être lus et sont affichés sous forme de remplacement',
	// source: 8a0336d7
	'inspector.many.note':
		'La taille compte les fichiers sélectionnés; les dossiers ne sont pas inclus.',
	// source: dcd1d522
	'inspector.field.name': 'Nom',
	// source: f5387f9b
	'inspector.field.kind': 'Genre',
	// source: 1af85190
	'inspector.field.size': 'Taille',
	// source: 51cdf6b2
	'inspector.field.totalSize': 'Taille totale',
	// source: 57fd7a0c
	'inspector.field.selected': 'Sélection',
	// source: 2eaecb3d
	'inspector.field.contains': 'Contenu',
	// source: 00ca4dac
	'inspector.field.onDisk': 'Sur le disque',
	// source: 15b61974
	'inspector.field.location': 'Emplacement',
	// source: e8ce5dca
	'inspector.field.modified': 'Modifié',
	// source: d70b9e24
	'inspector.field.created': 'Créé',
	// source: 0c366b4f
	'inspector.field.accessed': 'Consulté',
	// source: 4b1b8aa3
	'inspector.field.owner': 'Propriétaire',
	// source: 34ca0e76
	'inspector.field.group': 'Groupe',
	// source: abccc78c
	'inspector.field.permissions': 'Permissions',
	// source: efc211fc
	'inspector.field.linkTarget': 'Cible du lien',
	// source: 6f51cb04
	'inspector.field.contentType': 'Type de contenu',
	// source: 64cd989e
	'inspector.field.freeSpace': 'Espace libre',
	// source: cd1d8a1f
	'inspector.field.defaultApp': 'S’ouvre avec',
	// source: 1f991c1f
	'inspector.defaultApp.none': 'Aucune application',
	// source: a46cd4eb
	'inspector.freeOf': '{free} libres sur {total}',
	// source: ca184496
	'inspector.unavailable': 'Indisponible',
	// source: a568319c
	'inspector.link.broken': 'Lien rompu',
	// source: b5530336
	'inspector.details.failed': 'Certains détails n’ont pas pu être lus.',
	// source: 6fa62b3d
	'inspector.rename': 'Renommer…',
	// source: 224798dd
	'inspector.size.calculating': 'Calcul en cours…',
	// source: 49492103
	'inspector.size.calculatingSoFar': 'Calcul en cours… {size} jusqu’ici',
	// source: 31800b69
	'inspector.size.bytes': '{bytes} octets',
	// source: 91829f41
	'inspector.size.contents': '{files}, {folders}',
	// source: f3124cb9
	'inspector.size.announce': '{name} fait {size}',
	// source: 0358ab76
	'inspector.files.one': '{count} fichier',
	// source: 63352fdc
	'inspector.files.many': '{count} de fichiers',
	// source: 63352fdc
	'inspector.files.other': '{count} fichiers',
	// source: d1f9ba4d
	'inspector.folders.one': '{count} dossier',
	// source: 27a38f81
	'inspector.folders.many': '{count} de dossiers',
	// source: 27a38f81
	'inspector.folders.other': '{count} dossiers',
	// source: 74ccd433
	'inspector.kind.folder': 'Dossier',
	// source: a6a32dbc
	'inspector.kind.link': 'Lien',
	// source: 808cc764
	'inspector.kind.special': 'Fichier spécial',
	// source: 50009ce1
	'inspector.kind.file': 'Fichier',
	// source: 1aa4cb0b
	'inspector.kind.image': 'Image',
	// source: bc1b8890
	'inspector.kind.audio': 'Audio',
	// source: d534be82
	'inspector.kind.video': 'Vidéo',
	// source: 66f4804e
	'inspector.kind.archive': 'Archive',
	// source: bf046c73
	'inspector.kind.code': 'Code source',
	// source: d6bd8c0a
	'inspector.kind.document': 'Document',
	// source: 64d53e28
	'inspector.kind.pdf': 'Document PDF',
	// source: e7ad522e
	'inspector.kind.app': 'Application',
	// source: 0eaa5cb3
	'inspector.kind.text': 'Fichier texte',
	// source: 00271ff9
	'inspector.kind.markdown': 'Document Markdown',
	// source: f3befcc6
	'inspector.kind.spreadsheet': 'Chiffrier',
	// source: 5d5ee157
	'inspector.kind.presentation': 'Présentation',
	// source: 64d0b3ad
	'inspector.kind.font': 'Police de caractères',
	// source: 0f12c3ab
	'inspector.kind.diskImage': 'Image disque',
	// source: fa7fe671
	'inspector.kind.database': 'Base de données',
	// source: f1c216dd
	'inspector.kind.config': 'Fichier de configuration',
	// source: 82fbe842
	'inspector.kind.shellScript': 'Script shell',
	// source: 72cd8a64
	'inspector.kind.executable': 'Exécutable',
	// source: c0d8d177
	'inspector.kind.certificate': 'Clé ou certificat',
	// source: 752c2f3e
	'inspector.kind.ebook': 'Livre numérique',
	// source: 4ea3c5f3
	'inspector.kind.torrent': 'Fichier torrent',
	// source: d5d0a30b
	'inspector.kind.calendar': 'Calendrier',
	// source: 2b5c3d26
	'inspector.kind.contact': 'Contact',
	// source: a5e1b5fb
	'inspector.kind.log': 'Fichier journal',
	// source: f7dedb5c
	'inspector.kind.model3d': 'Modèle 3D',
	// source: 0ee695bd
	'inspector.kind.subtitles': 'Sous-titres',
	// source: 7af36c50
	'inspector.kind.playlist': 'Liste de lecture',
	// source: 59de121d
	'inspector.kind.package': 'Paquet',
	// source: 1eb42bf2
	'properties.open': 'Ouvrir dans une fenêtre',
	// source: eab4391b
	'properties.limit':
		'Quatre fenêtres de propriétés sont déjà ouvertes. Fermez-en une pour en ouvrir une autre.',
	// source: c98c5503
	'properties.openFailed': 'Impossible d’ouvrir la fenêtre des propriétés : {reason}',
	// source: 26d870e2
	'properties.window.label': 'Propriétés de {name}',
	// source: cb661e83
	'properties.window.loading': 'Lecture des détails…',
	// source: fd909799
	'properties.window.gone.title': '{name} n’existe plus',
	// source: d03c238d
	'properties.window.gone.note':
		'Il a été supprimé ou déplacé à un endroit que cette fenêtre ne peut pas suivre. Rien de ce qui s’affiche ici n’est à jour.',
	// source: 0c41e946
	'properties.window.failed': 'Waypoint n’a pas pu lire l’élément que cette fenêtre concerne.',
	// source: 18e34b4d
	'properties.more.title': 'Plus de détails',
	// source: f55988a8
	'properties.perm.caption': 'Qui peut le lire, y écrire et l’exécuter',
	// source: c17b94b8
	'properties.perm.who': 'Qui',
	// source: 4b1b8aa3
	'properties.perm.who.owner': 'Propriétaire',
	// source: 34ca0e76
	'properties.perm.who.group': 'Groupe',
	// source: ebdad9ba
	'properties.perm.who.others': 'Autres',
	// source: 9b9a8d05
	'properties.perm.read': 'Lecture',
	// source: 3f00927a
	'properties.perm.write': 'Écriture',
	// source: 00d60e31
	'properties.perm.execute': 'Exécution',
	// source: 85a39ab3
	'properties.yes': 'Oui',
	// source: 1ea442a1
	'properties.no': 'Non',
	// source: 5e23ec6a
	'properties.field.mode': 'Mode',
	// source: c13d8621
	'properties.field.special': 'Bits spéciaux',
	// source: 59ee8c4b
	'properties.field.owner': 'Le propriétaire est',
	// source: 8d333b02
	'properties.field.group': 'Le groupe est',
	// source: a1a0ce05
	'properties.field.exactSize': 'Taille exacte',
	// source: 52d26f44
	'properties.field.allocated': 'Alloué',
	// source: 72bb9089
	'properties.field.readOnly': 'Lecture seule',
	// source: 7e6fefff
	'properties.field.hidden': 'Masqué',
	// source: a2d6cff7
	'properties.field.leadsTo': 'Mène à',
	// source: b3955ee7
	'properties.special.setuid': 'Setuid',
	// source: 462ee9b4
	'properties.special.setgid': 'Setgid',
	// source: 87471768
	'properties.special.sticky': 'Sticky',
	// source: 24ee890e
	'properties.times.utc': '{time} UTC',
	// source: 11c94bc7
	'checksum.title': 'Somme de contrôle',
	// source: 40dfc3df
	'checksum.note':
		'Une somme de contrôle lit tout le fichier; elle n’est donc calculée que sur demande.',
	// source: d704d8af
	'checksum.algorithm.label': 'Algorithme',
	// source: bbd07c4f
	'checksum.algorithm.sha256': 'SHA-256',
	// source: 6cf8f5ea
	'checksum.algorithm.blake3': 'BLAKE3',
	// source: 0b1b237e
	'checksum.calculate': 'Calculer la somme de contrôle…',
	// source: 906daf5d
	'checksum.again': 'Calculer de nouveau',
	// source: 04810639
	'checksum.largeWarning':
		'{name} fait {size}. Sa somme de contrôle le lit au complet, ce qui peut prendre du temps et garde le disque occupé.',
	// source: 9a1d6c36
	'checksum.largeConfirm': 'Calculer quand même',
	// source: 19766ed6
	'checksum.cancel': 'Annuler',
	// source: 2635625e
	'checksum.running': 'Calcul de la somme de contrôle {algorithm}…',
	// source: 56a66d60
	'checksum.progress.label': 'Progression de la somme de contrôle',
	// source: 1020e3a8
	'checksum.progress.value': '{read} sur {total}',
	// source: f70e3e9b
	'checksum.result.label': 'Somme de contrôle {algorithm}',
	// source: e21f935f
	'checksum.copy': 'Copier',
	// source: 8d525e5f
	'checksum.copied': 'Copié',
	// source: 1efa15c4
	'checksum.cancelled': 'Annulé. Rien n’a été calculé.',
	// source: 73e8d052
	'checksum.failed': 'La somme de contrôle n’a pas pu être calculée : {reason}',
	// source: 6760cf36
	'checksum.announce.done': 'La somme de contrôle {algorithm} de {name} est prête',
	// source: 5be0ad3c
	'checksum.reason.directory': 'c’est un dossier',
	// source: db60819c
	'checksum.reason.unsupported': 'seuls les fichiers de cet ordinateur peuvent être vérifiés',
	// source: cf0e3448
	'checksum.reason.missing': 'le fichier n’existe plus',
	// source: f3b739bc
	'checksum.reason.denied': 'la permission a été refusée',
	// source: 4d5bbb99
	'checksum.reason.other': 'le fichier n’a pas pu être lu',
	// source: ae532c51
	'shelf.copied.one': '{count} élément copié depuis l’Étagère',
	// source: 43f5bbbc
	'shelf.copied.many': '{count} d’éléments copiés depuis l’Étagère',
	// source: 43f5bbbc
	'shelf.copied.other': '{count} éléments copiés depuis l’Étagère',
	// source: b8c1fe23
	'shelf.copyFailed': 'Impossible de copier depuis l’Étagère : {reason}',
	// source: 8cbe0b65
	'shelf.reveal.failed': 'Impossible d’ouvrir le dossier de {name}',
	// source: b949c922
	'browse.column.git': 'Git',
	// source: 53aade77
	'browse.columns.menu.label': 'Colonnes',
	// source: 591239c6
	'git.column.show': 'État Git',
	// source: 591239c6
	'menu.sort.git': 'État Git',
	// source: e8ce5dca
	'git.change.modified': 'Modifié',
	// source: 6b02e0d3
	'git.change.added': 'Ajouté',
	// source: b48ff39c
	'git.change.deleted': 'Supprimé',
	// source: 05487af3
	'git.change.renamed': 'Renommé',
	// source: a949ceb1
	'git.change.typeChanged': 'Type modifié',
	// source: c7ba5477
	'git.change.untracked': 'Non suivi',
	// source: 7d648f5b
	'git.change.ignored': 'Ignoré',
	// source: 014659ab
	'git.change.conflicted': 'Conflit',
	// source: 08691509
	'git.side.staged': '{change}, indexé',
	// source: 58ea04df
	'git.side.unstaged': '{change}, non indexé',
	// source: fe8dcd07
	'git.mark.repository': 'Dépôt Git',
	// source: 21aea620
	'git.mark.inside.one': '{count} élément modifié à l’intérieur',
	// source: cd26c1b8
	'git.mark.inside.many': '{count} d’éléments modifiés à l’intérieur',
	// source: cd26c1b8
	'git.mark.inside.other': '{count} éléments modifiés à l’intérieur',
	// source: 85f656d3
	'git.mark.insideConflicts.one': '{count} en conflit',
	// source: 85f656d3
	'git.mark.insideConflicts.many': '{count} en conflit',
	// source: 85f656d3
	'git.mark.insideConflicts.other': '{count} en conflit',
	// source: 778e8537
	'git.head.detached': 'Tête détachée à {commit}',
	// source: e36dc64a
	'git.head.unborn': '{branch} (aucun commit)',
	// source: 5a336012
	'git.words.branch': 'Dépôt Git {repository}, sur la branche {branch}',
	// source: ffc19087
	'git.words.detached': 'Dépôt Git {repository}, tête détachée au commit {commit}',
	// source: 5f861e70
	'git.words.unborn': 'Dépôt Git {repository}, sur la branche {branch}, sans commit pour l’instant',
	// source: 4f382b2a
	'git.words.upToDate': 'À jour avec {upstream}',
	// source: e8da486e
	'git.words.ahead.one': '{count} commit en avance sur la branche amont',
	// source: 769261dd
	'git.words.ahead.many': '{count} de commits en avance sur la branche amont',
	// source: 769261dd
	'git.words.ahead.other': '{count} commits en avance sur la branche amont',
	// source: 51f7ded9
	'git.words.behind.one': '{count} commit en retard sur la branche amont',
	// source: a7ac331e
	'git.words.behind.many': '{count} de commits en retard sur la branche amont',
	// source: a7ac331e
	'git.words.behind.other': '{count} commits en retard sur la branche amont',
	// source: c699aa00
	'git.words.clean': 'Aucune modification',
	// source: 568f313a
	'git.operation.merge': 'Une fusion est en cours',
	// source: 58130307
	'git.operation.rebase': 'Un rebasage est en cours',
	// source: d6de27a6
	'git.operation.cherryPick': 'Un cherry-pick est en cours',
	// source: 0ef11abf
	'git.operation.revert': 'Une annulation (revert) est en cours',
	// source: d30ed5e6
	'git.operation.bisect': 'Une recherche par bissection est en cours',
	// source: 85f656d3
	'git.count.conflicted.one': '{count} en conflit',
	// source: 85f656d3
	'git.count.conflicted.many': '{count} en conflit',
	// source: 85f656d3
	'git.count.conflicted.other': '{count} en conflit',
	// source: 3fddc533
	'git.count.staged.one': '{count} indexé',
	// source: 3fddc533
	'git.count.staged.many': '{count} indexés',
	// source: 3fddc533
	'git.count.staged.other': '{count} indexés',
	// source: 1c6bf5e9
	'git.count.unstaged.one': '{count} non indexé',
	// source: 1c6bf5e9
	'git.count.unstaged.many': '{count} non indexés',
	// source: 1c6bf5e9
	'git.count.unstaged.other': '{count} non indexés',
	// source: 822e45d9
	'git.count.untracked.one': '{count} non suivi',
	// source: 822e45d9
	'git.count.untracked.many': '{count} non suivis',
	// source: 822e45d9
	'git.count.untracked.other': '{count} non suivis',
	// source: 903136bd
	'git.status.label': 'Branche Git',
	// source: 5843dc29
	'settings.general.gitDecorations.label': 'Afficher l’état Git',
	// source: 78869e3d
	'settings.general.gitDecorations.description':
		'Dans un dossier d’une copie de travail Git, signale les fichiers et les dossiers modifiés, ajoute une colonne Git à la liste et affiche la branche dans la barre d’état et dans l’Inspecteur. Waypoint ne fait que lire le dépôt et ne le modifie jamais. Désactivé, Waypoint ne lit rien des dépôts.',
	// source: 591239c6
	'services.name.waypoint-git': 'État Git',
	// source: b07c0f49
	'services.name.sftp': 'SFTP',
	// source: f54ab2de
	'services.name.smb': 'SMB',
	// source: d92634e4
	'services.name.webdav': 'WebDAV',
	// source: 44d6a8a7
	'services.name.s3': 'S3',
	// source: 82c08932
	'protocol.off.reason': 'Désactivé dans Paramètres → Expérimental',
	// source: d5b9458e
	'services.protocol.notBuilt': 'Non inclus dans cette version',
	// source: 9557af38
	'protocol.off.title': '{protocol} est désactivé',
	// source: e9f631e8
	'protocol.off.detail':
		'Cette adresse utilise un protocole désactivé. Vous pouvez l’activer dans Paramètres → Expérimental.',
	// source: c4dd57df
	'protocol.off.action': 'Ouvrir les paramètres expérimentaux',
	// source: d5d70220
	'nav.path.protocolOff':
		'{protocol} est désactivé. Vous pouvez l’activer dans Paramètres → Expérimental.',
	// source: 86baf491
	'connect.address.protocolOff': '{protocol} est désactivé dans Paramètres → Expérimental.',
	// source: fcdcc355
	'connect.error.protocolOff':
		'Connexion impossible : {protocol} est désactivé dans Paramètres → Expérimental.',
	// source: 1cd21129
	'connect.scheme.off': '{protocol} (désactivé)',
	// source: 74c6372b
	'network.menu.experimental': 'Ouvrir les paramètres expérimentaux…',
	// source: 86baf491
	'destination.check.protocolOff': '{protocol} est désactivé dans Paramètres → Expérimental.',
	// source: 86baf491
	'ops.problem.message.protocolOff': '{protocol} est désactivé dans Paramètres → Expérimental.',
	// source: dd56cc14
	'ops.error.protocolOff': 'Protocole désactivé',
	// source: 3dc9f569
	'settings.section.experimental': 'Expérimental',
	// source: bc8d9fb3
	'settings.group.protocols': 'Protocoles distants',
	// source: a5bb8222
	'settings.experimental.intro':
		'Ces fonctions sont conçues, mais pas encore éprouvées à l’usage; elles peuvent mal se comporter. Chacune est désactivée tant que vous ne l’activez pas, et un changement s’applique immédiatement.',
	// source: 3dc9f569
	'settings.experimental.badge': 'Expérimental',
	// source: b07c0f49
	'settings.experimental.sftp.label': 'SFTP',
	// source: 810ccd28
	'settings.experimental.sftp.description':
		'Connectez-vous à des serveurs par SFTP (SSH), puis parcourez et copiez leurs fichiers.',
	// source: f54ab2de
	'settings.experimental.smb.label': 'SMB',
	// source: e7b2f24d
	'settings.experimental.smb.description': 'Parcourez les partages Windows et Samba par SMB.',
	// source: d92634e4
	'settings.experimental.webdav.label': 'WebDAV',
	// source: ef668159
	'settings.experimental.webdav.description': 'Parcourez des serveurs WebDAV, y compris Nextcloud.',
	// source: 44d6a8a7
	'settings.experimental.s3.label': 'S3',
	// source: cd385c00
	'settings.experimental.s3.description':
		'Parcourez le stockage S3 et les stockages compatibles S3.',
	// source: 011805aa
	'settings.experimental.unavailable': 'Pas encore dans cette version.',
	// source: b949c922
	'inspector.tab.git': 'Git',
	// source: 13d6ff07
	'git.pane.repository': 'Dépôt',
	// source: 52656e81
	'git.pane.branch': 'Branche',
	// source: 94adc696
	'git.pane.upstream': 'Branche amont',
	// source: bbd4b6a8
	'git.pane.changes': 'Modifications',
	// source: c1f88e9d
	'git.pane.operation': 'En cours',
	// source: abb8fd53
	'git.pane.item': 'Élément sélectionné',
	// source: 920e413c
	'git.pane.status': 'État',
	// source: c699aa00
	'git.pane.clean': 'Aucune modification',
	// source: 924fd54d
	'git.pane.sinceCommit': 'Depuis le dernier commit',
	// source: 9c46d354
	'git.pane.diff.files.one': '{count} fichier modifié',
	// source: 959e8066
	'git.pane.diff.files.many': '{count} de fichiers modifiés',
	// source: 959e8066
	'git.pane.diff.files.other': '{count} fichiers modifiés',
	// source: aeb400b3
	'git.pane.diff.lines': '+{added} −{removed} lignes',
	// source: d51a439b
	'git.pane.diff.binary.one': '{count} fichier binaire',
	// source: f3603cb4
	'git.pane.diff.binary.many': '{count} de fichiers binaires',
	// source: f3603cb4
	'git.pane.diff.binary.other': '{count} fichiers binaires',
	// source: 29e3ab3d
	'git.pane.diff.partial': 'Certains fichiers n’ont pas été lus : ces nombres sont des minimums.',
	// source: 24a1708d
	'git.pane.commits': 'Commits récents',
	// source: c435448b
	'git.pane.loading': 'Lecture de l’historique…',
	// source: d267a85e
	'git.pane.noCommits': 'Aucun commit n’a encore modifié cet élément.',
	// source: 3e22d058
	'git.pane.truncated':
		'Il peut exister des commits plus anciens : seuls les plus récents ont été examinés.',
	// source: 52df9b88
	'git.pane.failed': 'Impossible de lire l’historique.',
	// source: dd689975
	'git.pane.commit.by': '{author}, {date}',
	// source: b003a54b
	'git.pane.many':
		'L’historique s’affiche pour un seul élément à la fois. Sélectionnez un élément pour le voir.',
	// source: 46053dfe
	'git.pane.repositoryName': 'Dépôt Git {name}',
	// source: 9a911b08
	'connect.auth.token': 'Jeton d’accès',
	// source: 916993a1
	'connect.auth.tokenHint':
		'Se connecte avec un jeton fourni par le serveur, demandé à la connexion.',
	// source: 3b0736da
	'connect.auth.autoHintSmb':
		'Utilise le mot de passe que Waypoint a retenu pour ce serveur et cet utilisateur, et le demande s’il n’en a aucun.',
	// source: 9715f26b
	'connect.auth.autoHintDav':
		'Laisse le serveur indiquer comment se connecter, puis utilise un mot de passe retenu ou en demande un.',
	// source: aa20ea4e
	'connect.address.placeholderSmb': 'smb://domaine;utilisateur@hôte/partage',
	// source: 4e236334
	'connect.address.placeholderDav': 'davs://hôte/dossier',
	// source: 79fa3361
	'connect.field.domain': 'Domaine',
	// source: 5ebaf4ff
	'connect.field.domainHint': 'Facultatif. Pour un compte de domaine Windows, comme TRAVAIL.',
	// source: 29887a5f
	'connect.field.share': 'Partage',
	// source: 780d2c63
	'connect.field.shareHint': 'Facultatif. Laissez vide pour parcourir les partages du serveur.',
	// source: 9a911b08
	'connect.field.token': 'Jeton d’accès',
	// source: 24b96146
	'connect.field.davAuth': 'Mot de passe envoyé en mode',
	// source: 0f37e150
	'connect.davAuth.auto': 'Au choix du serveur',
	// source: 568f00ed
	'connect.davAuth.basic': 'Basique (un mot de passe d’application)',
	// source: 6e2f80bf
	'connect.davAuth.digest': 'Condensé (Digest)',
	// source: 9881c85a
	'connect.field.nextcloud': 'Ceci est un serveur Nextcloud',
	// source: 342658d3
	'connect.field.nextcloudHint':
		'S’ouvre sur vos propres fichiers. Connectez-vous avec un mot de passe d’application créé dans les paramètres de sécurité de votre compte.',
	// source: a40b044e
	'connect.nextcloud.fill': 'Remplir mon dossier de fichiers',
	// source: 934afebe
	'connect.dav.unencrypted':
		'Cette connexion n’est pas chiffrée : toute personne sur le réseau peut lire ce qui est envoyé, mots de passe compris.',
	// source: 64a4a301
	'connect.problem.domainUser': 'Indiquez le nom d’utilisateur qui accompagne le domaine.',
	// source: 033fc044
	'compress.title': 'Compresser',
	// source: 6f8d5512
	'compress.description': 'Regroupe la sélection dans une nouvelle archive, dans ce dossier.',
	// source: 80c2a46d
	'compress.name': 'Nom de l’archive',
	// source: 2f343666
	'compress.format': 'Format',
	// source: 541cdce2
	'compress.format.zip': 'Zip (.zip)',
	// source: 04f9fef5
	'compress.format.tarGz': 'Tar, gzip (.tar.gz)',
	// source: c8ad4c9c
	'compress.format.tarXz': 'Tar, xz (.tar.xz)',
	// source: 53feccbd
	'compress.format.tarBz2': 'Tar, bzip2 (.tar.bz2)',
	// source: 63aa5aca
	'compress.format.tar': 'Tar, sans compression (.tar)',
	// source: 54ed0473
	'compress.format.sevenZ': '7z (.7z)',
	// source: 033fc044
	'compress.confirm': 'Compresser',
	// source: 19766ed6
	'compress.cancel': 'Annuler',
	// source: 66f4804e
	'compress.default': 'Archive',
	// source: bc70f3aa
	'compress.exists.title': 'Ajouter à l’archive existante?',
	// source: 1d58840c
	'compress.exists.message':
		'{archive} existe déjà ici. Y ajouter {what}? Cela réécrit toute l’archive ({size}), qui garde son propre format. L’archive précédente va dans la Corbeille, et l’annulation la rétablit.',
	// source: 8e3facb4
	'compress.exists.action': 'Ajouter à l’archive',
	// source: b839bf5a
	'compress.error.empty': 'Donnez un nom à l’archive.',
	// source: 6889e47e
	'compress.error.invalid': 'Un nom ne peut pas contenir de barre oblique.',
	// source: 8b274afe
	'archive.locked.title': '{name} est verrouillée',
	// source: 9ff4dfa1
	'archive.locked.detail': 'Cette archive est chiffrée. Entrez son mot de passe pour l’ouvrir.',
	// source: fdff224b
	'archive.locked.refused':
		'Ce mot de passe n’a pas été accepté. Entrez-le de nouveau pour ouvrir l’archive.',
	// source: 7014aff1
	'archive.locked.action': 'Entrer le mot de passe',
	// source: ec963ffc
	'archive.locked.checking': 'Vérification…',
	// source: 3e0d95e3
	'archive.locked.announce': '{name} déverrouillée',
	// source: 82f299da
	'archive.locked.dialogDescription':
		'Waypoint ne garde le mot de passe que jusqu’à sa fermeture et ne l’enregistre jamais.',
	// source: 5a6220d3
	'archive.locked.failed': 'Le mot de passe n’a pas pu être transmis à l’archive.',
	// source: ecbd22c9
	'archive.slow':
		'Lecture de {name}. Une archive {format} n’a pas d’index; son ouverture prend donc un moment.',
};

export default messages;
