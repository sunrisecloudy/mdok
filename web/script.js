(() => {
  'use strict';

  const root = document.documentElement;
  const body = document.body;
  body.classList.add('js-ready');

  // Keep the color mode local and deterministic; no framework is needed.
  const themeToggle = document.querySelector('[data-theme-toggle]');
  const storedTheme = (() => {
    try { return localStorage.getItem('mdok-theme'); } catch (_) { return null; }
  })();
  const systemLight = window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches;
  const initialTheme = storedTheme || (systemLight ? 'light' : 'dark');

  function setTheme(theme) {
    root.dataset.theme = theme;
    if (themeToggle) {
      const light = theme === 'light';
      themeToggle.setAttribute('aria-pressed', String(light));
      themeToggle.querySelector('.theme-icon').textContent = light ? '◑' : '◐';
      themeToggle.querySelector('.theme-label').textContent = light ? 'Light' : 'Dark';
    }
    try { localStorage.setItem('mdok-theme', theme); } catch (_) { /* private mode */ }
  }
  setTheme(initialTheme);
  themeToggle?.addEventListener('click', () => setTheme(root.dataset.theme === 'light' ? 'dark' : 'light'));

  const menuToggle = document.querySelector('.menu-toggle');
  const nav = document.querySelector('#primary-nav');
  function closeMenu() {
    if (!nav || !menuToggle) return;
    nav.classList.remove('is-open');
    menuToggle.setAttribute('aria-expanded', 'false');
  }
  menuToggle?.addEventListener('click', () => {
    const open = nav.classList.toggle('is-open');
    menuToggle.setAttribute('aria-expanded', String(open));
    if (open) nav.querySelector('a')?.focus();
  });
  nav?.querySelectorAll('a').forEach((link) => link.addEventListener('click', closeMenu));
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      closeMenu();
      menuToggle?.focus();
    }
  });

  // Copy buttons use the Clipboard API when available and a selection fallback otherwise.
  const liveRegion = document.querySelector('.copy-live');
  function announce(message) {
    if (!liveRegion) return;
    liveRegion.textContent = message;
    window.setTimeout(() => { liveRegion.textContent = ''; }, 1800);
  }
  async function copyText(text) {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return;
    }
    const area = document.createElement('textarea');
    area.value = text;
    area.setAttribute('readonly', '');
    area.style.cssText = 'position:fixed;opacity:0;pointer-events:none';
    document.body.append(area);
    area.select();
    document.execCommand('copy');
    area.remove();
  }
  document.querySelectorAll('[data-copy-target]').forEach((button) => {
    button.addEventListener('click', async () => {
      const target = document.getElementById(button.dataset.copyTarget);
      if (!target) return;
      const original = button.textContent;
      try {
        await copyText(target.innerText || target.textContent || '');
        button.textContent = 'Copied';
        announce('Code copied to clipboard');
      } catch (_) {
        button.textContent = 'Select';
        announce('Copy unavailable; select the code manually');
      }
      window.setTimeout(() => { button.textContent = original; }, 1600);
    });
  });

  // Accessible, keyboard-friendly example tabs.
  const tabs = [...document.querySelectorAll('.example-tab')];
  const panels = [...document.querySelectorAll('.example-panel')];
  function selectExample(name, focusTab = false) {
    tabs.forEach((tab) => {
      const active = tab.dataset.example === name;
      tab.classList.toggle('is-active', active);
      tab.setAttribute('aria-selected', String(active));
      tab.tabIndex = active ? 0 : -1;
      if (active && focusTab) tab.focus();
    });
    panels.forEach((panel) => {
      const active = panel.dataset.panel === name;
      panel.classList.toggle('is-active', active);
      panel.hidden = !active;
    });
  }
  tabs.forEach((tab, index) => {
    tab.addEventListener('click', () => selectExample(tab.dataset.example));
    tab.addEventListener('keydown', (event) => {
      if (!['ArrowRight', 'ArrowLeft', 'Home', 'End'].includes(event.key)) return;
      event.preventDefault();
      let next = index;
      if (event.key === 'ArrowRight') next = (index + 1) % tabs.length;
      if (event.key === 'ArrowLeft') next = (index - 1 + tabs.length) % tabs.length;
      if (event.key === 'Home') next = 0;
      if (event.key === 'End') next = tabs.length - 1;
      selectExample(tabs[next].dataset.example, true);
    });
  });

  // Subtle reveal only when the browser supports observation and motion is allowed.
  const reducedMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
  if (!reducedMotion && 'IntersectionObserver' in window) {
    const observer = new IntersectionObserver((entries, instance) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        entry.target.classList.add('is-visible');
        instance.unobserve(entry.target);
      });
    }, { threshold: 0.12, rootMargin: '0px 0px -30px' });
    document.querySelectorAll('.reveal').forEach((element) => observer.observe(element));
  } else {
    document.querySelectorAll('.reveal').forEach((element) => element.classList.add('is-visible'));
  }
})();
