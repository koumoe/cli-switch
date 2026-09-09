(function (global) {
  'use strict';
  const ALLOWED_STATUS = new Set(['running','response_finished','completed','failed','cancelled','unknown']);
  const LABELS = {
    'zh-CN': { activityTitle:'活动', openActivities:'在主窗口查看全部活动', running:'执行中', response_finished:'响应完成', completed:'已完成', failed:'失败', cancelled:'已取消', unknown:'状态未知', noActivity:'暂无活动', close:'关闭', petWorking:'小拨：查看活动', toastDone:'本轮完成', petDocked:'小拨贴边露出半张脸' },
    'en-US': { activityTitle:'Activities', openActivities:'View all activities', running:'Running', response_finished:'Response finished', completed:'Completed', failed:'Failed', cancelled:'Cancelled', unknown:'Unknown status', noActivity:'No activities', close:'Close', petWorking:'Pibo: view activities', toastDone:'Turn completed', petDocked:'Pibo peeking from the edge' }
  };
  function localeOf(value) { return value === 'en-US' ? 'en-US' : 'zh-CN'; }
  function titleOf(entry, locale) {
    if (entry && typeof entry.title === 'string' && entry.title.trim()) return entry.title.trim().slice(0, 120);
    if (entry && typeof entry.thread_id === 'string' && entry.thread_id.trim()) return `Codex · ${entry.thread_id.trim().slice(0, 8)}`;
    const bits = [entry && entry.protocol, entry && entry.model].filter(v => typeof v === 'string' && v.trim());
    const id = entry && typeof entry.id === 'string' ? entry.id.slice(0, 8) : '';
    const fallback = locale === 'en-US' ? 'Request activity' : '请求活动';
    return (bits.length ? bits.join(' / ') : fallback) + (id ? ` · ${id}` : '');
  }
  function normalizeSnapshot(snapshot, locale) {
    const entries = Array.isArray(snapshot && snapshot.entries) ? snapshot.entries : [];
    const byId = new Map();
    for (const raw of entries) {
      if (!raw || typeof raw !== 'object' || typeof raw.id !== 'string' || !raw.id) continue;
      const status = ALLOWED_STATUS.has(raw.status) ? raw.status : 'unknown';
      const source = typeof raw.source === 'string' && raw.source.trim() ? raw.source.trim().slice(0, 80) : null;
      if (!source) continue;
      const item = { id: raw.id, kind: raw.kind === 'bridge_turn' ? 'bridge_turn' : 'proxy_request', status, title: titleOf(raw, locale), source, protocol: typeof raw.protocol === 'string' ? raw.protocol.trim().slice(0, 40) : null, project: typeof raw.project === 'string' ? raw.project.trim().slice(0, 60) : null, updated_at_ms: Number.isFinite(raw.updated_at_ms) ? raw.updated_at_ms : 0 };
      const old = byId.get(item.id); if (!old || item.updated_at_ms >= old.updated_at_ms) byId.set(item.id, item);
    }
    return Array.from(byId.values()).sort((a,b) => (a.status === 'running' ? 0 : 1) - (b.status === 'running' ? 0 : 1) || b.updated_at_ms - a.updated_at_ms);
  }
  function runningCount(entries, omitted) { return entries.reduce((n, e) => n + (e.status === 'running' ? 1 : 0), 0) + (Number.isFinite(omitted) ? Math.max(0, omitted) : 0); }
  function dragMoved(dx, dy, threshold) { const t = Number.isFinite(threshold) ? threshold : 4; return Math.hypot(dx, dy) >= t; }
  global.__PET_CORE__ = { normalizeSnapshot, runningCount, titleOf, dragMoved };
  if (!global.document) return;
  const doc = global.document;
  const root = doc.getElementById('pet-surface');
  if (!root) return;
  const surface = global.__PET_SURFACE__ || root.dataset.surface || 'pet';
  root.dataset.surface = surface;
  const pet = doc.getElementById('pet'), canvas = doc.getElementById('pet-canvas'), count = doc.getElementById('activity-count');
  const panel = doc.getElementById('activity-panel'), list = doc.getElementById('activity-list'), toast = doc.getElementById('toast'), toastText = doc.getElementById('toast-text');
  const state = { locale: 'zh-CN', dock:'none', entries:[], omitted_running:0, celebrating:false, notification:null, frame:0, lastMask:'' , drag:null, moved:false, panelOpen:false };
  function send(message) { const body = JSON.stringify(message); try { if (global.ipc && typeof global.ipc.postMessage === 'function') global.ipc.postMessage(body); else if (global.webkit && global.webkit.messageHandlers && global.webkit.messageHandlers.ipc) global.webkit.messageHandlers.ipc.postMessage(body); } catch (_) {} }
  function tr(key) { return (LABELS[state.locale] || LABELS['zh-CN'])[key] || key; }
  function statusLabel(status) { return tr(status); }
  function clear(el) { while (el && el.firstChild) el.removeChild(el.firstChild); }
  function renderRows() {
    if (!list) return; clear(list);
    const visible = state.entries.slice(0, 2);
    if (!visible.length) { const empty = doc.createElement('li'); empty.className = 'activity-meta'; empty.textContent = tr('noActivity'); list.appendChild(empty); return; }
    for (const item of visible) {
      const row = doc.createElement('li'); row.className = 'activity-row';
      const copy = doc.createElement('span'); copy.className = 'activity-copy';
      const title = doc.createElement('strong'); title.className = 'activity-title'; title.textContent = item.title;
      const meta = doc.createElement('small'); meta.className = 'activity-meta'; meta.textContent = `${item.source} · ${statusLabel(item.status)}`;
      copy.append(title, meta);
      const dot = doc.createElement('i'); dot.className = `activity-dot ${item.status === 'completed' ? 'done' : item.status === 'failed' || item.status === 'cancelled' ? 'failed' : ''}`;
      row.append(copy, dot); list.appendChild(row);
    }
  }
  function updateMask() {
    if (surface !== 'pet' || !canvas || typeof global.petHitRegions !== 'function') return;
    const total = runningCount(state.entries, state.omitted_running);
    const badgeWidth = total > 0 ? (total > 99 ? 22 : total > 9 ? 17 : 14) : 0;
    let mask = global.petHitRegions(canvas, state.dock !== 'none', badgeWidth);
    if (state.dock !== 'none') mask = mask.map(r => ({...r, width: Math.max(0, Math.min(28, r.x + r.width) - r.x)})).filter(r => r.width > 0);
    mask = mask.slice(0, 512);
    const key = JSON.stringify(mask); if (key !== state.lastMask) { state.lastMask = key; send({ type:'hit-regions', rects:mask }); }
  }
  function draw() {
    if (!canvas || typeof global.drawPetSprite !== 'function') return;
    global.drawPetSprite(canvas, { dock: state.dock, celebrating: state.celebrating, working: runningCount(state.entries, state.omitted_running) > 0 || state.celebrating, frame: state.frame }); updateMask();
    if (pet) { pet.classList.toggle('docked', state.dock !== 'none'); pet.classList.toggle('celebrating', state.celebrating); pet.setAttribute('aria-label', state.dock !== 'none' ? tr('petDocked') : tr('petWorking')); }
  }
  function setPanel(open) { if (!panel) return; state.panelOpen = !!open; panel.hidden = !state.panelOpen; if (pet) pet.setAttribute('aria-expanded', String(state.panelOpen)); }
  function apply(payload) {
    payload = payload && typeof payload === 'object' ? payload : {};
    state.locale = localeOf(payload.locale); localize(); state.dock = ['left','right'].includes(payload.dock) ? payload.dock : 'none'; state.entries = normalizeSnapshot(payload.snapshot, state.locale); state.omitted_running = Number.isFinite(payload.snapshot && payload.snapshot.omitted_running) ? Math.max(0, payload.snapshot.omitted_running) : 0; state.celebrating = !!payload.celebrating; state.notification = payload.notification && typeof payload.notification.title === 'string' ? { title: payload.notification.title.slice(0, 120), count: Number(payload.notification.count) || 1 } : null;
    const n = runningCount(state.entries, state.omitted_running); if (count) { count.hidden = n < 1; count.textContent = n > 99 ? '99+' : String(n); }
    renderRows(); draw();
    if (surface === 'toast' && toast) { toast.hidden = !state.notification; if (state.notification) { const suffix = tr('toastDone'); const title = state.notification.title.endsWith(suffix) ? state.notification.title : `${state.notification.title} · ${suffix}`; toastText.textContent = `${title}${state.notification.count > 1 ? ` · ${state.notification.count}` : ''}`; } }
    if (surface === 'panel') { if (panel) panel.hidden = false; if (pet) pet.hidden = true; }
  }
  global.renderPetState = apply;
  function localize() { const h = panel && panel.querySelector('[data-i18n=activityTitle]'); const o = doc.getElementById('open-activities'); const c = doc.getElementById('panel-close'); if (h) h.textContent = tr('activityTitle'); if (o) o.textContent = tr('openActivities'); if (c) c.setAttribute('aria-label', tr('close')); }
  function init() {
    localize(); if (surface === 'pet') { if (panel) panel.hidden = true; if (toast) toast.hidden = true; }
    if (surface === 'toast') { if (pet) pet.hidden = true; if (panel) panel.hidden = true; }
    const ready = () => send({type:'ready'}); ready(); draw();
    if (!pet) return;
    pet.addEventListener('click', () => { if (state.moved) { state.moved = false; return; } send({type:'toggle-list'}); });
    pet.addEventListener('contextmenu', e => { e.preventDefault(); send({type:'hide'}); });
    pet.addEventListener('pointerdown', e => { if (e.button !== 0) return; state.drag = { x:e.clientX, y:e.clientY, started:false }; state.moved = false; pet.setPointerCapture(e.pointerId); });
    pet.addEventListener('pointermove', e => { if (!state.drag) return; const dx=e.clientX-state.drag.x, dy=e.clientY-state.drag.y; if (!state.moved && !global.__PET_CORE__.dragMoved(dx, dy, 4)) return; state.moved=true; pet.classList.add('dragging'); if (!state.drag.started) { state.drag.started = true; send({type:'drag-start', x:state.drag.x, y:state.drag.y}); } send({type:'drag-move', x:e.clientX, y:e.clientY}); });
    pet.addEventListener('pointerup', e => { if (!state.drag) return; state.drag=null; pet.classList.remove('dragging'); if (pet.hasPointerCapture(e.pointerId)) pet.releasePointerCapture(e.pointerId); if (state.moved) send({type:'drag-end'}); });
    pet.addEventListener('pointercancel', () => { const started = !!(state.drag && state.drag.started); state.drag=null; state.moved=true; pet.classList.remove('dragging'); if (started) send({type:'drag-end'}); });
    doc.addEventListener('pointerdown', e => { if (state.panelOpen && !panel.contains(e.target) && !pet.contains(e.target)) { setPanel(false); send({type:'dismiss'}); } });
    doc.addEventListener('keydown', e => { if (e.key === 'Escape') { setPanel(false); send({type:'dismiss'}); } });
    doc.getElementById('panel-close')?.addEventListener('click', () => { setPanel(false); send({type:'dismiss'}); });
    doc.getElementById('open-activities')?.addEventListener('click', () => send({type:'open-activities'}));
    const motionOK = !global.matchMedia || !global.matchMedia('(prefers-reduced-motion: reduce)').matches; if (surface === 'pet' && motionOK) global.setInterval(() => { if (runningCount(state.entries, state.omitted_running) > 0 || state.celebrating) { state.frame = (state.frame + 1) % 16; if (!state.celebrating) draw(); } }, 900);
  }
  if (doc.readyState === 'loading') doc.addEventListener('DOMContentLoaded', init); else init();
})(typeof window !== 'undefined' ? window : globalThis);
