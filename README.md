# FlowSeal User List Editor

Портативный редактор `.txt`-листов для Zapret / FlowSeal на Rust + egui.

## Сборка portable EXE

В PowerShell из этой папки:

```powershell
cargo build --release
```

Готовый единый файл: `target\release\flowseal-user-list-editor.exe`.
CRT подключается статически через `.cargo/config.toml`; рядом с EXE создаётся только `config.json` с последней выбранной папкой.
