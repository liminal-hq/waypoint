// Inline glyphs for context menu items, in the same 16 px stroke style as `AppIcons`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Glyph, type IconProps } from './AppIcons';

export const FolderOpenIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M1.5 12.5v-8h4l1.5 1.5h6.5v2" />
		<path data-fill d="M1.5 12.5l2-5.5h11.5l-2 5.5z" />
	</Glyph>
);

export const NewTabIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path
			data-fill
			d="M2 13V4.5a1 1 0 0 1 1-1h3l1 1.5h6a1 1 0 0 1 1 1V13a.5.5 0 0 1-.5.5h-11A.5.5 0 0 1 2 13zM8 7.5v4M6 9.5h4"
		/>
	</Glyph>
);

export const SplitPaneIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="2" y="3" width="12" height="10" rx="1.2" />
		<path d="M8 3v10" />
	</Glyph>
);

export const TabsIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="2" y="5.5" width="12" height="8" rx="1.2" />
		<path d="M2 5.5V4a1 1 0 0 1 1-1h2.5a1 1 0 0 1 1 1v1.5M8 3h3.5a1 1 0 0 1 1 1v1.5" />
	</Glyph>
);

export const WindowIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="2" y="3" width="12" height="10" rx="1.5" />
		<path d="M2 6.5h12" />
	</Glyph>
);

export const WindowsIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="2" y="5.5" width="9" height="8" rx="1.2" />
		<path d="M5 5.5V4.2A1.2 1.2 0 0 1 6.2 3h6.6A1.2 1.2 0 0 1 14 4.2v5.6a1.2 1.2 0 0 1-1.2 1.2H11" />
	</Glyph>
);

export const ExternalLinkIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M12.5 9v3.5a1 1 0 0 1-1 1h-8a1 1 0 0 1-1-1v-8a1 1 0 0 1 1-1H7M9.5 2.5h4v4M13.5 2.5L7.5 8.5" />
	</Glyph>
);

export const CopyIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="5.5" y="5.5" width="8" height="8" rx="1.2" />
		<path d="M10.5 5.5V4a1 1 0 0 0-1-1H4a1 1 0 0 0-1 1v5.5a1 1 0 0 0 1 1h1.5" />
	</Glyph>
);

export const LinkIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M6.8 9.2a2.6 2.6 0 0 0 3.7 0l2.4-2.4a2.6 2.6 0 0 0-3.7-3.7l-.7.7M9.2 6.8a2.6 2.6 0 0 0-3.7 0L3.1 9.2a2.6 2.6 0 0 0 3.7 3.7l.7-.7" />
	</Glyph>
);

export const TrashIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M2.5 4.5h11M6 4.5V3h4v1.5M3.8 4.5l.7 8.5a1 1 0 0 0 1 .9h5a1 1 0 0 0 1-.9l.7-8.5M6.5 7v4M9.5 7v4" />
	</Glyph>
);

export const RestoreIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M3.5 6.5h6a3 3 0 0 1 0 6H6M3.5 6.5L6 4M3.5 6.5L6 9" />
	</Glyph>
);

export const EditIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path data-fill d="M10.5 3l2.5 2.5-7.5 7.5-3.2.7.7-3.2zM9 4.5L11.5 7" />
	</Glyph>
);

export const PaletteIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path
			data-fill
			d="M8 2.5a5.5 5.5 0 1 0 0 11c1 0 1.4-.7 1.2-1.4-.3-.9.2-1.6 1.1-1.6h1.2a2.5 2.5 0 0 0 2.5-2.5C14 4.9 11.3 2.5 8 2.5z"
		/>
		<path d="M5 8h.01M6.5 5.5h.01M9.5 5.5h.01" />
	</Glyph>
);

export const CircleIcon = (props: IconProps) => (
	<Glyph {...props}>
		<circle data-fill cx="8" cy="8" r="4.5" />
	</Glyph>
);

export const GroupIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path data-fill d="M8 2.5l5.5 3L8 8.5l-5.5-3z" />
		<path d="M2.5 8.5L8 11.5l5.5-3M2.5 11L8 14l5.5-3" />
	</Glyph>
);

