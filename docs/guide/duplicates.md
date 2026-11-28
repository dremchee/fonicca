# Compare and clean up duplicate files {#overview}

Fonicca groups files with matching content. Use this view to choose which copy belongs in the library. To look for sounds with a similar character, use [Find similar](search.md#similar-sounds).

## Compare copies and choose one to keep {#compare}

1. Open **Duplicates** in the sidebar. Use **Groups** to compare sets or **Files** to see the individual copies. Filter the view with the duplicate search field.
2. Inspect the paths and dates, and preview copies if needed. The **Keep** rule can propose the shortest path, oldest file or newest file.
3. Use **Keep this copy** for an individual choice. If you always want to keep copies from a particular folder, use the folder preference in the context menu.
4. Select the groups you want to resolve and check the proposed **Keep** and **Extra** labels before applying an action. The buttons show the number of copies they will affect.

## Hide or move extra copies to Trash {#resolve}

- **Hide from library** removes extra copies from the library view while keeping their files on disk. Use this when you want to tidy the list and still need the original folder layout.
- **Move to Trash** moves the files to the operating system's Trash. Large groups may still be verifying their content. Fonicca skips unverified copies for this action.

When Fonicca resolves copies, it combines their tags, favorites and collection membership on the copy you keep. If a DAW project references a duplicate's path, hiding it preserves that path on disk.

## Restore a copy {#restore}

For a hidden entry, open **Settings → Library → Hidden files** and choose **Unhide** next to it, or use **Unhide all**. For a file moved to Trash, restore it through the operating system and [rescan the source](library.md#refresh) if the file is missing from the library.
