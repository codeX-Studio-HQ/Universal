<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount, tick } from 'svelte';
  import en from '../locales/en.json';
  import tr from '../locales/tr.json';
  import de from '../locales/de.json';

  const LOCALES = { en, tr, de };
  const LANG_NAMES = { en: 'English', tr: 'Türkçe', de: 'Deutsch' };

  let currentLang = 'en';
  let t = LOCALES[currentLang];

  function setLang(key) {
    currentLang = key;
    t = LOCALES[key];
    localStorage.setItem('launcher-lang', key);
    updatePlaceholders();
  }

  const THEMES = {
  blue:   { key: 'blue',   accent: '#3d7fff', accentRgb: '61,127,255',  bg1: '#0e1628', bg2: '#080d1a', glow: '30,80,200' },
  violet: { key: 'violet', accent: '#8b5cf6', accentRgb: '139,92,246',  bg1: '#120e28', bg2: '#0a0814', glow: '90,40,180' },
  emerald:{ key: 'emerald',accent: '#10b981', accentRgb: '16,185,129',  bg1: '#0a1e18', bg2: '#060f0d', glow: '10,120,80'  },
  rose:   { key: 'rose',   accent: '#e5383b', accentRgb: '229,56,59',   bg1: '#1e0e0e', bg2: '#120808', glow: '180,30,30' },
  amber:  { key: 'amber',  accent: '#f59e0b', accentRgb: '245,158,11',  bg1: '#1a1408', bg2: '#0f0c04', glow: '160,100,10' },
  midnight:{ key: 'midnight', accent: '#1e3a5f', accentRgb: '30,58,95',  bg1: '#060d1a', bg2: '#020610', glow: '20,40,80' },
  neon:   { key: 'neon',   accent: '#ec4899', accentRgb: '236,72,153',  bg1: '#1a0814', bg2: '#0f040a', glow: '200,50,130' },
  arctic: { key: 'arctic', accent: '#38bdf8', accentRgb: '56,189,248',  bg1: '#081820', bg2: '#040c10', glow: '30,150,200' },
};

  let currentTheme = 'blue';
  let theme = THEMES[currentTheme];

  function setTheme(key) {
    currentTheme = key;
    theme = THEMES[key];
    applyThemeVars(theme);
    localStorage.setItem('launcher-theme', key);
  }

  function applyThemeVars(th) {
    const r = document.documentElement;
    r.style.setProperty('--accent', th.accent);
    r.style.setProperty('--accent-rgb', th.accentRgb);
    r.style.setProperty('--bg1', th.bg1);
    r.style.setProperty('--bg2', th.bg2);
    r.style.setProperty('--glow-rgb', th.glow);
  }

  let query = '';
  let results = [];
  let selectedIndex = 0;
  let loading = false;
  let inputEl;
  let showSettings = false;
  let showSysInfo = false;
  let showEmoji = false;
let copiedText = '';
let showCopied = false;
  let systemInfo = null;
  let recentFiles = [];
  let bodyVisible = false;

  let placeholders = [];
  let placeholderIndex = 0;
  let placeholderText = '';
  let placeholderVisible = true;

  function updatePlaceholders() {
    placeholders = [
      t.search.placeholder,
      t.search.placeholder2,
      t.search.placeholder3,
      t.search.placeholder4,
    ];
    placeholderText = placeholders[placeholderIndex];
  }

  $: groupedResults = groupByType(results);
  $: flatResults = results;
  $: selectedItem = results[selectedIndex] || null;

  function groupByType(items) {
    const g = {};
    for (const item of items) {
      if (!g[item.result_type]) g[item.result_type] = [];
      g[item.result_type].push(item);
    }
    return g;
  }

  const sl = type => t.sections[type] || type;
  const al = item => t.actions[item?.result_type] || t.actions.open || 'Open';
  const tl = type => t.types[type] || type;

  onMount(async () => {
    const savedLang = localStorage.getItem('launcher-lang');
    if (savedLang && LOCALES[savedLang]) setLang(savedLang);
    else updatePlaceholders();

    const savedTheme = localStorage.getItem('launcher-theme');
    if (savedTheme && THEMES[savedTheme]) setTheme(savedTheme);
    else applyThemeVars(theme);

    inputEl?.focus();
    await tick();
    setTimeout(() => bodyVisible = true, 30);
    loadAutostart();
    loadShortcut();

    try {
  const stats = await invoke('get_index_stats');
  totalApps = stats.apps;
  totalFiles = stats.files;
} catch {}

    try { recentFiles = await invoke('get_recent_files'); }
    catch { recentFiles = []; }

    const iv = setInterval(async () => {
      placeholderVisible = false;
      await new Promise(r => setTimeout(r, 200));
      placeholderIndex = (placeholderIndex + 1) % placeholders.length;
      placeholderText = placeholders[placeholderIndex];
      placeholderVisible = true;
    }, 3000);
    return () => clearInterval(iv);
  });

  async function handleInput() {
    if (!query.trim()) { results = []; return; }
    loading = true;
    try { results = await invoke('search', { query: query.trim() }); selectedIndex = 0; }
    catch (e) { console.error(e); }
    loading = false;
  }


  async function selectResult(item) {
    try {
      if (item.result_type === 'app') await invoke('open_app', { path: item.path });
      else if (item.result_type === 'file') await invoke('open_file', { path: item.path });
      else if (item.result_type === 'web_search') await invoke('web_search', { query: item.path });
      else if (item.result_type === 'system') await invoke('system_command', { cmd: item.path });
      else if (item.result_type === 'recent') { query = item.name; await handleInput(); return; }
      else if (item.result_type === 'youtube_search') await invoke('youtube_search', { query: item.path });
      else if (item.result_type === 'emoji') await invoke('copy_to_clipboard', { text: item.path });
      else if (item.result_type === 'command') {
        const output = await invoke('execute_command', { cmd: item.path });
        alert(output);
      }
      reset();
    } catch (e) { console.error(e); }
  }

  function reset() {
    query = ''; results = []; selectedIndex = 0; showSettings = false; showSysInfo = false; showEmoji = false;
    inputEl?.focus();
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') { reset(); return; }
    if (showSettings || showSysInfo) return;
    if (e.key === 'ArrowDown') { e.preventDefault(); selectedIndex = Math.min(selectedIndex + 1, results.length - 1); scrollToSelected(); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); selectedIndex = Math.max(selectedIndex - 1, 0); scrollToSelected(); }
    else if (e.key === 'Enter') { if (results[selectedIndex]) selectResult(results[selectedIndex]); }
  }

  function scrollToSelected() {
    const el = document.querySelector('.result-item.selected');
    if (el) { el.scrollIntoView({ block: 'nearest', behavior: 'instant' }); }
  }

  $: if (query === '') { results = []; selectedIndex = 0; }

  let totalApps = 0;
