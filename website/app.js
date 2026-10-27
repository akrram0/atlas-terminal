/**
 * Atlas Terminal Website Interactive Core
 * Modern Theme Switcher, Terminal Simulator, Magnetic Cursor, and Micro-Interactions
 */

(function () {
  'use strict';

  // 1. Theme Management (Sync with meta color-scheme and localStorage)
  const metaColorScheme = document.querySelector('meta[name="color-scheme"]');
  const themeToggleBtns = document.querySelectorAll('.theme-toggle-btn');

  function getActiveTheme() {
    return localStorage.getItem('color-scheme') || 'system';
  }

  function applyTheme(theme) {
    if (theme === 'dark') {
      document.documentElement.setAttribute('data-theme', 'dark');
      if (metaColorScheme) metaColorScheme.content = 'dark';
    } else if (theme === 'light') {
      document.documentElement.setAttribute('data-theme', 'light');
      if (metaColorScheme) metaColorScheme.content = 'light';
    } else {
      document.documentElement.removeAttribute('data-theme');
      if (metaColorScheme) metaColorScheme.content = 'light dark';
    }
    updateToggleIcons(theme);
  }

  function updateToggleIcons(theme) {
    themeToggleBtns.forEach((btn) => {
      const isDark =
        theme === 'dark' ||
        (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
      btn.innerHTML = isDark
        ? `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>`
        : `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>`;
      btn.setAttribute(
        'aria-label',
        isDark ? 'Switch to Warm Paper theme' : 'Switch to Matte Dark theme'
      );
    });
  }

  function toggleTheme() {
    const current = getActiveTheme();
    const systemIsDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    let nextTheme;

    if (current === 'system') {
      nextTheme = systemIsDark ? 'light' : 'dark';
    } else if (current === 'dark') {
      nextTheme = 'light';
    } else {
      nextTheme = 'dark';
    }

    localStorage.setItem('color-scheme', nextTheme);
    applyTheme(nextTheme);
  }

  themeToggleBtns.forEach((btn) => btn.addEventListener('click', toggleTheme));

  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
    if (getActiveTheme() === 'system') {
      applyTheme('system');
    }
  });

  window.addEventListener('storage', (e) => {
    if (e.key === 'color-scheme') {
      applyTheme(e.newValue || 'system');
    }
  });

  // Apply initial theme
  applyTheme(getActiveTheme());

  // 2. Ambient Cursor Spotlight Torch (Apple / Linear Aesthetic)
  const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  const isTouchDevice = window.matchMedia('(pointer: coarse)').matches;

  if (!isTouchDevice && !prefersReducedMotion) {
    let spotlight = document.getElementById('ambient-spotlight');
    if (!spotlight) {
      spotlight = document.createElement('div');
      spotlight.id = 'ambient-spotlight';
      spotlight.className = 'ambient-spotlight';
      spotlight.setAttribute('aria-hidden', 'true');
      document.body.prepend(spotlight);
    }

    let isVisible = false;
    window.addEventListener('mousemove', (e) => {
      if (!isVisible) {
        isVisible = true;
        spotlight.style.opacity = '1';
      }
      spotlight.style.transform = `translate3d(${e.clientX - 325}px, ${e.clientY - 325}px, 0)`;
    }, { passive: true });

    window.addEventListener('mouseleave', () => {
      isVisible = false;
      spotlight.style.opacity = '0';
    });

    window.addEventListener('mouseenter', () => {
      isVisible = true;
      spotlight.style.opacity = '1';
    });

    // Dynamic Spotlight on Cards
    const spotlightCards = document.querySelectorAll('.feature-card, .asset-card, .terminal-window, .release-card');
    spotlightCards.forEach((card) => {
      card.addEventListener('mousemove', (e) => {
        const rect = card.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        card.style.setProperty('--mouse-x', `${x}px`);
        card.style.setProperty('--mouse-y', `${y}px`);
      }, { passive: true });
    });

    // Subtle 3D Perspective Tilt on Hero App Icon
    const appIcon = document.querySelector('.app-icon-container');
    if (appIcon) {
      appIcon.addEventListener('mousemove', (e) => {
        const rect = appIcon.getBoundingClientRect();
        const x = (e.clientX - rect.left) / rect.width - 0.5;
        const y = (e.clientY - rect.top) / rect.height - 0.5;
        const tiltX = -y * 10;
        const tiltY = x * 10;
        appIcon.style.transform = `perspective(600px) rotateX(${tiltX}deg) rotateY(${tiltY}deg) scale3d(1.03, 1.03, 1.03)`;
      }, { passive: true });

      appIcon.addEventListener('mouseleave', () => {
        appIcon.style.transform = 'perspective(600px) rotateX(0deg) rotateY(0deg) scale3d(1, 1, 1)';
      });
    }
  }

  // 5. Interactive Terminal Sandbox Simulator
  const termBody = document.getElementById('terminal-body');
  const termInput = document.getElementById('term-input');
  const termTabs = document.querySelectorAll('.terminal-tab');

  const fastfetchHTML = `
    <div class="fastfetch-block">
      <img src="assets/app-icon.png" alt="Atlas Fastfetch Graphic" class="fastfetch-logo" />
      <div class="fastfetch-specs">
        <div class="spec-line"><span class="spec-key">User</span><span class="spec-val">developer@atlas</span></div>
        <div class="spec-line"><span class="spec-key">OS</span><span class="spec-val">Windows 11 Pro 64-bit</span></div>
        <div class="spec-line"><span class="spec-key">Host</span><span class="spec-val">DirectX 12 (DWM Acrylic Blur)</span></div>
        <div class="spec-line"><span class="spec-key">Terminal</span><span class="spec-val">Atlas Terminal v0.1.74 (GPUI Engine)</span></div>
        <div class="spec-line"><span class="spec-key">Graphics</span><span class="spec-val">iTerm2 Inline Images (OSC 1337)</span></div>
        <div class="spec-line"><span class="spec-key">Refresh</span><span class="spec-val">240Hz Adaptive Frame Scheduler</span></div>
        <div class="spec-line"><span class="spec-key">CPU Usage</span><span class="spec-val">0.0% (Quiescent Idle)</span></div>
      </div>
    </div>
  `;

  // Realistic Shell Environments for Tabs
  const shellEnvironments = {
    powershell: `
      <div class="fastfetch-block">
        <img src="assets/app-icon.png" alt="Atlas Fastfetch Graphic" class="fastfetch-logo" />
        <div class="fastfetch-specs">
          <div class="spec-line"><span class="spec-key">User</span><span class="spec-val">developer@atlas</span></div>
          <div class="spec-line"><span class="spec-key">OS</span><span class="spec-val">Windows 11 Pro 64-bit</span></div>
          <div class="spec-line"><span class="spec-key">Host</span><span class="spec-val">DirectX 12 (DWM Acrylic Blur)</span></div>
          <div class="spec-line"><span class="spec-key">Terminal</span><span class="spec-val">Atlas Terminal v0.1.74 (GPUI Engine)</span></div>
          <div class="spec-line"><span class="spec-key">Graphics</span><span class="spec-val">iTerm2 Inline Images (OSC 1337)</span></div>
          <div class="spec-line"><span class="spec-key">Refresh</span><span class="spec-val">240Hz Adaptive Frame Scheduler</span></div>
          <div class="spec-line"><span class="spec-key">CPU Usage</span><span class="spec-val">0.0% (Quiescent Idle)</span></div>
        </div>
      </div>
    `,
    wsl: `
      <div style="color:var(--terminal-dim);font-size:12px;margin-bottom:8px;">[wsl2: Arch Linux 6.6.137-microsoft-standard-WSL2 · Native Vector Arch Logo]</div>
      <div class="term-cmd-row">
        <span style="color:#00e5ff;font-weight:600;">archlinux</span><span style="color:rgba(255,255,255,0.4);">:</span><span style="color:#10b981;font-weight:600;">~</span> <span style="color:#ffffff;">❯ uname -r -m</span>
      </div>
      <div class="term-out-row" style="color:var(--terminal-dim);">
        6.6.137-microsoft-standard-WSL2 x86_64
      </div>
      <div class="term-cmd-row" style="margin-top:8px;">
        <span style="color:#00e5ff;font-weight:600;">archlinux</span><span style="color:rgba(255,255,255,0.4);">:</span><span style="color:#10b981;font-weight:600;">~</span> <span style="color:#ffffff;">❯ atlas --version</span>
      </div>
      <div class="term-out-row">
        <div>atlas-terminal <span style="color:var(--accent);">v0.1.74</span> (Windows Subsystem for Linux bridged socket)</div>
        <div style="color:var(--terminal-dim);font-size:11px;margin-top:2px;">TrueColor 24-bit + OSC 1337 terminal graphics verified.</div>
      </div>
    `,
    opencode: `
      <div style="background:rgba(255,255,255,0.04);border:1px solid rgba(255,255,255,0.1);border-radius:6px;padding:10px 14px;margin-bottom:12px;">
        <div style="display:flex;align-items:center;gap:8px;margin-bottom:4px;">
          <span style="display:inline-block;width:8px;height:8px;border-radius:50%;background:#10b981;box-shadow:0 0 8px #10b981;"></span>
          <span style="font-weight:700;color:#ffffff;font-size:12px;letter-spacing:0.04em;">OPENCODE AI AGENT · CLAUDE 3.7 SONNET</span>
        </div>
        <div style="color:var(--terminal-dim);font-size:12px;line-height:1.5;">
          Direct acrylic buffer active · No background halo on custom theme switch · Full anti-leak geometry verified.
        </div>
      </div>
      <div class="term-cmd-row">
        <span style="color:#f59e0b;font-weight:600;">opencode</span> ❯ <span style="color:#ffffff;">opencode run build --release</span>
      </div>
      <div class="term-out-row">
        <div style="color:#10b981;">✓ Finished release [optimized] target(s) in 0.84s</div>
        <div style="color:var(--terminal-dim);font-size:11px;margin-top:2px;">Ready for interactive developer commands. Type 'fastfetch' or 'theme' below.</div>
      </div>
    `
  };

  const commandResponses = {
    help: `
      <div>Available commands:</div>
      <div style="padding-left: 12px; margin-top: 4px;">
        <div><strong style="color:var(--accent);">fastfetch</strong> - Display system specs & inline graphic</div>
        <div><strong style="color:var(--accent);">theme</strong>     - Toggle between Matte Dark & Warm Paper</div>
        <div><strong style="color:var(--accent);">version</strong>   - Print Atlas Terminal release specs</div>
        <div><strong style="color:var(--accent);">clear</strong>     - Clear the terminal history</div>
      </div>
    `,
    version: `
      <div>Atlas Terminal <span style="color:var(--accent);">v0.1.74</span> (Rust 2024 / Zed GPUI DirectX)</div>
      <div style="color:rgba(255,255,255,0.6);font-size:11px;">Features: 240Hz frame scheduling, dynamic WSL detection, background acrylic blur, iTerm2 inline graphics</div>
    `,
  };

  function executeTermCommand(cmdText) {
    const raw = cmdText.trim();
    if (!raw || !termBody) return;

    const lower = raw.toLowerCase();

    // Create command log
    const cmdRow = document.createElement('div');
    cmdRow.className = 'term-cmd-row';
    cmdRow.innerHTML = `<span class="term-prompt">❯</span> <span class="term-cmd">${escapeHTML(raw)}</span>`;

    const outRow = document.createElement('div');
    outRow.className = 'term-out-row';

    if (lower === 'clear') {
      const outputArea = document.getElementById('terminal-history');
      if (outputArea) outputArea.innerHTML = '';
      if (termInput) termInput.value = '';
      return;
    } else if (lower === 'fastfetch' || lower === 'neofetch') {
      outRow.innerHTML = fastfetchHTML;
    } else if (lower === 'theme') {
      toggleTheme();
      outRow.innerHTML = `<div>Theme changed. Active: <span style="color:var(--accent);">${getActiveTheme()}</span></div>`;
    } else if (commandResponses[lower]) {
      outRow.innerHTML = commandResponses[lower];
    } else {
      outRow.innerHTML = `<div style="color:#ef4444;">Command not found: ${escapeHTML(raw)}. Try 'help' or 'fastfetch'.</div>`;
    }

    const outputArea = document.getElementById('terminal-history');
    if (outputArea) {
      outputArea.appendChild(cmdRow);
      outputArea.appendChild(outRow);
      termBody.scrollTop = termBody.scrollHeight;
    }

    if (termInput) termInput.value = '';
  }

  // Handle Tab Switch in Mockup with Authentic Shell Views
  termTabs.forEach((tab) => {
    tab.addEventListener('click', function () {
      termTabs.forEach((t) => t.classList.remove('active'));
      this.classList.add('active');
      const tabName = this.dataset.tab;
      const history = document.getElementById('terminal-history');
      if (history && shellEnvironments[tabName]) {
        history.innerHTML = shellEnvironments[tabName];
      }
    });
  });

  // Handle Quick Command Pills with Tactile Typing Simulation
  let isTyping = false;
  document.querySelectorAll('.term-pill').forEach((pill) => {
    pill.addEventListener('click', function () {
      if (isTyping || !termInput) return;
      const cmd = this.dataset.cmd;
      if (!cmd) return;

      termInput.focus();
      termInput.value = '';
      isTyping = true;
      let charIdx = 0;

      const typeInterval = setInterval(() => {
        if (charIdx < cmd.length) {
          termInput.value += cmd[charIdx];
          charIdx++;
        } else {
          clearInterval(typeInterval);
          setTimeout(() => {
            executeTermCommand(cmd);
            isTyping = false;
          }, 120);
        }
      }, 35);
    });
  });

  if (termInput) {
    termInput.addEventListener('keydown', function (e) {
      if (e.key === 'Enter') {
        executeTermCommand(this.value);
      }
    });
  }

  // 6. Tagline Word-by-Word Scroll Reveal (Part B11)
  const taglineEl = document.querySelector('.reveal-text');
  if (taglineEl) {
    const rawText = taglineEl.textContent.trim();
    const words = rawText.split(/\s+/);
    taglineEl.innerHTML = words
      .map((w) => `<span class="reveal-word">${escapeHTML(w)}</span>`)
      .join(' ');

    const wordSpans = taglineEl.querySelectorAll('.reveal-word');

    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            wordSpans.forEach((span, i) => {
              setTimeout(() => {
                span.classList.add('active');
              }, i * 65);
            });
            observer.unobserve(taglineEl);
          }
        });
      },
      { threshold: 0.25 }
    );

    observer.observe(taglineEl);
  }

  // 7. FAQ Accordion
  document.querySelectorAll('.faq-question').forEach((btn) => {
    btn.addEventListener('click', function () {
      const item = this.closest('.faq-item');
      if (item) {
        item.classList.toggle('open');
      }
    });
  });

  // 8. One-click Copy Feedback
  document.querySelectorAll('[data-copy]').forEach((el) => {
    el.addEventListener('click', function () {
      const textToCopy = this.getAttribute('data-copy');
      if (textToCopy) {
        navigator.clipboard.writeText(textToCopy).then(() => {
          const originalHTML = this.innerHTML;
          this.style.pointerEvents = 'none';
          this.innerHTML = `<span style="color:#2f7d32;font-size:11px;font-weight:600;">✓ Copied!</span>`;
          setTimeout(() => {
            this.innerHTML = originalHTML;
            this.style.pointerEvents = '';
          }, 1800);
        });
      }
    });
  });

  // 9. Docs Search Filter
  const docsSearchInput = document.getElementById('docs-search-input');
  if (docsSearchInput) {
    docsSearchInput.addEventListener('input', function () {
      const query = this.value.toLowerCase().trim();
      const articles = document.querySelectorAll('.docs-article');
      articles.forEach((art) => {
        const text = art.textContent.toLowerCase();
        art.style.display = text.includes(query) ? '' : 'none';
      });
    });
  }

  function escapeHTML(str) {
    return str
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#039;');
  }
})();
