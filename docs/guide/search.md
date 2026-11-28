# Find and filter sounds {#overview}

## Search by name, folder or tag {#text-search}

Open a folder, collection or tag to search there, or choose **All samples** for a library-wide search. Click **Search** or press **Command/Ctrl + F**. Enter the beginning of one or more words from a file name, folder name or library tag. For example, `kick hard` can match `Kick Hard.wav`. Each word in the query has to match.

If a track is missing, check the current view, clear the search and reset any filters. A search in **Favorites** only returns tracks you marked as favorites.

## Combine filters {#filters}

1. Choose **One-shots**, **Loops** or **Longs** to narrow the list by sound type.
2. Enter the minimum, maximum or both values in the **BPM** filter. In **Key**, choose a note and major or minor. Use **Category** to select an instrument or sound category.
3. Use the search field with your filters still active. For example, choose **Loops**, set BPM from 90 to 110 and type a pack name.
4. Use the clear-filters control to reset the analysis filters. Clear the search field separately to remove the text query.

An unknown BPM, key or category cannot match a filter for that field. Try loosening the filter while analysis runs.

## Check background analysis {#analysis}

Open **Settings → Analysis** to see progress, pause or resume work, and set the number of CPU threads. Fonicca analyzes loudness and sound type and fills BPM, key and category where it can. Some audio has no clear tempo or key. Filename clues and the optional **Sound model** help with categorization. You can browse and play tracks while analysis runs.

## Find sounds that resemble a sample {#similar-sounds}

1. Download the **Sound model** from the sidebar or **Settings → Analysis**. It needs internet access for the download and runs locally after installation.
2. Wait for analysis of the source sample and the sounds you want to compare. Right-click the source and choose **Find similar**, or press **S** while the sample is focused.
3. Audition the results, which are ranked by similarity. Add promising tracks to a [collection](organize.md#collections). Use **Back** to leave the similarity results.

Similarity compares how sounds are characterized by the model. Use text search if you need a particular file name or tag.
