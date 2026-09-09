import assert from 'node:assert/strict';
import fs from 'node:fs';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { JSDOM } = require('../../../ui/node_modules/jsdom');
const dir = new URL('..', import.meta.url);
const htmlTemplate = fs.readFileSync(new URL('../index.html', import.meta.url), 'utf8')
  .replace('/*__PET_CSS__*/', fs.readFileSync(new URL('../style.css', import.meta.url), 'utf8'))
  .replace('/*__PET_SPRITE__*/', fs.readFileSync(new URL('../sprite.js', import.meta.url), 'utf8'))
  .replace('/*__PET_JS__*/', fs.readFileSync(new URL('../pet.js', import.meta.url), 'utf8'));

function makeDom(surface = 'pet') {
  const messages = [];
  const dom = new JSDOM(htmlTemplate, {
    runScripts: 'dangerously', pretendToBeVisual: true,
    beforeParse(window) {
      window.__PET_SURFACE__ = surface;
      window.ipc = { postMessage: value => messages.push(JSON.parse(value)) };
      window.matchMedia = () => ({ matches: false, addEventListener() {}, removeEventListener() {} });
      window.setPointerCapture = () => {};
      window.HTMLButtonElement.prototype.setPointerCapture = () => {};
      window.HTMLButtonElement.prototype.releasePointerCapture = () => {};
      window.HTMLButtonElement.prototype.hasPointerCapture = () => true;
      window.HTMLCanvasElement.prototype.getContext = function () {
        if (this._ctx) return this._ctx;
        const canvas = this;
        const ctx = { fillStyle: '#000', clearRect() {}, translate() {}, scale() {},
          fillRect(x, y, w, h) { ctx._painted = true; },
          getImageData() { return { data: new Uint8ClampedArray(canvas.width * canvas.height * 4).fill(255) }; } };
        this._ctx = ctx; return ctx;
      };
    }
  });
  return { dom, messages };
}
function pointer(window, type, x, y, screenX = x, screenY = y) {
  const e = new window.MouseEvent(type, { bubbles: true, button: 0, clientX: x, clientY: y, screenX, screenY });
  Object.defineProperty(e, 'pointerId', { value: 1 });
  return e;
}

{
  const { dom, messages } = makeDom('pet');
  const { document, renderPetState } = dom.window;
  await new Promise(resolve => dom.window.setTimeout(resolve, 0));
  const pet = document.getElementById('pet');
  assert.equal(document.getElementById('activity-panel').hidden, true, 'pet surface never embeds the panel');
  pet.dispatchEvent(pointer(dom.window, 'pointerdown', 10, 10));
  pet.dispatchEvent(pointer(dom.window, 'pointermove', 12, 12));
  assert.equal(messages.some(m => m.type === 'drag-start'), false, 'sub-threshold motion remains a click');
  pet.dispatchEvent(pointer(dom.window, 'pointermove', 20, 10));
  assert.equal(messages.some(m => m.type === 'drag-start'), true, 'drag-start is sent after threshold');
  assert.equal(messages.find(m => m.type === 'drag-start').x, 10, 'drag uses absolute screen coordinates');
  pet.dispatchEvent(pointer(dom.window, 'pointerup', 20, 10));
  renderPetState({ locale: 'zh-CN', dock: 'right', snapshot: { entries: [{ id:'x', status:'running', source:'Codex', title:null, thread_id:'12345678-abcd' }] } });
  const hit = messages.filter(m => m.type === 'hit-regions').at(-1);
  assert.ok(hit?.rects.some(r => r.x === 18 && r.width === 10), 'docked badge hit region stays within the 28px dock');
  dom.window.close();
}

{
  const { dom } = makeDom('panel');
  const { renderPetState, document } = dom.window;
  await new Promise(resolve => dom.window.setTimeout(resolve, 0));
  renderPetState({ locale: 'en-US', dock: 'none', snapshot: { entries: [{ id:'x', status:'completed', source:'Codex', title:'<unsafe>', thread_id:'12345678-abcd' }] } });
  assert.equal(document.querySelector('[data-i18n="activityTitle"]').textContent, 'Activities', 'locale is applied to panel chrome');
  assert.equal(document.querySelector('.activity-title').textContent, '<unsafe>', 'title is rendered as text');
  assert.equal(document.querySelector('.activity-title').children.length, 0, 'title cannot inject markup');
  dom.window.close();
}
console.log('desktop pet DOM tests passed');
