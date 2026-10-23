import { getVersion } from '@tauri-apps/api/app'
import { Menu, MenuItem, PredefinedMenuItem, Submenu } from '@tauri-apps/api/menu'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { t } from '@/i18n'

/**
 * The native menu bar: Tauri's default menu, built here so its items follow the
 * app language, plus Settings… (⌘,). Called by the main window on start and on
 * every language switch; the menu it replaces is released.
 */
export async function installAppMenu(openSettings: () => void) {
  const item = (kind: Exclude<Parameters<typeof PredefinedMenuItem.new>[0], undefined>['item'], text?: string) =>
    PredefinedMenuItem.new({ item: kind, text })
  const separator = () => item('Separator')

  const app = await Submenu.new({
    text: 'Samplette',
    items: [
      await item({ About: { name: 'Samplette', version: await getVersion() } }, t('menu.app.about')),
      await separator(),
      await MenuItem.new({
        id: 'settings',
        text: t('menu.app.settings'),
        accelerator: 'CmdOrCtrl+,',
        // The menu is shared by all windows; settings live in the main one.
        action: () => {
          getCurrentWindow().setFocus()
          openSettings()
        },
      }),
      await separator(),
      await item('Services', t('menu.app.services')),
      await separator(),
      await item('Hide', t('menu.app.hide')),
      await item('HideOthers', t('menu.app.hideOthers')),
      await item('ShowAll', t('menu.app.showAll')),
      await separator(),
      await item('Quit', t('menu.app.quit')),
    ],
  })
  const file = await Submenu.new({ text: t('menu.file.title'), items: [await item('CloseWindow', t('menu.file.closeWindow'))] })
  // Kept from the default menu: copy and paste in text fields go through these items.
  const edit = await Submenu.new({
    text: t('menu.edit.title'),
    items: [
      await item('Undo', t('menu.edit.undo')),
      await item('Redo', t('menu.edit.redo')),
      await separator(),
      await item('Cut', t('menu.edit.cut')),
      await item('Copy', t('menu.edit.copy')),
      await item('Paste', t('menu.edit.paste')),
      await item('SelectAll', t('menu.edit.selectAll')),
    ],
  })
  const view = await Submenu.new({ text: t('menu.view.title'), items: [await item('Fullscreen', t('menu.view.fullscreen'))] })
  const window = await Submenu.new({
    text: t('menu.window.title'),
    items: [
      await item('Minimize', t('menu.window.minimize')),
      await item('Maximize', t('menu.window.zoom')),
      await separator(),
      await item('CloseWindow', t('menu.file.closeWindow')),
    ],
  })
  const help = await Submenu.new({ text: t('menu.help.title'), items: [] })

  const menu = await Menu.new({ items: [app, file, edit, view, window, help] })
  const previous = await menu.setAsAppMenu()
  // macOS adds its window list to the Window menu and search to Help.
  await window.setAsWindowsMenuForNSApp()
  await help.setAsHelpMenuForNSApp()
  await previous?.close()
}
