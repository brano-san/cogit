# F-516 · Diff называет смену режима и пустой файл

- [ ] `chmod +x script.sh` (или `git update-index --chmod=+x`) и коммит: клик по файлу в этом
      коммите показывает «Only the file mode changed: 100644 → 100755 (now executable)», а не
      «No change in this file». То же для застейдженной смены режима в Staged.
- [ ] Закоммитить пустой файл, затем удалить его коммитом: Diff говорит «An empty file was
      added» и «An empty file was deleted».
