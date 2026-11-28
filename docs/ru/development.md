# Разработка

Fonicca использует Tauri 2, Rust, Vue 3 и TypeScript. Библиотека хранится локально в SQLite, аудиомодели запускаются в ONNX Runtime.

## Запуск из исходников {#run-from-source}

Установите Node.js 26, pnpm 9.10.0, версию Rust из `rust-toolchain.toml` и [зависимости Tauri для вашей платформы](https://v2.tauri.app/start/prerequisites/). Затем выполните:

```sh
git clone https://github.com/dremchee/fonicca.git
cd fonicca
pnpm install --frozen-lockfile
pnpm tauri dev
```

## Материалы проекта {#project-references}

- [Архитектура и границы модулей](https://github.com/dremchee/fonicca/blob/main/docs/architecture.md)
- [Сборки и выпуск версий (EN)](https://github.com/dremchee/fonicca/blob/main/docs/builds.md)
- [Зависимости и условия использования моделей](https://github.com/dremchee/fonicca/blob/main/docs/dependencies.md)
- [Загружаемые графы моделей (EN)](https://github.com/dremchee/fonicca/blob/main/model-packs/README.md)
- [Лицензии и уведомления сторонних компонентов](https://github.com/dremchee/fonicca/tree/main/docs/licenses)
