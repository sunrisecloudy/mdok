(() => {
  const root = document.documentElement;
  const themeButton = document.querySelector('[data-theme]');
  const prefersLight = window.matchMedia?.('(prefers-color-scheme: light)').matches;
  let saved = null;
  try { saved = localStorage.getItem('mdok-theme'); } catch (_) {}
  const setTheme = (theme) => {
    root.dataset.theme = theme;
    const light = theme === 'light';
    themeButton?.setAttribute('aria-pressed', String(light));
    themeButton?.setAttribute('title', light ? 'Switch to dark theme' : 'Switch to light theme');
    try { localStorage.setItem('mdok-theme', theme); } catch (_) {}
  };
  setTheme(saved || (prefersLight ? 'light' : 'dark'));
  themeButton?.addEventListener('click', () => setTheme(root.dataset.theme === 'light' ? 'dark' : 'light'));

  const live = document.querySelector('.live');
  const announce = (message) => {
    if (!live) return;
    live.textContent = message;
    window.setTimeout(() => { live.textContent = ''; }, 1600);
  };
  const copy = async (text) => {
    if (navigator.clipboard?.writeText) return navigator.clipboard.writeText(text);
    const area = document.createElement('textarea');
    area.value = text; area.setAttribute('readonly', ''); area.style.cssText = 'position:fixed;opacity:0';
    document.body.append(area); area.select(); document.execCommand('copy'); area.remove();
  };
  document.querySelectorAll('[data-copy]').forEach((button) => {
    button.addEventListener('click', async () => {
      const target = document.getElementById(button.dataset.copy);
      if (!target) return;
      const original = button.textContent;
      try {
        await copy(target.innerText || target.textContent || '');
        button.textContent = 'Copied'; announce('Copied to clipboard');
      } catch (_) { button.textContent = 'Select'; announce('Copy unavailable; select the command'); }
      window.setTimeout(() => { button.textContent = original; }, 1500);
    });
  });
})();
