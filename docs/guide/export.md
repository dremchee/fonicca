# Save and export audio {#overview}

Open the editor's **Save** menu and choose the command that matches the result you want.

## Choose what to save {#commands}

| Command | Result |
| --- | --- |
| **Save** | On the first save, choose a new file. Later saves update the editor's current saved target in its format and at its last export rate. |
| **Save as…** | Write the entire edit to the file you choose. The editor now uses that file as its saved target. |
| **Save selection…** | Write the selected range to a new file. The current session and its save target stay in place. |
| **Save regions** | Write included regions as separate WAV files in a folder next to the current track. See [slice export](loops-slices.md#save-regions). |
| **Replace original…** | Write the entire edit over the original WAV when eligible. Confirm the path before proceeding. Fonicca keeps a backup of the previous audio. |

**Save** is available when you have changes. **Save selection…** needs an active selection. To change the format or sample rate of the current edit, use **Save as…**.

## Pick a format and rate {#formats}

| Format | What Fonicca writes |
| --- | --- |
| WAV | Uncompressed audio. Preserves the source bit depth where supported, together with loop points and slice markers. |
| FLAC | Lossless compression, 24-bit PCM. |
| AIFF | Uncompressed 24-bit PCM. |
| M4A | Apple Lossless (ALAC), 16-bit PCM. |
| CAF | Uncompressed 32-bit float PCM. |
| MP3 | Lossy compression. The available rates depend on the encoder. |
| Ogg Vorbis | Lossy compression with a variable bitrate. |

In **Save audio**, pick the format and **Sample rate**, then click **Continue** to choose a filename and folder. Use **Same as source** to keep the current rate when the format supports it. Select WAV if another program needs the embedded [loop points or slice markers](loops-slices.md).

## Export a short clip {#export-selection}

1. Drag over the desired passage in the editor. Press **Space** to check the beginning and end of the selection.
2. Choose **Save selection…**. Select the format and sample rate, then give the excerpt a distinct filename.
3. Open the resulting file in a player or [drag it into your DAW](playback.md#daw). If you saved it outside an indexed folder, open it from the file manager or [add the folder](library.md#add-folders) to see it in Fonicca.

If Fonicca says the audio was saved but the library could not be updated, check the destination in the file manager. When it belongs to an indexed source, try rescanning that source.