let totalFiles = 0;

  let currentShortcut = 'ctrl+space';

async function loadShortcut() {
  try { currentShortcut = await invoke('get_shortcut'); }
  catch { currentShortcut = 'ctrl+space'; }
}

async function changeShortcut(key) {
  if (key === currentShortcut) return;
  currentShortcut = key;
  await invoke('set_shortcut', { shortcut: key });
  alert('Kısayol kaydedildi. Değişikliğin etkili olması için uygulamayı yeniden başlatın.');
}

  let autostart = true;

  async function loadAutostart() {
    try { autostart = await invoke('get_autostart'); }
    catch { autostart = false; }
  }

  async function toggleAutostart() {
    autostart = !autostart;
    try { await invoke('set_autostart', { enable: autostart }); }
    catch { autostart = !autostart; }
  }

  let gearSpinning = false;
  function toggleSettings() {
    gearSpinning = true;
    setTimeout(() => gearSpinning = false, 500);
    showSettings = !showSettings;
    showSysInfo = false;
    showEmoji = false; 
  }

  async function toggleSysInfo() {
    showSettings = false;
    showEmoji = false;
    showSysInfo = !showSysInfo;
    if (showSysInfo) {
      try { systemInfo = await invoke('get_system_info'); }
      catch { systemInfo = 'Error loading system info'; }
    }
  }

  function toggleEmoji() {
  showSettings = false;
  showSysInfo = false;
  showEmoji = !showEmoji;
}

$: EMOJI_CATEGORIES = [
    {
      title: t.common.emojiCategories?.faces || "Yüz İfadeleri",
      emojis: ['😀','😃','😄','😁','😅','😂','🤣','😊','😇','🙂','😉','😌','😍','🥰','😘','😗','😋','😛','😜','🤪','😝','🤑','🤗','🤭','🤫','🤔','🤐','🤨','😐','😑','😶','😏','😒','🙄','😬','😮‍💨','🤥','😔','😪','🤤','😴','😷','🤒','🤕','🤢','🤮','🤧','🥵','🥶','🥴','😵','🤯','🤠','🥳','🥸','😎','🤓','🧐','😕','😟','🙁','😮','😯','😲','😳','🥺','😦','😧','😨','😰','😥','😢','😭','😱','😖','😣','😞','😓','😩','😫','🥱','😤','😡','😠','🤬','😈','👿','💀','☠️','💩','🤡','👹','👺','👻','👽','👾','🤖']
    },
    {
      title: t.common.emojiCategories?.hands || "Eller & Vücut",
      emojis: ['👋','🤚','🖐️','✋','🖖','👌','🤌','🤏','✌️','🤞','🤟','🤘','🤙','👈','👉','👆','🖕','👇','☝️','👍','👎','✊','👊','🤛','🤜','👏','🙌','👐','🤲','🤝','🙏','✍️','💅','🤳','💪','🦵','🦶','👂','🦻','👃']
    },
    {
      title: t.common.emojiCategories?.hearts || "Kalpler & Duygular",
      emojis: ['💋','💌','💘','💝','💖','💗','💓','💞','💕','💟','❣️','💔','❤️','🧡','💛','💚','💙','💜','🤎','🖤','🤍','💯','💢','💥','💫','💦','💨','💣','💬','👁️‍🗨️','🗨️','🗯️','💭','💤']
    },
    {
      title: t.common.emojiCategories?.people || "İnsanlar & Aile",
      emojis: ['👶','🧒','👦','👧','🧑','👱','👨','🧔','👩','🧓','👴','👵','🙍','🙎','🙅','🙆','💁','💆','💇','🧖','🧘','🛀','🛌','👭','👫','👬','💏','👩‍❤️‍💋‍👨','👨‍👩‍👦','👨‍👩‍👧','👨‍👩‍👧‍👦','👩‍👩‍👦','👨‍👨‍👦']
    },
    {
      title: t.common.emojiCategories?.animals || "Hayvanlar",
      emojis: ['🐶','🐱','🐭','🐹','🐰','🦊','🐻','🐼','🐻‍❄️','🐨','🐯','🦁','🐮','🐷','🐸','🐵','🙈','🙉','🙊','🐒','🐔','🐧','🐦','🐤','🐣','🐥','🦆','🦅','🦉','🦇','🐺','🐗','🐴','🦄','🐝','🪱','🐛','🦋','🐌','🐞','🐜','🪰','🪲','🪳','🦟','🦗','🕷️','🦂','🐢','🐍','🦎','🦖','🦕','🐙','🦑','🦐','🦞','🐠','🐟','🐡','🦈','🐳','🐋','🐬','🦭','🐊','🐅','🐆','🦓','🦍','🦧','🐘','🦛','🦏','🐪','🐫','🦒','🦘','🐕','🐩','🦮','🐈','🐈‍⬛','🐓','🦃','🦤','🦚','🦜','🦢','🦩','🕊️','🐇','🦝','🦨','🦡','🦫','🦦','🦥']
    },
    {
      title: t.common.emojiCategories?.food || "Yiyecek & İçecek",
      emojis: ['🍏','🍎','🍐','🍊','🍋','🍌','🍉','🍇','🍓','🫐','🍈','🍒','🍑','🥭','🍍','🥥','🥝','🍅','🍆','🥑','🥦','🥬','🥒','🌶️','🫑','🌽','🥕','🫒','🧄','🧅','🥔','🍠','🥐','🍞','🥖','🥨','🧀','🥚','🍳','🧈','🥞','🧇','🥓','🥩','🍗','🍖','🌭','🍔','🍟','🍕','🫓','🥪','🥙','🧆','🌮','🌯','🥗','🥘','🫕','🍝','🍜','🍲','🍛','🍣','🍱','🥟','🦪','🍤','🍙','🍚','🍘','🍥','🥠','🥮','🍢','🍡','🍧','🍨','🍦','🥧','🧁','🍰','🎂','🍮','🍭','🍬','🍫','🍿','🍩','🍪','🌰','🥜','🍯','🥛','🍼','☕','🍵','🧃','🥤','🧋','🍶','🍺','🍻','🥂','🍷','🥃','🍸','🍹','🧉']
    },
    {
      title: t.common.emojiCategories?.travel || "Seyahat & Ulaşım",
      emojis: ['🚗','🚕','🚙','🚌','🚎','🏎️','🚓','🚑','🚒','🚐','🛻','🚚','🚛','🏍️','🛵','🚲','🛴','🛹','✈️','🛩️','🚀','🛰️','🚁','⛵','🚤','🛳️','🚢','🚂','🚄','🚅','🚇','🚉','🛤️','🛣️','⛽','🚨']
    },
    {
      title: t.common.emojiCategories?.sports || "Spor & Aktiviteler",
      emojis: ['⚽','🏀','🏈','⚾','🥎','🎾','🏐','🏉','🎱','🏓','🏸','🥍','🏒','🏑','⛳','🏹','🎣','🥊','🥋','🎽','🏋️','🤼','🤸','🤺','⛹️','🤾','🏌️','🏄','🏊','🚣','🧗','🚵','🚴','🏆','🥇','🥈','🥉']
    },
    {
      title: t.common.emojiCategories?.objects || "Nesneler & Semboller",
      emojis: ['📱','💻','⌨️','🖥️','🖨️','📷','🎥','📺','📻','🎙️','🎧','🎤','📢','📣','🕯️','💡','🔦','🏠','🏡','🏢','🏣','🏥','🏦','🏨','🏪','🏫','⛪','🕌','🕍','🛕','⛩️','🗿','🏰','🎪','🎡','🎢','🎠','📚','📖','🖋️','🖊️','📝','📌','📍','🔍','🔎','🔬','🔭','🧪','🧫','🧬','💊','🩹','🩺','🔑','🔒','🔓','🛠️','🔨','⚒️','🛡️','⚔️','🔫','🏹','💣','🧨','🎇','🎆','✨','⭐','🌟','🌍','🌎','🌏']
    },
    {
      title: t.common.emojiCategories?.nature || "Doğa & Hava",
      emojis: ['🌵','🎄','🌲','🌳','🌴','🌱','🌿','☘️','🍀','🌸','🌺','🌻','🌼','🌷','🥀','🌹','🍁','🍂','🍃','🌞','🌝','🌛','🌜','🌚','🌕','🌖','🌗','🌘','🌑','🌒','🌓','🌔','🌙','🌧️','🌨️','🌩️','🌪️','🌈','☀️','⛅','☁️','❄️','☃️','🌬️','💧','🌊']
    }
  ];

