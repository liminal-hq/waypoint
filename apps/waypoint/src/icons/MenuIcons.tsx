// Inline glyphs for context menu items, in the same 16 px stroke style as `AppIcons`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Glyph, type IconProps } from './AppIcons';

export const FolderOpenIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M1.5 12.5v-8h4l1.5 1.5h6.5v2M1.5 12.5l2-5.5h11.5l-2 5.5z" />
	</Glyph>
);

export const NewTabIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M2 13V4.5a1 1 0 0 1 1-1h3l1 1.5h6a1 1 0 0 1 1 1V13a.5.5 0 0 1-.5.5h-11A.5.5 0 0 1 2 13zM8 7.5v4M6 9.5h4" />
	</Glyph>
);

export const TabsIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect x="2" y="5.5" width="12" height="8" rx="1.2" />
		<path d="M2 5.5V4a1 1 0 0 1 1-1h2.5a1 1 0 0 1 1 1v1.5M8 3h3.5a1 1 0 0 1 1 1v1.5" />
	</Glyph>
);

export const WindowIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect x="2" y="3" width="12" height="10" rx="1.5" />
		<path d="M2 6.5h12" />
	</Glyph>
);

export const WindowsIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect x="2" y="5.5" width="9" height="8" rx="1.2" />
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
		<rect x="5.5" y="5.5" width="8" height="8" rx="1.2" />
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
		<path d="M10.5 3l2.5 2.5-7.5 7.5-3.2.7.7-3.2zM9 4.5L11.5 7" />
	</Glyph>
);

export const PaletteIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M8 2.5a5.5 5.5 0 1 0 0 11c1 0 1.4-.7 1.2-1.4-.3-.9.2-1.6 1.1-1.6h1.2a2.5 2.5 0 0 0 2.5-2.5C14 4.9 11.3 2.5 8 2.5z" />
		<path d="M5 8h.01M6.5 5.5h.01M9.5 5.5h.01" />
	</Glyph>
);

export const CircleIcon = (props: IconProps) => (
	<Glyph {...props}>
		<circle cx="8" cy="8" r="4.5" />
	</Glyph>
);

export const GroupIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M8 2.5l5.5 3L8 8.5l-5.5-3zM2.5 8.5L8 11.5l5.5-3M2.5 11L8 14l5.5-3" />
	</Glyph>
);

export const UngroupIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect x="2.5" y="2.5" width="11" height="11" rx="1.5" strokeDasharray="2.2 2.2" />
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
		<path d="M8 2l1.8 3.7 4 .6-2.9 2.8.7 4L8 11.2 4.4 13.1l.7-4L2.2 6.3l4-.6zM2.5 2.5l11 11" />
	</Glyph>
);

export const BookmarkIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M4 2.5h8v11L8 10.5l-4 3z" />
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
		<circle cx="8" cy="8" r="5.5" />
		<path d="M8 4.8V8l2.2 1.4" />
	</Glyph>
);

export const HistoryIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M2.5 8a5.5 5.5 0 1 0 1.7-4M2.5 2.5v3h3M8 5v3.2l2 1.3" />
	</Glyph>
);

export const TagIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M2.5 2.5h5l6 6-5 5-6-6zM5.5 5.5h.01" />
	</Glyph>
);

export const DriveIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect x="2" y="8" width="12" height="4.5" rx="1.2" />
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
		<path d="M1.5 8S4 3.5 8 3.5 14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8z" />
		<circle cx="8" cy="8" r="1.8" />
	</Glyph>
);

export const ColumnsIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect x="2" y="3" width="12" height="10" rx="1.5" />
		<path d="M8 3v10" />
	</Glyph>
);

export const RowsIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect x="2" y="3" width="12" height="10" rx="1.5" />
		<path d="M2 8h12" />
	</Glyph>
);

export const LayoutIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect x="2" y="3" width="12" height="10" rx="1.5" />
		<path d="M7 3v10M7 8h7" />
	</Glyph>
);

export const SeparateIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect x="1.5" y="3.5" width="5" height="9" rx="1" />
		<rect x="9.5" y="3.5" width="5" height="9" rx="1" />
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
	<Glyph {...props}>
		<path d="M5.5 3L2.5 6l3 3M2.5 6h7a3.5 3.5 0 0 1 0 7H6" />
	</Glyph>
);

export const CloseOthersIcon = (props: IconProps) => (
	<Glyph {...props}>
		<circle cx="8" cy="8" r="5.5" />
		<path d="M6 6l4 4M10 6l-4 4" />
	</Glyph>
);

export const CloseToRightIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M2.5 8h7M7 5.5L9.5 8 7 10.5M13 3v10" />
	</Glyph>
);

export const RedoIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M10.5 3l3 3-3 3M13.5 6h-7a3.5 3.5 0 0 0 0 7H10" />
	</Glyph>
);

export const NewFolderIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M2 13V4.5a1 1 0 0 1 1-1h3l1 1.5h6a1 1 0 0 1 1 1V13a.5.5 0 0 1-.5.5h-11A.5.5 0 0 1 2 13zM8 7.5v4M6 9.5h4" />
	</Glyph>
);

export const NewFileIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M9.5 2.5h-5a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h7a1 1 0 0 0 1-1v-7zM9.5 2.5v3h3M8 7.5v4M6 9.5h4" />
	</Glyph>
);

export const DuplicateIcon = (props: IconProps) => (
	<Glyph {...props}>
		<rect x="5.5" y="5.5" width="8" height="8" rx="1.2" />
		<path d="M9.5 8v3.5M7.75 9.75h3.5M10.5 5.5V4a1 1 0 0 0-1-1H4a1 1 0 0 0-1 1v5.5a1 1 0 0 0 1 1h1.5" />
	</Glyph>
);

export const DeleteForeverIcon = (props: IconProps) => (
	<Glyph {...props}>
		<path d="M2.5 4.5h11M6 4.5V3h4v1.5M3.8 4.5l.7 8.5a1 1 0 0 0 1 .9h5a1 1 0 0 0 1-.9l.7-8.5M6.5 7.5l3 3M9.5 7.5l-3 3" />
	</Glyph>
);