export const UngroupIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="2.5" y="2.5" width="11" height="11" rx="1.5" strokeDasharray="2.2 2.2" />
		<path d="M5.5 8h5" />
	</Glyph>
);

export const CollapseIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M4.5 2.5L8 6l3.5-3.5M4.5 13.5L8 10l3.5 3.5" />
	</Glyph>
);

export const ExpandIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M4.5 6L8 2.5 11.5 6M4.5 10L8 13.5 11.5 10" />
	</Glyph>
);

export const PinOffIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M9.5 2.5l4 4-2 .8-2.2 2.2.4 2.6-1 1L5.5 9.5 2.5 13.5M6.5 5.5l-1 1 4 4M2.5 2.5l11 11" />
	</Glyph>
);

export const StarOffIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path
			data-fill
			d="M8 2l1.8 3.7 4 .6-2.9 2.8.7 4L8 11.2 4.4 13.1l.7-4L2.2 6.3l4-.6zM2.5 2.5l11 11"
		/>
	</Glyph>
);

export const BookmarkIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path data-fill d="M4 2.5h8v11L8 10.5l-4 3z" />
	</Glyph>
);

export const SortIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M2.5 4h6M2.5 8h4M2.5 12h2M11.5 3v10M9 10.5l2.5 2.5 2.5-2.5" />
	</Glyph>
);

export const TextIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M3 12.5l5-10 5 10M4.8 9h6.4" />
	</Glyph>
);

export const SizeIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M9.5 2.5h4v4M13.5 2.5L9 7M6.5 13.5h-4v-4M2.5 13.5L7 9" />
	</Glyph>
);

export const ClockIcon = (props: IconProps) => (
	<Glyph {...props}>
		<circle data-fill cx="8" cy="8" r="5.5" />
		<path d="M8 4.8V8l2.2 1.4" />
	</Glyph>
);

export const PauseIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="4" y="3.5" width="2.6" height="9" rx="0.6" />
		<rect data-fill x="9.4" y="3.5" width="2.6" height="9" rx="0.6" />
	</Glyph>
);

export const PlayIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path data-fill d="M5 3.5v9l7.5-4.5z" />
	</Glyph>
);

export const HistoryIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M2.5 8a5.5 5.5 0 1 0 1.7-4M2.5 2.5v3h3M8 5v3.2l2 1.3" />
	</Glyph>
);

export const TagIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path data-fill d="M2.5 2.5h5l6 6-5 5-6-6zM5.5 5.5h.01" />
	</Glyph>
);

/** A branch: two lines that part, with a dot at each end. Git's status and branch. */
export const GitIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M5 4.5v7M5 9c0-2.5 6-1 6-4" />
		<circle data-fill cx="5" cy="3" r="1.5" />
		<circle data-fill cx="5" cy="13" r="1.5" />
		<circle data-fill cx="11" cy="3.5" r="1.5" />
	</Glyph>
);

export const DriveIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="2" y="8" width="12" height="4.5" rx="1.2" />
		<path d="M2 8l1.5-4.5h9L14 8M11 10.25h.01" />
	</Glyph>
);

export const ArrowDownIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M8 3v9.5M3.5 8.5L8 13l4.5-4.5" />
	</Glyph>
);

export const EyeIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path data-fill d="M1.5 8S4 3.5 8 3.5 14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8z" />
		<circle data-fill cx="8" cy="8" r="1.8" />
	</Glyph>
);

export const ColumnsIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="2" y="3" width="12" height="10" rx="1.5" />
		<path d="M8 3v10" />
	</Glyph>
);

export const RowsIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="2" y="3" width="12" height="10" rx="1.5" />
		<path d="M2 8h12" />
	</Glyph>
);

export const LayoutIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="2" y="3" width="12" height="10" rx="1.5" />
		<path d="M7 3v10M7 8h7" />
	</Glyph>
);

export const SeparateIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="1.5" y="3.5" width="5" height="9" rx="1" />
		<rect data-fill x="9.5" y="3.5" width="5" height="9" rx="1" />
	</Glyph>
);

