import type en from '../en/trackInfo'

export default {
  edit: 'Информация…',
  title: 'Информация о треке',
  description: 'Записывается в теги самого файла, поэтому её увидят и другие программы.',
  fields: {
    title: 'Название',
    artist: 'Исполнитель',
    album: 'Альбом',
    year: 'Год',
    genre: 'Жанр',
    sampleRate: 'Частота',
    channels: 'Каналы',
    bitDepth: 'Разрядность',
    size: 'Размер файла',
  },
  cover: 'Обложка',
  noCover: 'Нет обложки',
  chooseImage: 'Выбрать картинку…',
  removeCover: 'Убрать обложку',
  newCover: 'Новая: {name}',
  coverRemoved: 'Будет удалена',
  imageFiles: 'Изображения',
  saved: 'Сохранено в файл',
} satisfies typeof en
