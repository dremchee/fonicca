# Configure Fonicca {#overview}

Open **Settings** from the sidebar or press **Command/Ctrl + ,**. The main sections cover how you browse, listen and work in the editor.

## Set language and appearance {#language-and-appearance}

In **General**, set **Language** to **English** or **Русский**. Set **Appearance** to **System**, **Light** or **Dark**. The appearance setting selects which variant of the active theme Fonicca shows.

## Customize and share a theme {#themes}

1. Open **Settings → Themes** and select a built-in palette, such as **Tape**.
2. To make a custom version, open the palette's menu and choose **Duplicate**. Open **Edit** on the copy and adjust the light and dark variants. You can change the background, text, accent and waveform colors. Use the preview and contrast indicators while editing, then save.
3. Use **Export…** in the theme controls to save a theme JSON, or **Import…** in **Themes** to load a saved theme.

## Set up waveform displays {#waveforms}

Open **Settings → Waveform** to customize the editor waveform. You can also use **Waveform settings…** from the editor’s view options.

- **Peaks + RMS** shows the peak outline with the average level inside. **Peaks** emphasizes the outline and **Bars** uses discrete bars.
- **Decibels** expands quiet details visually, useful for inspecting fades and reverb tails. **Linear** keeps the display proportional to the sample amplitude.
- **Separate** gives each stereo channel its own lane. **Mixed** combines them into a single view. Choose **Theme** colors or **By frequency** to distinguish low, mid and high frequency content.

These settings change the display. To adjust the audio level itself, use [editor processing](editor.md#processing).

## Manage analysis, models and audio devices {#analysis-and-audio}

- **Analysis** shows the background queue and lets you pause or resume it, choose the CPU thread budget, and manage model downloads. In **Models**, check the installed size or remove a downloaded model to reclaim space. The audio and previously exported results stay on disk.
- **Audio** controls the [output device, level and playback behavior](playback.md#click-behavior). Choose a level meter style there if you want a different view of stereo levels.
- **Library** manages [folders](library.md#refresh), [hidden files](duplicates.md#restore), and [data import and export](library-data.md).
- **Shortcuts** lists keyboard controls in the app. **About** shows the version, storage paths and [update options](../installation.md#downloads).
