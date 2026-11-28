# Convert audio to MIDI with Basic Pitch {#overview}

Basic Pitch recognizes notes and writes a MIDI file with their timing and velocities. Start with a clear melodic part, such as solo bass, piano or guitar. MIDI describes notes for an instrument to play. Pitch bends are omitted.

## Choose the audio to transcribe {#choose-input}

| Entry point | Audio used |
| --- | --- |
| Track context menu → **Processing → Export MIDI…** | The entire source file. |
| Editor → **Export MIDI…** with an active selection | The selected passage in the edit, including changes in the editor. |
| Editor → **Export MIDI…** with no selection | The entire current edit, including unsaved changes. |

Basic Pitch accepts mono or stereo input up to 10 minutes. For a passage in a longer recording, select the passage in the editor before opening the export dialog.

## Export MIDI and check it in your DAW {#export-midi}

1. Confirm that the dialog shows the intended source. On first use, choose **Download model and export…**. Otherwise, choose **Export MIDI…**.
2. Choose where to save the MIDI file. Note recognition runs locally. The result displays the number of detected notes.
3. Import the MIDI into an instrument track in your DAW. Play it back against the recording and adjust note lengths or pitches if needed.

If few notes are detected, try a shorter, cleaner phrase or an isolated instrument track. For example, you can [separate bass into its own stem](stems.md) and transcribe that file. If the download fails, check its status under **Settings → Analysis → Models**.