async function copyEmoji(emoji) {
  try {
    await invoke('copy_to_clipboard', { text: emoji });
    copiedText = `${emoji} ${t.common.copied || 'Kopyalandı!'}`;
    showCopied = true;
    setTimeout(() => {
      showCopied = false;
    }, 1400);
  } catch (e) {
    console.error(e);
  }
}
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="root">
  <div class="launcher" class:visible={bodyVisible}>

    <div class="search-bar">
      <button class="icon-btn back-btn" on:click={reset} title="Back">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="15 18 9 12 15 6"/>
        </svg>
      </button>

      <div class="input-wrap">
        <input type="text" bind:value={query} bind:this={inputEl} on:input={handleInput} placeholder="" autofocus spellcheck="false" />
        {#if !query}
          <span class="placeholder" class:fade={!placeholderVisible}>{placeholderText}</span>
        {/if}
      </div>

      {#if loading}
        <div class="spinner"></div>
      {:else if query !== ''}
        <button class="icon-btn" on:click={reset} title="Clear" style="animation: fade-in 0.15s ease">
          <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
            <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
          </svg>
        </button>
      {/if}

      <button class="icon-btn emoji-btn" class:active={showEmoji} on:click={toggleEmoji} title="{t.common.emojis}">😊</button>
      <button class="icon-btn sys-btn" class:active={showSysInfo} on:click={toggleSysInfo} title="{t.common.systemInfo}">📊</button>

      <button class="icon-btn settings-btn" class:active={showSettings} class:spin={gearSpinning} on:click={toggleSettings} title={t.settings.title}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="3"/>
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
        </svg>
      </button>
      
    </div>

    <div class="body">

      {#if showSettings}
        <div class="settings-panel">
          <div class="settings-inner">

            <div class="settings-section-title">{t.settings.appearance}</div>
            <div class="theme-label">{t.settings.colorTheme}</div>
            <div class="theme-grid">
              {#each Object.entries(THEMES) as [key, th], i}
                <button class="theme-swatch"
                        class:active={currentTheme === key}
                        style="--sw: {th.accent}; animation-delay: {i * 40}ms"
                        on:click={() => setTheme(key)}
                        title={t.themes[key]}>
                  <span class="swatch-dot" style="background: {th.accent}; box-shadow: 0 0 10px {th.accent}88;"></span>
                  <span class="swatch-name">{t.themes[key]}</span>
                  {#if currentTheme === key}<span class="swatch-check">✓</span>{/if}
                </button>
              {/each}
            </div>

            <div class="settings-divider"></div>
            <div class="settings-section-title">{t.settings.language}</div>
            <div class="theme-grid">
              {#each Object.entries(LOCALES) as [key, loc], i}
                <button class="theme-swatch" class:active={currentLang === key} style="--sw: var(--accent); animation-delay: {i * 40}ms" on:click={() => setLang(key)}>
                  <span class="flag-emoji">{key === 'en' ? '🇬🇧' : key === 'tr' ? '🇹🇷' : '🇩🇪'}</span>
                  <span class="swatch-name">{LANG_NAMES[key]}</span>
                  {#if currentLang === key}<span class="swatch-check">✓</span>{/if}
                </button>
              {/each}
            </div>

            <div class="settings-divider"></div>
            <div class="settings-section-title">Shortcut</div>
            <div class="theme-grid">
              {#each [
                {key:'alt+space', label:'Alt + Space'},
                {key:'ctrl+space', label:'Ctrl + Space'},
                {key:'alt+l', label:'Alt + L'},
                {key:'alt+p', label:'Alt + P'}
              ] as opt}
                <button class="theme-swatch" class:active={currentShortcut === opt.key}
                  style="--sw: var(--accent)" on:click={() => changeShortcut(opt.key)}>
                  <span class="swatch-dot" style="background:var(--accent);box-shadow:0 0 10px var(--accent)88"></span>
                  <span class="swatch-name">{opt.label}</span>
                  {#if currentShortcut === opt.key}<span class="swatch-check">✓</span>{/if}
                </button>
              {/each}
            </div>

            <div class="settings-divider"></div>
            <div class="settings-section-title">{t.settings.startup}</div>
            <label class="toggle-row">
              <span class="toggle-label">{t.settings.startWithSystem}</span>
              <div class="toggle-switch" class:on={autostart} on:click={toggleAutostart} role="switch" tabindex="0" aria-checked={autostart}>
                <div class="toggle-thumb"></div>
              </div>
            </label>

            <div class="settings-divider"></div>
            <div class="settings-section-title">{t.settings.about}</div>
            <div class="about-row"><span class="about-key">{t.settings.version}</span><span class="about-val">Beta v1.2</span></div>
            <div class="about-row"><span class="about-key">{t.settings.developer}</span><span class="about-val">Wrenchiz</span></div>
            <div class="about-row"><span class="about-key">{t.settings.engine}</span><span class="about-val">Tauri v2 + Svelte</span></div>

            <div class="made-with">
              <span>{t.settings.madeWith}</span>
              <span>{t.settings.by}</span>
              <span class="studio-link" on:click={() => invoke('open_file', { path: 'https://codex-studio-hq.netlify.app/' })} role="link" tabindex="0" on:keydown={(e) => e.key === 'Enter' && invoke('open_file', { path: 'https://codex-studio-hq.netlify.app/' })}>codeX Studio</span>
            </div>

          </div>
        </div>

      {:else if showSysInfo}
        <div class="settings-panel">
          <div class="settings-inner">
            <div class="settings-section-title">{t.common.systemInfo}</div>

            {#if systemInfo}
              <div class="info-card">
                <div class="info-card-icon">🖥️</div>
                <div class="info-card-content">
                  <div class="info-card-title">{t.systemInfo.os}</div>
                  <div class="info-card-sub">{systemInfo.os || 'Bilinmiyor'}</div>
                  <div class="info-card-badge">Kernel: {systemInfo.kernel || '-'}</div>
                </div>
              </div>

              <div class="info-card">
                <div class="info-card-icon">🏷️</div>
                <div class="info-card-content">
                  <div class="info-card-title">{t.systemInfo.hostname}</div>
                  <div class="info-card-sub">{systemInfo.hostname || 'Bilinmiyor'}</div>
                </div>
              </div>

              <div class="info-card">
                <div class="info-card-icon">⏱️</div>
                <div class="info-card-content">
                  <div class="info-card-title">{t.systemInfo.uptime}</div>
                  <div class="info-card-sub">{systemInfo.uptime || '—'}</div>
                </div>
              </div>

              <div class="info-card">
                <div class="info-card-icon">📈</div>
                <div class="info-card-content">
                  <div class="info-card-title">{t.systemInfo.loadavg}</div>
                  <div class="info-card-sub">{systemInfo.loadavg || '—'}</div>
                </div>
              </div>

              <div class="info-card">
                <div class="info-card-icon">⚡</div>
                <div class="info-card-content">
                  <div class="info-card-title">{t.systemInfo.cpu}</div>
                  <div class="info-card-sub">{systemInfo.cpu?.name || 'Bilinmiyor'}</div>
                  <div class="info-card-badge">{systemInfo.cpu?.cores || 0} {t.systemInfo.cores}</div>
                </div>
              </div>

              <div class="info-card">
                <div class="info-card-icon">🧠</div>
                <div class="info-card-content">
                  <div class="info-card-title">{t.systemInfo.ram}</div>
                  <div class="info-card-sub">{systemInfo.ram?.available || '?'} / {systemInfo.ram?.total || '?'}</div>
                  <div class="progress-bar">
                    <div class="progress-fill" style="width: {systemInfo.ram?.percent || 0}%; background: {(systemInfo.ram?.percent || 0) > 80 ? '#e5383b' : 'var(--accent)'}"></div>
                  </div>
                  <span class="progress-text">{systemInfo.ram?.percent || 0}% {t.systemInfo.percent_used}</span>
                </div>
              </div>

              <div class="info-card">
                <div class="info-card-icon">🔄</div>
                <div class="info-card-content">
                  <div class="info-card-title">{t.systemInfo.swap}</div>
                  <div class="info-card-sub">{systemInfo.swap?.free || '?'} / {systemInfo.swap?.total || '?'}</div>
                  <div class="progress-bar">
                    <div class="progress-fill" style="width: {systemInfo.swap?.percent || 0}%; background: {(systemInfo.swap?.percent || 0) > 70 ? '#e5383b' : 'var(--accent)'}"></div>
                  </div>
                  <span class="progress-text">{systemInfo.swap?.percent || 0}% {t.systemInfo.percent_used}</span>
                </div>
              </div>

              <div class="info-card">
                <div class="info-card-icon">💾</div>
                <div class="info-card-content">
                  <div class="info-card-title">{t.systemInfo.disk}</div>
                  <div class="info-card-sub">{systemInfo.disk?.free || '?'} / {systemInfo.disk?.total || '?'}</div>
                  <div class="progress-bar">
                    <div class="progress-fill" style="width: {systemInfo.disk?.percent || 0}%; background: {(systemInfo.disk?.percent || 0) > 85 ? '#e5383b' : 'var(--accent)'}"></div>
                  </div>
                  <span class="progress-text">{systemInfo.disk?.percent || 0}% {t.systemInfo.percent_full}</span>
                </div>
              </div>

              <div class="info-card">
                <div class="info-card-icon">🌡️</div>
                <div class="info-card-content">
                  <div class="info-card-title">{t.systemInfo.temperature}</div>
                  <div class="info-card-sub">{systemInfo.temperature || '—'}</div>
                </div>
              </div>

            {:else}
              <div class="sys-loading">{t.systemInfo.loading}</div>
            {/if}
          </div>
        </div>

      {:else if showEmoji}
        <div class="settings-panel">
          <div class="settings-inner">
            <div class="settings-section-title">{t.common.emojis}</div>

            <div class="emoji-container">
              {#each EMOJI_CATEGORIES as category}
                <div class="emoji-category">
                  <div class="category-title">{category.title}</div>
                  <div class="emoji-grid">
                    {#each category.emojis as emoji}
                      <button class="emoji-item" on:click={() => copyEmoji(emoji)} title={emoji}>
                        {emoji}
                      </button>
                    {/each}
                  </div>
                </div>
              {/each}
            </div>

            {#if showCopied}
              <div class="copied-toast">{copiedText}</div>
            {/if}
          </div>
        </div>

      {:else if results.length > 0}
        <div class="split">
          <div class="results-list">
            {#each Object.entries(groupedResults) as [type, items]}
              <div class="section-label">{sl(type)}</div>
              {#each items as item, rowI}
                {@const fi = flatResults.indexOf(item)}
                <div class="result-item" class:selected={fi === selectedIndex} role="button" tabindex="0" style="animation-delay: {rowI * 25}ms" on:click={() => selectResult(item)} on:mouseenter={() => selectedIndex = fi}>
                  <span class="item-icon">{item.icon}</span>
                  <div class="item-info">
                    {#if item.result_type === 'web_search'}
                      <span class="item-name">{t.actions.web_search}: "{query}"</span>
                      <span class="item-sub">{t.actions.open_in_browser}</span>
                    {:else if item.result_type === 'youtube_search'}
                      <span class="item-name">{t.actions.youtube_search}: "{query}"</span>
                      <span class="item-sub">{t.actions.open_in_browser}</span>
                    {:else if item.result_type === 'command'}
                      <span class="item-name">{t.actions.execute}: "{item.name.replace('Run: ', '')}"</span>
                      <span class="item-sub">{t.actions.execute}</span>
                    {:else}
                      <span class="item-name">{item.name}</span>
                      <span class="item-sub">{item.desc}</span>
                    {/if}
                  </div>
                  {#if fi === selectedIndex}<span class="item-badge">{al(item)}</span>{/if}
                </div>
              {/each}
            {/each}
          </div>

          {#if selectedItem}
            <div class="detail-panel" key={selectedItem.name}>
              <div class="detail-icon-wrap">
                <div class="detail-icon-glow" style="background: radial-gradient(circle, rgba(var(--accent-rgb),0.2) 0%, transparent 65%)"></div>
                <span class="detail-icon">{selectedItem.icon}</span>
              </div>
              <div class="detail-body">
                <div class="meta-heading">{t.common.metadata}</div>
                <div class="meta-row" style="animation-delay:0ms">
                  <span class="meta-k">{t.metadata.name}</span>
                  <span class="meta-v">
                    {#if selectedItem.result_type === 'web_search'}
                      {t.actions.web_search}: "{query}"
                    {:else if selectedItem.result_type === 'youtube_search'}
                      {t.actions.youtube_search}: "{query}"
                    {:else if selectedItem.result_type === 'command'}
                      {t.actions.execute}: "{selectedItem.name.replace('Run: ', '')}"
                    {:else}
                      {selectedItem.name}
                    {/if}
                  </span>
                </div>
                {#if selectedItem.desc}
                  <div class="meta-row" style="animation-delay:40ms">
                    <span class="meta-k">{t.metadata.where}</span>
                    <span class="meta-v mono">
                      {#if selectedItem.result_type === 'web_search'}
                        {t.actions.open_in_browser}
                      {:else if selectedItem.result_type === 'youtube_search'}
                        {t.actions.open_in_browser}
                      {:else if selectedItem.result_type === 'command'}
                        {t.actions.execute}
                      {:else}
                        {selectedItem.desc}
                      {/if}
                    </span>
                  </div>
                {/if}
                <div class="meta-row" style="animation-delay:80ms"><span class="meta-k">{t.metadata.type}</span><span class="meta-v">{tl(selectedItem.result_type)}</span></div>
                {#if selectedItem.path && selectedItem.path !== selectedItem.desc}
                  <div class="meta-row" style="animation-delay:120ms"><span class="meta-k">{t.metadata.path}</span><span class="meta-v mono">{selectedItem.path}</span></div>
                {/if}
              </div>
            </div>
          {/if}
        </div>

      {:else}
        <div class="split">
          <div class="results-list">
            {#if recentFiles.length > 0}
              <div class="section-label">{t.sections.recent}</div>
              {#each recentFiles as file, i}
                <div class="result-item recent-row" role="button" tabindex="0" style="animation: slide-in 0.22s cubic-bezier(0.34,1.3,0.64,1) {i * 45}ms both" on:click={() => selectResult(file)}>
                  <span class="item-icon">{file.icon}</span>
                  <div class="item-info">
                    <span class="item-name">{file.name}</span>
                    <span class="item-sub">{file.desc}</span>
                  </div>
                </div>
              {/each}
            {/if}

            <div class="section-label">{t.common.suggestions}</div>
            {#each [
              {icon:'🔒', name:'lock', sub: t.suggestions.lock},
              {icon:'⏻', name:'shutdown', sub: t.suggestions.shutdown},
              {icon:'🔄', name:'reboot', sub: t.suggestions.reboot},
              {icon:'🖥️', name:'> whoami', sub: t.suggestions.terminal},
              {icon:'🧮', name:'2 + 2', sub: t.suggestions.calculator},
            ] as tip}
              <div class="result-item tip">
                <span class="item-icon">{tip.icon}</span>
                <div class="item-info">
                  <span class="item-name">{tip.name}</span>
                  <span class="item-sub">{tip.sub}</span>
                </div>
              </div>
            {/each}
          </div>

          <div class="detail-panel welcome">
  <div class="welcome-orb"></div>
  <div class="welcome-orb2"></div>
  <div class="w-title-big">Universal</div>
  <div class="w-stats">
    <div class="w-stat">
      <span class="w-stat-num">{totalApps}</span>
      <span class="w-stat-label">{t.common.apps}</span>
    </div>
    <div class="w-stat-divider"></div>
    <div class="w-stat">
      <span class="w-stat-num">{totalFiles}</span>
      <span class="w-stat-label">{t.common.files}</span>
    </div>
  </div>
</div>
        </div>
      {/if}

    </div>

    <div class="footer">
      <span class="fhint"><kbd>↑↓</kbd> {t.footer.navigate}</span>
      <span class="fhint"><kbd>↵</kbd> {selectedItem ? al(selectedItem) : t.footer.open}</span>
      <span class="fhint"><kbd>Esc</kbd> {t.footer.close}</span>
      <span class="fsep"></span>
      {#if selectedItem && !showSettings && !showSysInfo}
        <span class="ftype" style="animation: fade-in 0.2s ease">{tl(selectedItem.result_type)}</span>
      {/if}
      {#if showSettings}
        <span class="ftype" style="animation: fade-in 0.2s ease; color: rgba(var(--accent-rgb),0.5)">{t.settings.title}</span>
      {/if}
      {#if showSysInfo}
        <span class="ftype" style="animation: fade-in 0.2s ease; color: rgba(var(--accent-rgb),0.5)">{t.common.systemInfo}</span>
      {/if}
      {#if showEmoji}
        <span class="ftype" style="animation: fade-in 0.2s ease; color: rgba(var(--accent-rgb),0.5)">{t.common.emojis}</span>
      {/if}
    </div>

  </div>
</div>

<style>
  :global(*, *::before, *::after) { box-sizing: border-box; margin: 0; padding: 0; }
  :global(html, body) { width: 100%; height: 100%; background: transparent !important; overflow: hidden; }

  :global(:root) {
    --accent: #3d7fff;
    --accent-rgb: 61,127,255;
    --bg1: #0e1628;
    --bg2: #080d1a;
    --glow-rgb: 30,80,200;
    scroll-behavior: smooth;
  }

  .root {
  width: 100vw;
  height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
}

  .launcher {
    width: 100%;
  height: 100%;
  aspect-ratio: 740 / 480;
  display: flex;
  flex-direction: column;
  border-radius: 14px;
  overflow: hidden;
    font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', sans-serif;
    background: radial-gradient(ellipse 80% 55% at 65% 0%, rgba(var(--accent-rgb), 0.16) 0%, transparent 60%),
                radial-gradient(ellipse 45% 35% at 5% 100%, rgba(var(--accent-rgb), 0.10) 0%, transparent 55%),
                linear-gradient(155deg, var(--bg1) 0%, var(--bg2) 100%);
    border: 1px solid rgba(var(--accent-rgb), 0.18);
    box-shadow: 0 0 0 0.5px rgba(var(--accent-rgb), 0.07) inset, 0 0 70px rgba(var(--glow-rgb), 0.22), 0 32px 90px rgba(0,0,0,0.8);
    opacity: 0; transform: scale(0.93) translateY(10px);
    transition: opacity 0.22s ease, transform 0.22s cubic-bezier(0.34,1.4,0.64,1);
  }
  .launcher.visible { opacity: 1; transform: scale(1) translateY(0); }

  .emoji-btn { font-size: 14px; margin-right: 0px; }
  .emoji-btn.active { background: rgba(var(--accent-rgb), 0.22); border-color: rgba(var(--accent-rgb), 0.45); }

  .emoji-item {
    width: 54px; height: 54px; font-size: 34px;
    border: 1px solid rgba(255,255,255,0.1); border-radius: 14px;
    background: rgba(255,255,255,0.05); cursor: pointer;
    display: flex; align-items: center; justify-content: center;
    transition: all 0.2s cubic-bezier(0.34,1.56,0.64,1);
    box-shadow: 0 2px 8px rgba(0,0,0,0.25);
  }
  .emoji-item:hover { background: rgba(var(--accent-rgb), 0.22); transform: scale(1.18); border-color: rgba(var(--accent-rgb), 0.5); }

  @keyframes fade-out { to { opacity: 0; transform: translateY(-10px); } }

  .search-bar { display: flex; align-items: center; gap: 6px; padding: 12px 14px; border-bottom: 1px solid rgba(var(--accent-rgb), 0.1); background: rgba(255,255,255,0.022); flex-shrink: 0; }

  .icon-btn { 
  width: 28px; 
  height: 28px; 
  min-width: 28px;
  min-height: 28px;
  border: 1px solid rgba(255,255,255,0.08); 
  border-radius: 7px; 
  background: rgba(255,255,255,0.05); 
  color: rgba(255,255,255,0.38); 
  cursor: pointer; 
  display: flex; 
  align-items: center; 
  justify-content: center; 
  flex-shrink: 0; 
  transition: background 0.15s, color 0.15s, border-color 0.15s, transform 0.1s; 
}
  .icon-btn:hover { background: rgba(var(--accent-rgb), 0.15); border-color: rgba(var(--accent-rgb), 0.35); color: rgba(255,255,255,0.9); transform: scale(1.08); }
  .icon-btn:active { transform: scale(0.95); }
  .settings-btn { margin-left: 0px; }
  .settings-btn.active { background: rgba(var(--accent-rgb), 0.22); border-color: rgba(var(--accent-rgb), 0.45); color: var(--accent); }
  .settings-btn.spin svg { animation: gear-spin 0.45s cubic-bezier(0.34,1.2,0.64,1); }
  .sys-btn { font-size: 14px; margin-right: 0px; }
  .sys-btn.active { background: rgba(var(--accent-rgb), 0.22); border-color: rgba(var(--accent-rgb), 0.45); color: var(--accent); }

  @keyframes gear-spin { from { transform: rotate(0deg); } to { transform: rotate(180deg); } }

  .input-wrap { flex: 1; position: relative; min-width: 0; }
  .input-wrap input { width: 100%; background: none; border: none; outline: none; color: #e8eeff; font-size: 14.5px; font-weight: 400; letter-spacing: 0.01em; caret-color: var(--accent); position: relative; z-index: 1; }
  .placeholder { position: absolute; left: 0; top: 50%; transform: translateY(-50%); color: rgba(180,200,255,0.28); font-size: 14.5px; pointer-events: none; user-select: none; transition: opacity 0.2s ease; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; width: 100%; }
  .placeholder.fade { opacity: 0; }

  .spinner { width: 15px; height: 15px; border: 2px solid rgba(var(--accent-rgb), 0.18); border-top-color: var(--accent); border-radius: 50%; animation: spin 0.65s linear infinite; flex-shrink: 0; }
  @keyframes spin { to { transform: rotate(360deg); } }

  .body { display: flex; flex: 1; min-height: 0; overflow: hidden; }
  .split { display: flex; flex: 1; min-height: 0; overflow: hidden; }

  .results-list {
    width: 252px; flex-shrink: 0;
    overflow-y: auto; padding: 6px 0;
    border-right: 1px solid rgba(var(--accent-rgb), 0.1);
    scrollbar-width: thin;
    scrollbar-color: rgba(var(--accent-rgb), 0.15) transparent;
    scroll-behavior: smooth;
    -webkit-overflow-scrolling: touch;
  }
  .results-list::-webkit-scrollbar { width: 3px; }
  .results-list::-webkit-scrollbar-thumb { background: rgba(var(--accent-rgb), 0.2); border-radius: 3px; }

  .section-label { font-size: 10.5px; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; color: rgba(var(--accent-rgb), 0.38); padding: 10px 14px 4px; user-select: none; }

  .result-item { display: flex; align-items: center; gap: 9px; padding: 7px 12px 7px 14px; cursor: pointer; user-select: none; border-left: 2px solid transparent; transition: background 0.1s, border-color 0.1s, transform 0.08s; animation: slide-in 0.2s cubic-bezier(0.34,1.2,0.64,1) both; }
  .result-item:hover { background: rgba(var(--accent-rgb), 0.07); transform: translateX(2px); }
  .result-item.selected { background: rgba(var(--accent-rgb), 0.13); border-left-color: var(--accent); transform: translateX(0); }
  .recent-row { opacity: 0.8; }
  .recent-row:hover { opacity: 1; }
  .tip { opacity: 0.5; }
  .tip:hover { opacity: 0.75; }

  @keyframes slide-in { from { opacity: 0; transform: translateX(-8px); } to { opacity: 1; transform: translateX(0); } }
  @keyframes fade-in { from { opacity: 0; } to { opacity: 1; } }

  .emoji-container { display: flex; flex-direction: column; gap: 24px; padding: 8px 12px; position: relative; overflow: visible; }
  .emoji-category { display: flex; flex-direction: column; gap: 8px; }
  .category-title { font-size: 13px; font-weight: 700; color: rgba(var(--accent-rgb), 0.8); padding: 4px 8px 6px; letter-spacing: 0.6px; text-transform: uppercase; border-bottom: 1px solid rgba(var(--accent-rgb), 0.15); }
  .emoji-grid { display: flex; flex-wrap: wrap; gap: 10px; justify-content: flex-start; padding: 4px 8px; }

  .item-icon { font-size: 15px; width: 20px; text-align: center; flex-shrink: 0; line-height: 1; }
  .item-info { display: flex; flex-direction: column; min-width: 0; flex: 1; }
  .item-name { font-size: 13px; color: rgba(220,235,255,0.85); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; line-height: 1.35; }
  .result-item.selected .item-name { color: #fff; }
  .item-sub { font-size: 11px; color: rgba(var(--accent-rgb), 0.38); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; line-height: 1.3; margin-top: 1px; }
  .item-badge { font-size: 10px; color: rgba(var(--accent-rgb), 0.6); white-space: nowrap; flex-shrink: 0; background: rgba(var(--accent-rgb), 0.1); border: 1px solid rgba(var(--accent-rgb), 0.22); border-radius: 4px; padding: 1px 6px; animation: fade-in 0.15s ease; }

  .detail-panel { flex: 1; min-width: 0; overflow-y: auto; display: flex; flex-direction: column; animation: fade-in 0.18s ease; }
  .detail-icon-wrap { padding: 20px 0 12px; text-align: center; position: relative; }
  .detail-icon-glow { position: absolute; top: 50%; left: 50%; transform: translate(-50%,-50%); width: 100px; height: 100px; pointer-events: none; animation: pulse-glow 2.5s ease-in-out infinite; }
  @keyframes pulse-glow { 0%,100% { opacity: 0.6; transform: translate(-50%,-50%) scale(1); } 50% { opacity: 1; transform: translate(-50%,-50%) scale(1.15); } }
  .detail-icon { font-size: 52px; line-height: 1; position: relative; animation: icon-bounce 0.35s cubic-bezier(0.34,1.5,0.64,1); }
  @keyframes icon-bounce { from { transform: scale(0.6); opacity: 0; } to { transform: scale(1); opacity: 1; } }

  .detail-body { padding: 0 18px 16px; }
  .meta-heading { font-size: 10px; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; color: rgba(var(--accent-rgb), 0.3); margin-bottom: 10px; }
  .meta-row { display: flex; justify-content: space-between; align-items: flex-start; gap: 14px; padding: 5px 0; border-bottom: 1px solid rgba(var(--accent-rgb), 0.07); animation: slide-in 0.2s cubic-bezier(0.34,1.2,0.64,1) both; }
  .meta-row:last-child { border-bottom: none; }
  .meta-k { font-size: 12px; color: rgba(var(--accent-rgb), 0.32); flex-shrink: 0; padding-top: 1px; }
  .meta-v { font-size: 12px; color: rgba(200,220,255,0.72); text-align: right; word-break: break-all; }
  .meta-v.mono { font-family: 'SF Mono','Fira Code',monospace; font-size: 11px; color: rgba(var(--accent-rgb), 0.45); }

  .search-logo {
  width: 24px;
  height: 24px;
  flex-shrink: 0;
  filter: drop-shadow(0 0 8px rgba(var(--accent-rgb), 0.3));
  margin-left: 4px;
  transition: transform 0.2s;
}
.search-logo:hover {
  transform: scale(1.1);
}

  .welcome { align-items: center; justify-content: center; gap: 0; position: relative; overflow: hidden; }
  .welcome-orb {
  position: absolute;
  top: 20%;
  left: 50%;
  transform: translate(-50%,-50%);
  width: 240px;
  height: 240px;
  background: radial-gradient(circle, rgba(var(--accent-rgb),0.08) 0%, transparent 70%);
  pointer-events: none;
  will-change: transform, opacity;
  opacity: 0.5;
  transition: opacity 0.8s ease;
}

.welcome-orb2 {
  position: absolute;
  bottom: 10%;
  right: 10%;
  width: 120px;
  height: 120px;
  background: radial-gradient(circle, rgba(var(--accent-rgb),0.05) 0%, transparent 70%);
  pointer-events: none;
  will-change: transform, opacity;
  opacity: 0.3;
  transition: opacity 1s ease;
}
  @keyframes float { 0%,100% { transform: translateY(0px); } 50% { transform: translateY(-6px); } }
  @keyframes pop-chip { from { opacity: 0; transform: scale(0.7) translateY(6px); } to { opacity: 1; transform: scale(1) translateY(0); } }


  .w-icon { font-size: 42px; margin-bottom: 12px; filter: drop-shadow(0 0 20px rgba(var(--accent-rgb),0.5)); position: relative; z-index: 1; }
  .w-title { font-size: 16px; font-weight: 600; color: rgba(220,235,255,0.9); letter-spacing: -0.015em; margin-bottom: 4px; position: relative; z-index:1; }
  .w-sub { font-size: 12px; color: rgba(var(--accent-rgb), 0.4); margin-bottom: 18px; position: relative; z-index:1; }
  .w-chips { display: flex; flex-wrap: wrap; gap: 5px; justify-content: center; max-width: 260px; position: relative; z-index:1; }
  .w-chips span { font-size: 11px; color: rgba(var(--accent-rgb), 0.5); background: rgba(var(--accent-rgb), 0.08); border: 1px solid rgba(var(--accent-rgb), 0.15); border-radius: 20px; padding: 3px 9px; transition: background 0.2s, transform 0.2s; }
  .w-chips span:hover { background: rgba(var(--accent-rgb), 0.15); transform: translateY(-2px); }

  .settings-panel { flex: 1; overflow-y: auto; padding: 16px; animation: fade-in 0.2s ease; scrollbar-width: thin; scrollbar-color: rgba(var(--accent-rgb),0.1) transparent; }
  .settings-inner { display: flex; flex-direction: column; gap: 8px; }
  .settings-section-title { font-size: 10px; font-weight: 700; letter-spacing: 0.1em; text-transform: uppercase; color: rgba(var(--accent-rgb), 0.4); margin-top: 6px; margin-bottom: 4px; }
  .theme-label { font-size: 12px; color: rgba(220,235,255,0.55); margin-bottom: 8px; }
  .theme-grid { display: flex; flex-direction: column; gap: 4px; margin-bottom: 4px; }
  .theme-swatch { display: flex; align-items: center; gap: 10px; padding: 9px 12px; border-radius: 9px; border: 1px solid rgba(255,255,255,0.06); background: rgba(255,255,255,0.03); cursor: pointer; color: rgba(220,235,255,0.65); font-size: 13px; font-family: inherit; transition: background 0.15s, border-color 0.15s, transform 0.1s; animation: slide-in 0.22s cubic-bezier(0.34,1.2,0.64,1) both; }
  .theme-swatch:hover { background: rgba(255,255,255,0.07); border-color: rgba(var(--sw), 0.35); transform: translateX(3px); color: #fff; }
  .theme-swatch.active { background: rgba(var(--accent-rgb), 0.1); border-color: rgba(var(--accent-rgb), 0.35); color: #fff; }
  .swatch-dot { width: 12px; height: 12px; border-radius: 50%; flex-shrink: 0; transition: transform 0.2s; }
  .theme-swatch:hover .swatch-dot, .theme-swatch.active .swatch-dot { transform: scale(1.25); }
  .flag-emoji { font-size: 18px; width: 24px; text-align: center; flex-shrink: 0; transition: transform 0.2s; }
  .theme-swatch:hover .flag-emoji, .theme-swatch.active .flag-emoji { transform: scale(1.2); }
  .swatch-name { flex: 1; text-align: left; }
  .swatch-check { font-size: 12px; color: var(--accent); animation: pop-chip 0.2s cubic-bezier(0.34,1.5,0.64,1); }
  .settings-divider { height: 1px; background: rgba(var(--accent-rgb), 0.08); margin: 8px 0; }

  .w-title-big {
  font-size: 56px;
  font-weight: 700;
  letter-spacing: -1px;
  margin-bottom: 16px;
  position: relative;
  z-index: 1;
  background: linear-gradient(135deg, #fff 0%, rgba(180,200,255,0.7) 100%);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}
  .w-stats { display: flex; align-items: center; gap: 24px; position: relative; z-index: 1; }
  .w-stat { display: flex; flex-direction: column; align-items: center; }
  .w-stat-num {
  font-size: 38px;
  font-weight: 700;
  color: #fff;
}
  .w-stat-label {
  font-size: 16px;
  font-weight: 700;
  color: rgba(200,220,255,0.7);
  text-transform: uppercase;
  letter-spacing: 2px;
  margin-top: 4px;
}
  .w-stat-divider { width: 1px; height: 36px; background: rgba(255,255,255,0.08); }

  .copied-toast {
    position: fixed; bottom: 80px; left: 50%; transform: translateX(-50%);
    background: linear-gradient(145deg, rgba(16, 185, 129, 0.85), rgba(5, 150, 105, 0.9));
    color: white; font-size: 14px; font-weight: 600; padding: 11px 20px; border-radius: 12px;
    box-shadow: 0 8px 25px rgba(0,0,0,0.45), 0 0 0 1px rgba(255,255,255,0.15) inset, 0 0 15px rgba(16, 185, 129, 0.4);
    z-index: 300; white-space: nowrap; pointer-events: none; backdrop-filter: blur(12px);
    border: 1px solid rgba(255,255,255,0.18); animation: toastPop 1.65s ease forwards; opacity: 0;
  }
  @keyframes toastPop {
    0%   { opacity: 0; transform: translate(-50%, 35px) scale(0.85); }
    18%  { opacity: 1; transform: translate(-50%, 0) scale(1); }
    75%  { opacity: 1; transform: translate(-50%, -18px) scale(1); }
    100% { opacity: 0; transform: translate(-50%, -55px) scale(0.92); }
  }

  .w-logo { width: 64px; height: 64px; margin-bottom: 12px; position: relative; z-index: 1; filter: drop-shadow(0 0 20px rgba(var(--accent-rgb),0.5)); }

  .info-card { display: flex; align-items: center; gap: 14px; padding: 14px; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.06); border-radius: 12px; animation: slide-in 0.2s ease both; }
  .info-card-icon { font-size: 28px; width: 48px; height: 48px; display: flex; align-items: center; justify-content: center; background: rgba(var(--accent-rgb), 0.1); border-radius: 12px; flex-shrink: 0; }
  .info-card-content { flex: 1; min-width: 0; }
  .info-card-title { font-size: 13px; font-weight: 600; color: rgba(220,235,255,0.9); margin-bottom: 2px; }
  .info-card-sub { font-size: 11px; color: rgba(200,220,255,0.5); margin-bottom: 6px; }
  .info-card-badge { display: inline-block; font-size: 10px; color: rgba(var(--accent-rgb), 0.7); background: rgba(var(--accent-rgb), 0.1); padding: 2px 8px; border-radius: 10px; }
  .progress-bar { width: 100%; height: 4px; background: rgba(255,255,255,0.06); border-radius: 2px; overflow: hidden; margin-bottom: 4px; }
  .progress-fill { height: 100%; border-radius: 2px; transition: width 0.5s ease; }
  .progress-text { font-size: 10px; color: rgba(200,220,255,0.4); }
  .sys-loading { text-align: center; color: rgba(200,220,255,0.3); padding: 20px; }

  .toggle-row { display: flex; align-items: center; justify-content: space-between; padding: 8px 0; }
  .toggle-label { font-size: 13px; color: rgba(220,235,255,0.7); }
  .toggle-switch { width: 40px; height: 22px; background: rgba(255,255,255,0.08); border-radius: 11px; cursor: pointer; position: relative; transition: background 0.2s; }
  .toggle-switch.on { background: var(--accent); }
  .toggle-thumb { width: 16px; height: 16px; background: #fff; border-radius: 50%; position: absolute; top: 3px; left: 3px; transition: transform 0.2s; }
  .toggle-switch.on .toggle-thumb { transform: translateX(18px); }

  .about-row { display: flex; justify-content: space-between; padding: 5px 0; border-bottom: 1px solid rgba(var(--accent-rgb), 0.06); }
  .about-row:last-child { border-bottom: none; }
  .about-key { font-size: 12px; color: rgba(var(--accent-rgb), 0.32); }
  .about-val { font-size: 12px; color: rgba(200,220,255,0.6); }

  .made-with { display: flex; align-items: center; justify-content: center; gap: 6px; margin-top: 20px; font-size: 13px; color: rgba(200,220,255,0.5); }
  .studio-link { color: var(--accent); font-weight: 600; letter-spacing: 0.3px; text-decoration: none; cursor: pointer; transition: text-shadow 0.3s, color 0.3s; }
  .studio-link:hover { color: #fff; text-shadow: 0 0 12px var(--accent), 0 0 24px var(--accent); }

  .footer { display: flex; align-items: center; gap: 10px; padding: 7px 14px; border-top: 1px solid rgba(var(--accent-rgb), 0.1); background: rgba(0,0,0,0.15); flex-shrink: 0; }
  .fhint { display: flex; align-items: center; gap: 4px; font-size: 11px; color: rgba(var(--accent-rgb), 0.28); }
  .fsep { flex: 1; }
  .ftype { font-size: 11px; color: rgba(var(--accent-rgb), 0.25); font-style: italic; }

  kbd { background: rgba(var(--accent-rgb), 0.1); border: 1px solid rgba(var(--accent-rgb), 0.2); border-radius: 4px; padding: 1px 5px; font-family: inherit; font-size: 10px; color: rgba(var(--accent-rgb), 0.5); line-height: 1.6; }
</style>