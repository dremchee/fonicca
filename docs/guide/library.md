# Add and browse audio folders {#overview}

Fonicca builds a library from folders you already use. It reads the original audio paths, so you can keep working with the same files in your DAW or file manager.

## Add a source folder {#add-folders}

1. Click **+** beside **Folders**, or open **Settings → Library → Add folders…**.
2. Choose a folder containing audio. Fonicca scans it and its subfolders. Repeat for sample packs, project exports or recordings stored elsewhere.
3. Open the folder from the sidebar and start [auditioning samples](playback.md). Tracks appear while indexing is in progress. Tempo, key, loudness and category can arrive later as [analysis](search.md#analysis) runs.

Fonicca reads WAV, AIFF, FLAC, MP3, OGG, M4A and CAF. A container such as M4A can hold different codecs, so the extension alone does not guarantee decoding.

## Navigate and select rows {#navigate}

Expand a source to see its subfolders. Use the path above the table to move up one or more levels. **All samples** shows tracks across your sources. A folder view includes tracks in its subfolders.

| Action | Control |
| --- | --- |
| Select and audition a row | Click a row. Double-click it to open the editor. |
| Select a range | Hold **Shift** and click another row, or hold **Shift** and use the arrow keys. |
| Add or remove individual rows from a selection | Hold **Command** on macOS or **Ctrl** on Windows and Linux while clicking. |
| Select all results in the current view | Press **Command/Ctrl + A** with the library focused. |

The selection lets you [tag](organize.md#tags), [drag](playback.md#daw) or [process](batch.md) several tracks together. Open the context menu on a selected row to act on the selection with commands that support multiple files.

## Set up the table {#columns}

Click a column heading to sort by that field. Drag the edge between two headings to resize a column. The **Columns** control shows or hides fields and includes **Reset column widths**. Enable **Directory** when you need to distinguish files with the same name across sources.

## Refresh or remove a folder {#refresh}

- Keep **Settings → Library → Watch folders for changes** on to pick up filesystem changes automatically.
- To refresh an indexed source manually, right-click it and choose **Rescan**. If it lives on an external drive, reconnect it at the same path first.
- **Remove from library** removes a source from the index and leaves its files on disk. To bring a removed source back, add the folder again. To recover an individually hidden file, use [Hidden files](duplicates.md#restore).
