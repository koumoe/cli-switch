/* Pixel sprite renderer. Kept deterministic so the native surface can build a hit mask. */
(function (global) {
  'use strict';
  function drawPet(canvas, options) {
    const dock = options && options.dock && options.dock !== 'none';
    const celebrating = !!(options && options.celebrating);
    const working = !(options && options.working === false);
    const frame = (options && options.frame) || 0;
    const size = dock ? 40 : 64;
    canvas.width = size; canvas.height = size;
    const ctx = canvas.getContext('2d');
    ctx.clearRect(0, 0, size, size);
    ctx.imageSmoothingEnabled = false;
    const r = (x, y, w, h, c) => { ctx.fillStyle = c; ctx.fillRect(x, y, w, h); };
    const p = { outline:'#2b3b60', shade:'#617da7', blue:'#a6c7f0', hi:'#d7e8ff', face:'#fff0d4', skin:'#e7cfa9', eye:'#244755', green:'#407beb', white:'#eef7ff' };
    if (dock) {
      if (options.dock === 'left') { ctx.translate(28, 0); ctx.scale(-1, 1); }
      ctx.translate(-5, -1);
      r(8,12,7,8,p.outline); r(8,9,3,6,p.outline); r(10,13,3,5,p.blue);
      r(27,12,7,8,p.outline); r(31,9,3,6,p.outline); r(29,14,3,4,p.blue);
      r(12,15,18,2,p.outline); r(9,17,24,16,p.outline); r(7,20,28,10,p.outline);
      r(12,17,18,2,p.hi); r(9,20,24,9,p.blue); r(11,19,20,12,p.blue);
      r(12,21,18,9,p.skin); r(13,21,16,8,p.face); r(12,23,18,5,p.face);
      if (celebrating || frame % 16 === 8) { r(14,24,2,1,p.eye); r(16,23,2,1,p.eye); r(18,24,1,1,p.eye); r(23,24,2,1,p.eye); r(25,23,2,1,p.eye); r(27,24,1,1,p.eye); }
      else { r(15,24,2,3,p.eye); r(24,24,2,3,p.eye); }
      r(19,29,3,1,p.eye); r(13,28,3,1,'#e2b0a4'); r(26,28,3,1,'#e2b0a4');
      if (working) { r(27,32,6,5,p.outline); r(28,33,4,3,p.face); } else { r(8,32,7,4,p.face); } return;
    }
    r(47,40,9,14,p.outline); r(52,35,8,10,p.outline); r(54,31,6,9,p.outline); r(50,42,4,9,p.shade); r(55,36,3,8,p.blue);
    r(22,36,21,20,p.outline); r(20,40,25,12,p.outline); r(23,54,7,5,p.outline); r(35,54,7,5,p.outline); r(24,37,17,17,p.blue); r(24,39,17,3,p.hi);
    r(19,42,5,8,p.outline); r(20,43,4,5,p.face); r(41,42,5,8,p.outline); r(41,43,4,5,p.face);
    r(13,17,12,7,p.outline); r(13,12,4,8,p.outline); r(17,14,4,6,p.outline); r(15,16,3,5,p.blue); r(18,18,4,4,p.blue);
    r(39,17,12,7,p.outline); r(47,12,4,8,p.outline); r(43,14,4,6,p.outline); r(46,16,3,5,p.blue);
    r(19,17,25,3,p.outline); r(14,20,36,20,p.outline); r(11,24,42,11,p.outline); r(17,39,30,3,p.outline); r(19,20,25,3,p.hi); r(14,24,36,11,p.blue); r(17,22,30,17,p.blue);
    r(19,23,26,14,p.skin); r(17,26,30,8,p.skin); r(20,24,24,12,p.face); r(18,27,28,6,p.face); r(20,33,4,2,'#e9a79d'); r(39,33,4,2,'#e9a79d');
    if (celebrating || frame % 16 === 8) { r(23,28,2,2,p.eye); r(25,27,3,2,p.eye); r(28,28,2,2,p.eye); r(35,28,2,2,p.eye); r(37,27,3,2,p.eye); r(40,28,2,2,p.eye); r(29,33,2,2,p.eye); r(31,34,4,2,p.eye); r(35,33,2,2,p.eye); }
    else { r(25,28,3,4,p.eye); r(37,28,3,4,p.eye); r(31,34,4,1,p.eye); }
    if (working) { r(28,44,11,6,p.outline); r(29,45,9,4,p.green); r(34,45,4,4,p.white); }
    if (celebrating) { r(46,19,13,18,p.outline); r(48,21,9,14,p.white); r(49,27,2,3,'#27866b'); r(51,29,2,3,'#27866b'); r(53,27,2,3,'#27866b'); r(55,24,2,4,'#27866b'); r(46,35,4,10,p.outline); r(47,36,3,6,p.face); }
    else if (working) { r(16,47,33,8,p.outline); r(18,48,29,4,p.shade); r(19,50,27,2,p.hi); const keyShift = frame % 2; [21 + keyShift,26,31 + keyShift,36,41 + keyShift].forEach(x => r(x,50,2,1,p.outline)); r(21 + keyShift,45,5,4,p.face); r(38 - keyShift,45,5,4,p.face); } else { r(17,45,6,8,p.outline); r(18,46,4,5,p.face); r(40,45,6,8,p.outline); r(40,46,4,5,p.face); }
  }
  function hitRegions(canvas, dock, badgeWidth) {
    const w = canvas.width, h = canvas.height, data = canvas.getContext('2d').getImageData(0, 0, w, h).data;
    const rows = [];
    for (let y = 0; y < h; y++) { const spans = []; let start = -1; for (let x = 0; x <= w; x++) { const opaque = x < w && data[(y*w+x)*4+3] > 12; if (opaque && start < 0) start = x; if ((!opaque || x === w) && start >= 0) { spans.push([start, x - start]); start = -1; } } rows.push(spans); }
    const rects = []; let y = 0;
    while (y < h) { const key = JSON.stringify(rows[y]); let end = y + 1; while (end < h && JSON.stringify(rows[end]) === key) end++; for (const [x, width] of rows[y]) rects.push({x, y, width, height:end-y}); y = end; }
    if (badgeWidth) { const bw = dock ? 10 : Math.max(14, Math.min(24, badgeWidth)); rects.push({x: Math.max(0, (dock ? 28 : w) - bw), y: 0, width: bw, height: dock ? 10 : 14}); }
    return rects.filter(r => r.width > 0 && r.height > 0);
  }
  global.drawPetSprite = drawPet;
  global.petHitRegions = hitRegions;
})(typeof window !== 'undefined' ? window : globalThis);
