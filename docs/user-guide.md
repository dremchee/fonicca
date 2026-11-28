# Fonicca user guide

Fonicca is a desktop app for auditioning, organizing and editing local audio. Choose a task below for step-by-step instructions.

## Get started

1. [Install Fonicca for your platform](installation.md).
2. Click **+** beside **Folders** and [add a folder with audio](guide/library.md#add-folders).
3. Click a track to play it in the mini-player. Double-click it to open the editor.
4. To make a new clip, drag to select a range, then choose [**Save selection…**](guide/export.md#export-selection).

## Browse and organize

Start with [folders and table controls](guide/library.md), then use [search and filters](guide/search.md) to narrow the list by name, tempo, key or category.

### Work with tags

See [favorites, collections and tags](guide/organize.md) for ways to label sounds and gather tracks from different folders in one collection.

### Find similar sounds

[Find similar](guide/search.md#similar-sounds) compares sounds after you install the sound model and analysis completes.

## Preview audio

Learn to [audition samples, seek and drag them into a DAW](guide/playback.md). Use [Track info](guide/track-info.md) to check file properties or edit embedded information such as the title and cover.

## Edit and export

Use the [audio editor](guide/editor.md) to select, trim, fade or process a passage, work against a grid or inspect a spectrogram.

### Save, slice or replace

See [saving and export formats](guide/export.md) to choose between saving the edit, exporting a selection or replacing a WAV. Use [loops and slicing](guide/loops-slices.md) to save loop points or export individual regions.

## Separate stems

[Separate a track into stems](guide/stems.md) to extract vocals and backing, or split a mix into vocals, drums, bass and other instruments.

## Process audio

### Prepare a batch of samples

[Batch processing](guide/batch.md) applies a recipe to multiple files. Use it for silence trimming, fades, leveling, WAV conversion and output naming.

### Restore a track or export notes

[Restore compressed music with Apollo](guide/restoration.md) to create a separate restored WAV, or [convert audio to MIDI with Basic Pitch](guide/audio-to-midi.md) to take the detected notes into a DAW.

## Manage your data

### Duplicates and hidden files

[Review duplicate copies](guide/duplicates.md), decide which one to keep, and hide or trash the extras.

### Export favorites, tags and collections

[Export and transfer library data](guide/library-data.md) to move your assignments to another installation.

## Settings

[Configure Fonicca](guide/settings.md) to change the interface language, theme, waveform, analysis or audio settings.

## Keyboard shortcuts

Use **Command** on macOS or **Ctrl** on Windows and Linux for the commands below.

| In the library | Action |
| --- | --- |
| Up / Down | Move focus. Add **Shift** to extend the selection. |
| Space | Play or pause the focused track. |
| Esc | Stop the current preview, or leave the search field. |
| Command/Ctrl + A | Select all results in the current view. |
| Command/Ctrl + F | Focus search. |
| Command/Ctrl + , | Open Settings. |
| F / T | Toggle favorite / open the tag picker. |
| E / I / S | Open the editor / edit track information / find similar sounds. |

| In the editor | Action |
| --- | --- |
| Space | Toggle editor playback. |
| Command/Ctrl + Z | Undo. Add **Shift** to redo. |
| Command/Ctrl + S | Save. Add **Shift** for **Save as…**. |
| M | Add a marker at the cursor or at both ends of a selection. |
| 1–9 / Shift + 1–9 | Play regions 1–9 / 10–18. |
| L / Shift + L | Toggle looping / set loop points from a selection or clear them without a selection. |
| S | Toggle snap to grid. |
| + / − / 0 | Zoom in / zoom out / fit the waveform. |

## Troubleshooting

| Symptom | Where to check |
| --- | --- |
| A track is missing | [Check the search and filters](guide/search.md#text-search), [rescan the source](guide/library.md#refresh), or [restore a hidden entry](guide/duplicates.md#restore). |
| BPM, key or category is blank | [Check background analysis](guide/search.md#analysis). |
| Find similar is unavailable | [Install the sound model and wait for analysis](guide/search.md#similar-sounds). |
| Preview has no sound | [Check the preview volume and output device](guide/playback.md#audio-output). |
| Model-based processing fails | Check the input and output requirements for [stems](guide/stems.md#results), [Apollo](guide/restoration.md#troubleshooting), or [Basic Pitch](guide/audio-to-midi.md#choose-input). |

When asking for help, include your Fonicca version from **Settings → About**, operating system, audio format and steps to reproduce the problem. [GitHub Issues](https://github.com/dremchee/fonicca/issues)
