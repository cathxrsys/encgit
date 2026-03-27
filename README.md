# encgit

> **Encrypted git wrapper** — храните репозиторий на GitHub в зашифрованном виде.  
> **Encrypted git wrapper** — store your repository on GitHub in encrypted form.

---

## Содержание / Table of Contents

- [Русский](#русский)
- [English](#english)

---

## Русский

### Что это

`encgit` — утилита командной строки, которая шифрует ваш локальный git-репозиторий и хранит зашифрованный контейнер на любом git-хостинге (GitHub, GitLab и т.д.). Удалённый сервер никогда не видит содержимое файлов — только зашифрованный бинарный BLOB.

### Дисклеймер

> ⚠️ **Проект предназначен для использования одним человеком.** Одновременная работа нескольких пользователей не поддерживается: нет механизма слияния конфликтов, блокировок и контроля конкурентного доступа. Если двое сделают `push` одновременно — один из них перезапишет данные другого без предупреждения. Для командной работы используйте стандартный git.

### Как это работает

```
Локальная папка → zip-архив → ChaCha20-Poly1305 по ключу деривированному по Argon2id из пароля → .data → git push → remote
```

1. При `push` рабочая директория архивируется в ZIP, шифруется с ключом, производным от пароля через Argon2id, и результат помещается в скрытую служебную папку `.encgit/` как файл `.data`.
2. Эта папка сама является отдельным git-репозиторием, который и отправляется на remote.
3. При `pull` / `clone` операция выполняется в обратном порядке.

### Требования

- Rust 1.85+ (edition 2024)
- Git, установленный в `PATH`

### Установка

```bash
git clone <this-repo>
cd encgit
cargo build --release
# бинарник: target/release/encgit
```

### Команды

| Команда | Описание |
|---------|----------|
| `encgit init <url>` | Инициализировать новый пустой зашифрованный репозиторий и отправить на remote |
| `encgit clone <url>` | Клонировать существующий зашифрованный репозиторий и расшифровать локально |
| `encgit push` | Зашифровать локальный репозиторий и отправить на remote |
| `encgit pull` | Получить зашифрованные данные с remote и расшифровать (слияние) |
| `encgit pull --force` | То же, но рабочее дерево приводится в точное соответствие с контейнером |

Глобальный флаг `--workdir <path>` задаёт рабочую директорию явно.

### Примеры использования

```bash
# Создать новый зашифрованный репозиторий
encgit init git@github.com:user/secret-repo.git
cd secret-repo

# Добавить файлы и отправить
echo "my secret" > notes.txt
encgit push

# На другой машине — клонировать
encgit clone git@github.com:user/secret-repo.git
cd secret-repo

# Получить обновления
encgit pull

# Переписать рабочее дерево точно по контейнеру
encgit pull --force
```

### Безопасность

| Свойство | Реализация |
|----------|-----------|
| Шифрование | ChaCha20-Poly1305 (AEAD) |
| Производство ключа | Argon2id — 256 МБ ОЗУ, 64 итерации (~12 секунд на среднестатистическом ПК) |
| Соль | 16 байт, случайная для каждого `push` |
| Nonce | 12 байт, случайный для каждого `push` |
| Дополнительные данные (AAD) | Параметры KDF + соль — защищают от подмены |
| Память | Секреты обнуляются через `zeroize` |

Каждый `push` создаёт новый случайный соль и nonce, поэтому два одинаковых репозитория дадут разный шифртекст.

---

## English

### What it is

`encgit` is a command-line tool that encrypts your local git repository and stores the encrypted container on any git hosting service (GitHub, GitLab, etc.). The remote server never sees the contents of your files — only an encrypted binary blob.

### Disclaimer

> ⚠️ **This project is designed for single-user use only.** Concurrent access by multiple users is not supported: there is no conflict resolution, locking, or concurrent write control. If two users push at the same time, one will silently overwrite the other's data. For team collaboration, use standard git.

### How it works

```
Local folder → ZIP archive → ChaCha20-Poly1305 with a key derived from password via Argon2id → .data → git push → remote
```

1. On `push`, the working directory is archived into a ZIP file, encrypted using a key derived from your password via Argon2id, and stored in a hidden `.encgit/` directory as the `.data` file.
2. That directory is itself a separate git repository that gets pushed to the remote.
3. On `pull` / `clone`, the process is reversed.

### Requirements

- Rust 1.85+ (edition 2024)
- Git available in `PATH`

### Installation

```bash
git clone <this-repo>
cd encgit
cargo build --release
# binary: target/release/encgit
```

### Commands

| Command | Description |
|---------|-------------|
| `encgit init <url>` | Initialize a new empty encrypted repository and push to remote |
| `encgit clone <url>` | Clone an existing encrypted repository and decrypt locally |
| `encgit push` | Encrypt the local repository and push to remote |
| `encgit pull` | Pull encrypted data from remote and decrypt (merge) |
| `encgit pull --force` | Same, but the working tree is restored to exactly match the container |

The global flag `--workdir <path>` allows you to specify the working directory explicitly.

### Usage examples

```bash
# Create a new encrypted repository
encgit init git@github.com:user/secret-repo.git
cd secret-repo

# Add files and push
echo "my secret" > notes.txt
encgit push

# On another machine — clone
encgit clone git@github.com:user/secret-repo.git
cd secret-repo

# Get updates
encgit pull

# Restore working tree to exactly match the container
encgit pull --force
```

### Security

| Property | Implementation |
|----------|---------------|
| Encryption | ChaCha20-Poly1305 (AEAD) |
| Key derivation | Argon2id — 256 MB RAM, 64 iterations (~12 seconds on the standart configuration PC) |
| Salt | 16 bytes, random per `push` |
| Nonce | 12 bytes, random per `push` |
| Additional data (AAD) | KDF parameters + salt — guards against parameter substitution |
| Memory | Secrets are zeroed via `zeroize` |

Every `push` generates a fresh random salt and nonce, so two identical repositories will produce different ciphertexts.

### License

See [LICENSE](LICENSE).
