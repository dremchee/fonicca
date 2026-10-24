import type en from '../en'
import common from './common'
import settings from './settings'
import sidebar from './sidebar'
import filters from './filters'
import list from './list'
import menu from './menu'
import player from './player'
import duplicates from './duplicates'
import editor from './editor'
import palette from './palette'
import sync from './sync'
import dialogs from './dialogs'
import trackInfo from './trackInfo'

export default {
  common,
  settings,
  sidebar,
  filters,
  list,
  menu,
  player,
  duplicates,
  editor,
  palette,
  sync,
  dialogs,
  trackInfo,
} satisfies typeof en
