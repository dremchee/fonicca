# Export and transfer library data {#overview}

The library export saves favorites, tag names and assignments, and collections to JSON. To move to another computer, bring both the JSON and the original audio folders. Theme files have a [separate export](settings.md#themes).

## Export your assignments {#export-data}

1. Open **Settings → Library → Your data → Export…**.
2. Choose where to save the JSON. Keep it with your audio backup so it is available when setting up another installation.

The JSON includes file paths so Fonicca can match existing files. Treat it as library data if you share it with someone else.

## Import on another computer {#import-data}

1. Copy the audio folders to the target computer. In Fonicca, [add the folders](library.md#add-folders) and wait for the scan to finish.
2. Open **Settings → Library → Your data → Import…** and choose the JSON. Fonicca first matches the saved file path. If that fails and the export contains a fingerprint, it looks for matching content and size.
3. Check the import report and open a known collection or tag to confirm the assignments. Import merges them into the current library. Running the same import again keeps existing assignments without duplicating them.

If the report lists unmatched files, confirm that the files are present and indexed. Add or rescan the source and import again. A changed audio file or missing fingerprint can prevent a match at a new path.

## Locate local storage {#storage}

Open **Settings → About → Storage** to find the library database, settings file and model directory. Those files serve different purposes from the assignment export. To reinstall a model on the new computer, open **Settings → Analysis → Models**.
