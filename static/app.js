(() => {
  const root = document.documentElement;
  const storedTheme = localStorage.getItem('simple-stats-theme');
  if (storedTheme === 'light' || storedTheme === 'dark') {
    root.dataset.theme = storedTheme;
  }

  const themeToggle = document.querySelector('[data-theme-toggle]');
  const updateThemeLabel = () => {
    if (!themeToggle) return;
    const current = getComputedStyle(root).colorScheme;
    themeToggle.setAttribute('aria-label', current === 'dark' ? 'Включить светлую тему' : 'Включить тёмную тему');
  };
  updateThemeLabel();

  themeToggle?.addEventListener('click', () => {
    const isDark = getComputedStyle(root).colorScheme === 'dark';
    const nextTheme = isDark ? 'light' : 'dark';
    root.dataset.theme = nextTheme;
    localStorage.setItem('simple-stats-theme', nextTheme);
    updateThemeLabel();
  });

  const normalize = (value) => value.toLocaleLowerCase('ru').trim();

  document.querySelectorAll('[data-table-filter]').forEach((input) => {
    const table = document.getElementById(input.dataset.tableFilter);
    const rows = table?.querySelectorAll('[data-filter-row]') ?? [];
    const empty = table?.parentElement?.querySelector('[data-filter-empty]');

    input.addEventListener('input', () => {
      const query = normalize(input.value);
      let visible = 0;
      rows.forEach((row) => {
        const matches = normalize(row.dataset.searchValue ?? '').includes(query);
        row.hidden = !matches;
        if (matches) visible += 1;
      });
      if (empty) empty.hidden = visible !== 0;
    });
  });

  const visitFilter = document.querySelector('[data-visit-filter]');
  if (visitFilter) {
    const visits = document.querySelectorAll('[data-visit-row]');
    const empty = document.querySelector('[data-visit-empty]');
    visitFilter.addEventListener('input', () => {
      const query = normalize(visitFilter.value);
      let visible = 0;
      visits.forEach((visit) => {
        const matches = normalize(visit.dataset.searchValue ?? '').includes(query);
        visit.hidden = !matches;
        if (matches) visible += 1;
      });
      if (empty) empty.hidden = visible !== 0;
    });
  }

  document.addEventListener('keydown', (event) => {
    if (event.key !== '/' || /input|textarea/i.test(document.activeElement?.tagName ?? '')) return;
    const search = document.querySelector('input[type="search"]');
    if (search) {
      event.preventDefault();
      search.focus();
    }
  });

  document.querySelectorAll('[data-copy]').forEach((button) => {
    button.addEventListener('click', async () => {
      const original = button.textContent;
      try {
        await navigator.clipboard.writeText(button.dataset.copy ?? '');
        button.textContent = button.classList.contains('mini-copy') ? '✓' : 'Скопировано';
      } catch {
        button.textContent = 'Не удалось';
      }
      window.setTimeout(() => { button.textContent = original; }, 1400);
    });
  });
})();
