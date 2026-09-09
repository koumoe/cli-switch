import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
const source = fs.readFileSync(new URL('../pet.js', import.meta.url), 'utf8');
const context = { window: {}, globalThis: {}, console };
context.window = context; context.globalThis = context;
vm.runInNewContext(source, context);
const core = context.__PET_CORE__;
assert.ok(core, 'pure state helpers are exposed for native-surface tests');
assert.match(source, /textContent = item\.title/, 'activity titles use safe textContent rendering');
const entries = core.normalizeSnapshot({ entries: [
  { id:'same', status:'completed', title:'<raw prompt>', source:'Codex', updated_at_ms:1 },
  { id:'same', status:'running', title:null, source:'Codex', protocol:'openai', model:'gpt', updated_at_ms:2 },
  { id:'other', status:'failed', title:'x', source:'', updated_at_ms:4 },
  { id:'third', status:'bogus', title:'safe', source:'Claude', updated_at_ms:3 }
] });
assert.equal(entries.length, 2, 'deduplicates id and drops entries without trusted source');
assert.equal(entries[0].status, 'running', 'running activities sort first');
assert.match(entries[0].title, /openai/);
assert.equal(core.runningCount(entries), 1, 'count includes running activities only');
assert.equal(core.runningCount(entries, 2), 3, 'omitted running activities are included in count');
assert.match(source, /item\.status === 'completed' \? 'done'/, 'response_finished remains neutral');
assert.equal(core.titleOf({ id:'abcdefghi', title:null, protocol:'p', model:'m' }), 'p / m · abcdefgh');
assert.equal(core.titleOf({ id:'codex:123', title:null, thread_id:'12345678-abcd' }), 'Codex · 12345678');
assert.equal(core.titleOf({ id:'req-123', title:null }, 'en-US'), 'Request activity · req-123');
console.log('pet core tests passed');

assert.equal(core.dragMoved(2, 2, 4), false, 'small pointer movement remains a click');
assert.equal(core.dragMoved(4, 0, 4), true, 'threshold movement starts dragging');

assert.match(source, /state\.drag\.started = true; send\(\{type:'drag-start'/, 'drag-start follows movement threshold');
assert.doesNotMatch(source.split("pet.addEventListener('pointerdown'")[1].split("pet.addEventListener('pointermove'")[0], /type:'drag-start'/, 'click does not start drag');