export const SwapIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M2.5 5.5h10M10 3l2.5 2.5L10 8M13.5 10.5h-10M6 8l-2.5 2.5L6 13" />
	</Glyph>
);

export const ResetIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M3 8a5 5 0 1 0 1.6-3.7M3 2.8v3h3" />
	</Glyph>
);

export const UndoIcon = (props: IconProps) => (
	<Glyph directional {...props}>
		<path d="M5.5 3L2.5 6l3 3M2.5 6h7a3.5 3.5 0 0 1 0 7H6" />
	</Glyph>
);

export const CloseOthersIcon = (props: IconProps) => (
	<Glyph {...props}>
		<circle data-fill cx="8" cy="8" r="5.5" />
		<path d="M6 6l4 4M10 6l-4 4" />
	</Glyph>
);

export const CloseToRightIcon = (props: IconProps) => (
	<Glyph directional {...props}>
		<path d="M2.5 8h7M7 5.5L9.5 8 7 10.5M13 3v10" />
	</Glyph>
);

export const RedoIcon = (props: IconProps) => (
	<Glyph directional {...props}>
		<path d="M10.5 3l3 3-3 3M13.5 6h-7a3.5 3.5 0 0 0 0 7H10" />
	</Glyph>
);

export const NewFolderIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path
			data-fill
			d="M2 13V4.5a1 1 0 0 1 1-1h3l1 1.5h6a1 1 0 0 1 1 1V13a.5.5 0 0 1-.5.5h-11A.5.5 0 0 1 2 13zM8 7.5v4M6 9.5h4"
		/>
	</Glyph>
);

export const NewFileIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path data-fill d="M9.5 2.5h-5a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h7a1 1 0 0 0 1-1v-7z" />
		<path d="M9.5 2.5v3h3M8 7.5v4M6 9.5h4" />
	</Glyph>
);

export const DuplicateIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="5.5" y="5.5" width="8" height="8" rx="1.2" />
		<path d="M9.5 8v3.5M7.75 9.75h3.5M10.5 5.5V4a1 1 0 0 0-1-1H4a1 1 0 0 0-1 1v5.5a1 1 0 0 0 1 1h1.5" />
	</Glyph>
);

export const DeleteForeverIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M2.5 4.5h11M6 4.5V3h4v1.5M3.8 4.5l.7 8.5a1 1 0 0 0 1 .9h5a1 1 0 0 0 1-.9l.7-8.5M6.5 7.5l3 3M9.5 7.5l-3 3" />
	</Glyph>
);

export const CutIcon = (props: IconProps) => (
	<Glyph {...props}>
		<circle data-fill cx="4.5" cy="11.5" r="1.8" />
		<circle data-fill cx="11.5" cy="11.5" r="1.8" />
		<path d="M5.6 10L11 2.5M10.4 10L5 2.5" />
	</Glyph>
);

export const PasteIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M5.5 3.5H4a1 1 0 0 0-1 1V13a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V4.5a1 1 0 0 0-1-1h-1.5" />
		<rect data-fill x="5.5" y="2" width="5" height="3" rx="0.8" />
	</Glyph>
);

export const CopyToIcon = (props: IconProps) => (
	<Glyph directional {...props}>
		<path d="M2 12.5V4.5a1 1 0 0 1 1-1h3l1 1.5h6a1 1 0 0 1 1 1v1M6.5 11.5h6.5M10.5 9l2.5 2.5-2.5 2.5" />
	</Glyph>
);

export const MoveToIcon = (props: IconProps) => (
	<Glyph directional {...props}>
		<path d="M2 12.5V4.5a1 1 0 0 1 1-1h3l1 1.5h6a1 1 0 0 1 1 1v6.5a1 1 0 0 1-1 1H9M2 9.5h6.5M6 7l2.5 2.5L6 12" />
	</Glyph>
);

/** Settings: a gear with eight teeth around a hub. */
export const SettingsIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path
			data-fill
			d="M6.90 3.23 L7.08 1.46 L8.92 1.46 L9.10 3.23 L10.60 3.84 L11.97 2.73 L13.27 4.03 L12.16 5.40 L12.77 6.90 L14.54 7.08 L14.54 8.92 L12.77 9.10 L12.16 10.60 L13.27 11.97 L11.97 13.27 L10.60 12.16 L9.10 12.77 L8.92 14.54 L7.08 14.54 L6.90 12.77 L5.40 12.16 L4.03 13.27 L2.73 11.97 L3.84 10.60 L3.23 9.10 L1.46 8.92 L1.46 7.08 L3.23 6.90 L3.84 5.40 L2.73 4.03 L4.03 2.73 L5.40 3.84z"
		/>
		<circle data-fill cx="8" cy="8" r="2" />
	</Glyph>
);

/** The radial mark that was Settings' first icon, kept as a spare: a hub with eight spokes. */
export const SunburstIcon = (props: IconProps) => (
	<Glyph {...props}>
		<circle data-fill cx="8" cy="8" r="2" />
		<path d="M8 1.8v1.7M8 12.5v1.7M1.8 8h1.7M12.5 8h1.7M3.6 3.6l1.2 1.2M11.2 11.2l1.2 1.2M12.4 3.6l-1.2 1.2M4.8 11.2l-1.2 1.2" />
	</Glyph>
);

export const SelectAllIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="2.5" y="2.5" width="11" height="11" rx="1.5" strokeDasharray="2 1.6" />
		<path d="M5.5 8.2l1.7 1.7 3.3-3.6" />
	</Glyph>
);

export const InvertSelectionIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="2.5" y="2.5" width="11" height="11" rx="1.5" />
		<path
			d="M8 2.5v11M8 2.5h3.5a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H8z"
			fill="currentColor"
			stroke="none"
		/>
	</Glyph>
);

export const ActionBarIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="1.5" y="3.5" width="13" height="5" rx="1.2" />
		<path d="M4 6h1.5M7.25 6h1.5M10.5 6H12M2 11.5h12" />
	</Glyph>
);

export const MoreIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path data-dots d="M3.5 8h.01M8 8h.01M12.5 8h.01" />
	</Glyph>
);

export const CommandPaletteIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M3 4.5 7 8l-4 3.5M8.5 12H13" />
	</Glyph>
);

export const HelpIcon = (props: IconProps) => (
	<Glyph {...props}>
		<circle data-fill cx="8" cy="8" r="6" />
		<path d="M6.2 6.3a1.9 1.9 0 0 1 3.7.5c0 1.2-1.9 1.5-1.9 2.7M8 11.4v.1" />
	</Glyph>
);

export const KeyboardIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect data-fill x="1.5" y="4" width="13" height="8" rx="1.4" />
		<path d="M4 6.8h.1M6.4 6.8h.1M8.8 6.8h.1M11.2 6.8h.1M4.5 9.4h7" />
	</Glyph>
);

export const TourIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M4 14V2.5M4 3h7.5l-1.6 2.6 1.6 2.6H4" />
	</Glyph>
);

export const InfoIcon = (props: IconProps) => (
	<Glyph {...props}>
		<circle data-fill cx="8" cy="8" r="6" />
		<path d="M8 7.2v4M8 4.9v.1" />
	</Glyph>
);

/** A box with a lid and an arrow out of it: take the contents of an archive out. */
export const ExtractIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path data-fill d="M2.5 6.5h11v6.5a.5.5 0 0 1-.5.5H3a.5.5 0 0 1-.5-.5V6.5z" />
		<path data-fill d="M2 3.5h12v3H2z" />
		<path d="M8 7.5v4M6 9.5l2 2 2-2" />
	</Glyph>
);

/** The same box with an arrow into it: pack items into an archive. */
export const CompressIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path data-fill d="M2.5 6.5h11v6.5a.5.5 0 0 1-.5.5H3a.5.5 0 0 1-.5-.5V6.5z" />
		<path data-fill d="M2 3.5h12v3H2z" />
		<path d="M8 11.5v-4M6 9.5l2-2 2 2" />
	</Glyph>
);
